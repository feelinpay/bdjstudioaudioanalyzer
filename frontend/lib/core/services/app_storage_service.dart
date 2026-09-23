import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:path/path.dart' as p;
import 'package:path_provider/path_provider.dart';

/// Punto único para los archivos administrados por BDJ Studio Audio Analyzer.
///
/// En cada plataforma la raíz de datos es el directorio de soporte que
/// proporciona `path_provider` (una sola carpeta por OS). Todos los datos viven
/// bajo esa raíz en subdirectorios con responsabilidades explícitas; los plugins
/// (preferencias, almacén seguro) escriben en la misma carpeta.
class AppStorageService {
  AppStorageService._();

  static const _brandFolder = 'BDJ Studio';
  static const _rootName = 'BDJ Studio Audio Analyzer';

  static Future<Directory> root() => _directory();
  static Future<Directory> databaseDirectory() => _directory('Database');
  static Future<Directory> cacheDirectory() => _directory('Cache');
  static Future<Directory> logsDirectory() => _directory('Logs');
  static Future<Directory> settingsDirectory() => _directory('Settings');
  static Future<Directory> tempDirectory() => _directory('Temp');

  static Future<void> initialize() async {
    final support = await _supportDirectory();
    await _migrateLegacyPluginSupport(support);
    await _migrateDesktopRoot(support);
    await Future.wait([
      root(),
      databaseDirectory(),
      cacheDirectory(),
      logsDirectory(),
      settingsDirectory(),
      tempDirectory(),
    ]);
  }

  static Future<void> clearPersistentData() async {
    final directory = await root();
    if (await directory.exists()) await directory.delete(recursive: true);
  }

  static Future<Directory> workDirectory(String operation) {
    final safeOperation = operation.replaceAll(RegExp(r'[^a-zA-Z0-9_-]'), '_');
    return _directory(
      'Temp',
      '${safeOperation}_${DateTime.now().microsecondsSinceEpoch}',
    );
  }

  static Future<Directory>? _cachedSupportDirectory;

  @visibleForTesting
  static void resetCacheForTesting() => _cachedSupportDirectory = null;

  static Future<Directory> _supportDirectory() {
    return _cachedSupportDirectory ??= getApplicationSupportDirectory();
  }

  static Future<Directory> _directory([String? first, String? second]) async {
    final support = await _supportDirectory();
    final segments = <String>[support.path];
    if (first != null) segments.add(first);
    if (second != null) segments.add(second);
    return Directory(p.joinAll(segments)).create(recursive: true);
  }

  static const _legacyPluginSupportName = 'bdj_studio_audio_analyzer';

  static Future<void> _migrateLegacyPluginSupport(Directory support) async {
    if (!Platform.isWindows) return;
    final legacy = Directory(p.join(support.parent.path, _legacyPluginSupportName));
    if (!await legacy.exists()) return;
    try {
      await support.create(recursive: true);
      for (final fileName in const [
        'flutter_secure_storage.dat',
        'shared_preferences.json',
      ]) {
        final source = File(p.join(legacy.path, fileName));
        if (!await source.exists()) continue;
        final target = File(p.join(support.path, fileName));
        if (await target.exists()) continue;
        await source.rename(target.path);
      }
      await _deleteIfEmpty(legacy);
    } catch (e) {
      debugPrint(
        'AppStorage: no se pudo migrar datos de plugins desde ${legacy.path}: $e',
      );
    }
  }

  static Future<void> _migrateDesktopRoot(Directory support) async {
    if (!Platform.isWindows && !Platform.isMacOS && !Platform.isLinux) return;
    final target = support;
    final candidates = [
      Directory(p.join(support.path, _rootName)),
      Directory(p.join(support.path, _brandFolder, _rootName)),
      Directory(p.join(support.parent.path, _rootName)),
      Directory(p.join(support.parent.path, _brandFolder, _rootName)),
    ];
    for (final legacy in candidates) {
      if (legacy.path == target.path || !await legacy.exists()) continue;
      try {
        await target.create(recursive: true);
        await _moveInto(legacy, target);
        if (legacy.parent.path != support.parent.path) {
          await _deleteIfEmpty(legacy.parent);
        }
      } catch (e) {
        debugPrint('AppStorage: no se pudo migrar datos desde ${legacy.path}: $e');
      }
    }
  }

  static Future<void> _moveInto(Directory source, Directory target) async {
    await for (final entry in source.list()) {
      final dest = p.join(target.path, p.basename(entry.path));
      if (await FileSystemEntity.type(dest) != FileSystemEntityType.notFound) {
        continue;
      }
      await entry.rename(dest);
    }
    await _deleteIfEmpty(source);
  }

  static Future<void> _deleteIfEmpty(Directory directory) async {
    if (await directory.exists() && (await directory.list().isEmpty)) {
      await directory.delete();
    }
  }
}
