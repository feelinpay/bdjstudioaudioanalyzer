import 'dart:io';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:window_manager/window_manager.dart';

import 'core/ffi/api.dart' as ffi;
import 'core/ffi/frb_generated.dart';
import 'core/licensing/license_manager.dart';
import 'core/security/device_fingerprint.dart';
import 'core/security/secure_storage_impl.dart';
import 'core/services/app_storage_service.dart';
import 'core/theme/app_colors.dart';
import 'core/theme/app_theme.dart';
import 'features/analyzer/presentation/screens/home_screen.dart';
import 'features/licensing/presentation/screens/activation_screen.dart';

void main() async {
  WidgetsFlutterBinding.ensureInitialized();

  // 1. Inicializar puente FFI Rust
  await RustLib.init();

  // 2. Configurar ventana en escritorio
  if (Platform.isWindows || Platform.isMacOS) {
    await windowManager.ensureInitialized();
    const windowOptions = WindowOptions(
      size: Size(1180, 780),
      minimumSize: Size(960, 640),
      center: true,
      backgroundColor: AppColors.background,
      title: 'BDJ Studio Audio Analyzer',
    );
    await windowManager.waitUntilReadyToShow(windowOptions, () async {
      await windowManager.show();
      await windowManager.focus();
    });
  }

  // 3. Inicializar Almacén Seguro y Licenciamiento
  final secureStorage = SecureStorageImpl();
  final fingerprint = DeviceFingerprint.withPersistentStorage(secureStorage);
  final licenseManager = LicenseManager(
    secureStorage: secureStorage,
    fingerprint: fingerprint,
  );

  runApp(
    ProviderScope(
      child: AudioAnalyzerApp(licenseManager: licenseManager),
    ),
  );
}

class AudioAnalyzerApp extends StatefulWidget {
  final LicenseManager licenseManager;

  const AudioAnalyzerApp({super.key, required this.licenseManager});

  @override
  State<AudioAnalyzerApp> createState() => _AudioAnalyzerAppState();
}

class _AudioAnalyzerAppState extends State<AudioAnalyzerApp> {
  bool _isCheckingLicense = true;
  bool _isLicensed = false;

  @override
  void initState() {
    super.initState();
    _checkLicense();
  }

  Future<void> _checkLicense() async {
    final check = await widget.licenseManager.validateLicense();
    final isLicensed = check.isRight();

    if (isLicensed) {
      await _initializeEngine();
    }

    if (mounted) {
      setState(() {
        _isLicensed = isLicensed;
        _isCheckingLicense = false;
      });
    }
  }

  Future<void> _initializeEngine() async {
    try {
      final hwid = await widget.licenseManager.getHardwareFingerprint();
      final engineRev = ffi.engineRevision();
      final capabilityToken = widget.licenseManager.deriveCapabilityToken(hwid, engineRev);

      await AppStorageService.initialize();
      final dbDir = await AppStorageService.databaseDirectory();

      await ffi.engineInit(
        capabilityToken: capabilityToken,
        dataDir: dbDir.path,
      );
    } catch (e) {
      debugPrint('Error inicializando motor nativo: $e');
    }
  }

  void _onActivated() async {
    await _initializeEngine();
    if (mounted) {
      setState(() => _isLicensed = true);
    }
  }

  void _onRevoked() async {
    await widget.licenseManager.deactivateLicense();
    if (mounted) {
      setState(() => _isLicensed = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'BDJ Studio Audio Analyzer',
      debugShowCheckedModeBanner: false,
      theme: AppTheme.darkTheme,
      home: _isCheckingLicense
          ? const Scaffold(
              backgroundColor: AppColors.background,
              body: Center(
                child: CircularProgressIndicator(color: AppColors.primary),
              ),
            )
          : _isLicensed
              ? HomeScreen(
                  licenseManager: widget.licenseManager,
                  onDeactivate: _onRevoked,
                  onRevokeLicense: _onRevoked,
                )
              : ActivationScreen(
                  licenseManager: widget.licenseManager,
                  onActivated: _onActivated,
                ),
    );
  }
}
