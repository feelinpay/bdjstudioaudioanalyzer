import 'dart:convert';
import 'package:bdj_license_core/bdj_license_core.dart';
import 'package:crypto/crypto.dart';
import 'package:dartz/dartz.dart';
import 'package:package_info_plus/package_info_plus.dart';

import '../errors/failures.dart';
import '../security/device_fingerprint.dart';
import '../security/security_port.dart';
import 'licensing_port.dart';

class LicenseStorageKeys {
  static const String licenseKey = 'bdj.audio_analyzer.license_key';
  static const String licenseStatus = 'bdj.audio_analyzer.license_status';
  static const String deviceId = 'bdj.audio_analyzer.device_id';
  static const String hardwareFingerprint = 'bdj.audio_analyzer.hardware_fingerprint';
  static const String lastLicenseCheckUtc = 'bdj.audio_analyzer.last_license_check_utc';
  static const String lastSyncAt = 'bdj.audio_analyzer.last_sync_at';
  static const String installId = 'bdj.audio_analyzer.install_id';
}

class LicenseManager implements LicensingPort {
  final SecurityPort secureStorage;
  final DeviceFingerprint fingerprint;
  final String productCode;
  final String defaultAppVersion;

  LicenseStatus _currentStatus = LicenseStatus.none;
  String? _cachedFingerprint;
  String? _cachedAppVersion;

  LicenseManager({
    required this.secureStorage,
    required this.fingerprint,
    String? productCode,
    this.defaultAppVersion = '1.0.0',
  }) : productCode = productCode ?? BdjProduct.audioAnalyzer.code;

  @override
  LicenseStatus get currentStatus => _currentStatus;

  @override
  bool get isLicensed => _currentStatus == LicenseStatus.active;

  Future<String> _getAppVersion() async {
    if (_cachedAppVersion != null) return _cachedAppVersion!;
    try {
      final packageInfo = await PackageInfo.fromPlatform();
      if (packageInfo.version.isNotEmpty) {
        _cachedAppVersion = packageInfo.version;
        return _cachedAppVersion!;
      }
    } catch (_) {}
    _cachedAppVersion = defaultAppVersion;
    return _cachedAppVersion!;
  }

  Future<String> getHardwareFingerprint() async {
    if (_cachedFingerprint != null) return _cachedFingerprint!;
    final stored = (await secureStorage.readSecure(
      LicenseStorageKeys.hardwareFingerprint,
    )).getOrElse(() => null);
    if (stored != null && stored.isNotEmpty) {
      _cachedFingerprint = stored;
      return _cachedFingerprint!;
    }
    _cachedFingerprint = await fingerprint.generate();
    await secureStorage.storeSecure(
      LicenseStorageKeys.hardwareFingerprint,
      _cachedFingerprint!,
    );
    return _cachedFingerprint!;
  }

  Future<Result<String>> _resolveHwid() async {
    try {
      return Right(await getHardwareFingerprint());
    } catch (e) {
      return Left(DeviceFailure(
        'No se pudo leer el identificador de hardware de este equipo: $e',
      ));
    }
  }

  @override
  Future<Result<LicenseInfo>> activateLicense(String licenseKey) async {
    final cleanKey = licenseKey.trim();
    if (cleanKey.isEmpty) {
      return const Left(ValidationFailure('La clave de licencia no puede estar vacia.'));
    }

    final selfTest = await secureStorage.performSelfTest();
    if (selfTest.isLeft()) {
      _currentStatus = LicenseStatus.invalid;
      return Left(
        selfTest.fold(
          (l) => l,
          (r) => const SecurityFailure('Error verificando el almacen seguro nativo del sistema.'),
        ),
      );
    }

    final hwidResult = await _resolveHwid();
    if (hwidResult.isLeft()) {
      _currentStatus = LicenseStatus.invalid;
      return Left(hwidResult.fold(
        (l) => l,
        (r) => const DeviceFailure('Identificador de hardware no disponible.'),
      ));
    }

    final hwid = hwidResult.getOrElse(() => '');
    final appVersion = await _getAppVersion();
    return _verifySpp3(cleanKey, hwid, appVersion, persist: true);
  }

