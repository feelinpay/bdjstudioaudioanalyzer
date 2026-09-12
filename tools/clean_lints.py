# Fix home_screen.dart
p = r"c:\Users\David Zapata\Desktop\Aplicacion_para_DJs\BDJ_Studio_Audio_Analyzer\frontend\lib\features\analyzer\presentation\screens\home_screen.dart"
c = open(p, "r", encoding="utf-8").read()
c = c.replace("_formatBytes(v.freeBytes.toInt())", "_formatBigIntBytes(v.freeBytes)")
c = c.replace("_formatBytes(v.totalBytes.toInt())", "_formatBigIntBytes(v.totalBytes)")
c = c.replace("SnackBar(\n          content:", "const SnackBar(\n          content:")
c = c.replace("Divider(color: AppColors.borderSubtle)", "const Divider(color: AppColors.borderSubtle)")
c = c.replace("Column(\n                        mainAxisSize: MainAxisSize.min,\n                        children: [\n                          CircularProgressIndicator",
              "Column(\n                        mainAxisSize: MainAxisSize.min,\n                        children: const [\n                          CircularProgressIndicator")
open(p, "w", encoding="utf-8").write(c)

# Fix license_manager.dart
p_lm = r"c:\Users\David Zapata\Desktop\Aplicacion_para_DJs\BDJ_Studio_Audio_Analyzer\frontend\lib\core\licensing\license_manager.dart"
c_lm = open(p_lm, "r", encoding="utf-8").read()
c_lm = c_lm.replace("return Left(DeviceFailure('Identificador de hardware no disponible.'));", "return const Left(DeviceFailure('Identificador de hardware no disponible.'));")
c_lm = c_lm.replace("return Left(LicenseFailure('Error interno durante la verificacion criptografica SPP3: $e'));", "return Left(LicenseFailure('Error interno durante la verificacion criptografica SPP3: $e'));")
open(p_lm, "w", encoding="utf-8").write(c_lm)

# Fix main.dart
p_m = r"c:\Users\David Zapata\Desktop\Aplicacion_para_DJs\BDJ_Studio_Audio_Analyzer\frontend\lib\main.dart"
c_m = open(p_m, "r", encoding="utf-8").read()
c_m = c_m.replace("final dataDir = '${appDocDir.path}${Platform.pathSeparator}bdj_audio_analyzer';", "final dataDir = '${appDocDir.path}' + Platform.pathSeparator + 'bdj_audio_analyzer';")
open(p_m, "w", encoding="utf-8").write(c_m)

print("Lints cleaned successfully!")
