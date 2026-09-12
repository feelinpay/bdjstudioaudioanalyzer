import 'package:bdj_license_core/bdj_license_core.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:bdj_studio_audio_analyzer/core/theme/app_colors.dart';

void main() {
  test('BdjProduct enum contains audioAnalyzer with correct code', () {
    const product = BdjProduct.audioAnalyzer;
    expect(product.code, equals('bdj_studio_audio_analyzer'));
    expect(product.displayName, equals('BDJ Studio Audio Analyzer'));
  });

  test('AppColors palette reflects logo electric cyan and midnight navy', () {
    expect(AppColors.background.toARGB32(), equals(0xFF050811));
    expect(AppColors.primary.toARGB32(), equals(0xFF00F2FE));
    expect(AppColors.verdictLossless.toARGB32(), equals(0xFF00F2FE));
    expect(AppColors.verdictTranscode.toARGB32(), equals(0xFFF43F5E));
  });
}