  @override
  Future<Result<LicenseInfo>> validateLicense() async {
    final storedKey = (await secureStorage.readSecure(LicenseStorageKeys.licenseKey)).getOrElse(() => null);
    final storedStatus = (await secureStorage.readSecure(LicenseStorageKeys.licenseStatus)).getOrElse(() => null);

    if (storedKey == null || storedKey.isEmpty) {
      _currentStatus = LicenseStatus.none;
      return const Left(LicenseFailure('No hay una licencia activa en este dispositivo.'));
    }

    if (storedStatus != LicenseStatus.active.name || !storedKey.startsWith('SPP3.')) {
      _currentStatus = LicenseStatus.invalid;
      return const Left(LicenseFailure('Estado de licencia invalido. Vuelve a activar tu clave SPP3.'));
    }

    final hwidResult = await _resolveHwid();
    if (hwidResult.isLeft()) {
      _currentStatus = LicenseStatus.invalid;
      return Left(hwidResult.fold(
        (l) => l,
        (r) => const DeviceFailure('Identificador de hardware no disponible.'),
      ));
    }

    final hwid = hwidResult.getOrElse(() => '');
    final appVersion = await _getAppVersion();
    final verification = await _verifySpp3(storedKey, hwid, appVersion, persist: true);
    if (verification.isLeft()) {
      _currentStatus = LicenseStatus.invalid;
    }
    return verification;
  }

  Future<Result<LicenseInfo>> _verifySpp3(
    String licenseKey,
    String hwid,
    String appVersion, {
    required bool persist,
  }) async {
    try {
      final hwidHash = KeyHierarchy.hashHwid(hwid);
      final result = await Spp3Token.verify(
        token: licenseKey,
        rootPublicKeyBase64: KeyHierarchy.ecosystemRootPublicKey,
        expectedProductCode: productCode,
        expectedVersion: appVersion,
        currentHwidHash: hwidHash,
      );

      if (!result.isValid) {
        _currentStatus = LicenseStatus.invalid;
        return Left(LicenseFailure(
          result.errorMessage ?? 'Validacion criptografica SPP3 rechazada por el sistema.',
        ));
      }

      final payload = result.payload!;
      final now = DateTime.now().toUtc();

      final lastCheck = DateTime.tryParse(
        (await secureStorage.readSecure(LicenseStorageKeys.lastLicenseCheckUtc)).getOrElse(() => null) ?? '',
      )?.toUtc();
      if (payload.expiresAtUtc != null &&
          lastCheck != null &&
          now.isBefore(lastCheck.subtract(const Duration(minutes: 5)))) {
        _currentStatus = LicenseStatus.invalid;
        return const Left(LicenseFailure(
          'La fecha y hora del sistema retrocedio de forma anormal. Ajusta tu reloj a la hora real.',
        ));
      }

      final remainingDays = payload.expiresAtUtc == null
          ? 3650
          : (payload.expiresAtUtc!.difference(now).inHours / 24).ceil();

      if (persist) {
        await _saveActivationData(
          licenseKey: licenseKey,
          status: LicenseStatus.active.name,
          deviceId: hwid,
        );
        await secureStorage.storeSecure(
          LicenseStorageKeys.lastLicenseCheckUtc,
          now.toIso8601String(),
        );
      }

      _currentStatus = LicenseStatus.active;
      return Right(LicenseInfo(
        licenseKey: licenseKey,
        status: LicenseStatus.active,
        activatedAt: payload.issuedAtUtc,
        expiresAt: payload.expiresAtUtc,
        deviceId: hwid,
        remainingOfflineDays: remainingDays > 0 ? remainingDays : 0,
      ));
    } catch (e) {
      _currentStatus = LicenseStatus.invalid;
      return Left(LicenseFailure('Error interno durante la verificacion criptografica SPP3: $e'));
    }
  }

  @override
  Future<Result<void>> deactivateLicense() async {
    await secureStorage.deleteSecure(LicenseStorageKeys.licenseKey);
    await secureStorage.deleteSecure(LicenseStorageKeys.licenseStatus);
    await secureStorage.deleteSecure(LicenseStorageKeys.lastSyncAt);
    await secureStorage.deleteSecure(LicenseStorageKeys.lastLicenseCheckUtc);
    _currentStatus = LicenseStatus.none;
    return const Right(null);
  }

  @override
  Future<Result<LicenseInfo>> syncLicense() => validateLicense();

  /// Deriva el token de capacidad efimero para inicializar el motor nativo (§14).
  String deriveCapabilityToken(String hwid, int engineRev) {
    final key = utf8.encode('BDJ_AUDIO_ANALYZER_CAPABILITY_SALT_2026');
    final message = utf8.encode('::::');
    final hmac = Hmac(sha256, key);
    return hmac.convert(message).toString();
  }

  Future<void> _saveActivationData({
    required String licenseKey,
    required String status,
    required String deviceId,
  }) async {
    await secureStorage.storeSecure(LicenseStorageKeys.licenseKey, licenseKey);
    await secureStorage.storeSecure(LicenseStorageKeys.licenseStatus, status);
    await secureStorage.storeSecure(LicenseStorageKeys.deviceId, deviceId);
    await secureStorage.storeSecure(
      LicenseStorageKeys.lastSyncAt,
      DateTime.now().toIso8601String(),
    );
  }
}
