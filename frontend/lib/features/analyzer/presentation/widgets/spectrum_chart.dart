import 'dart:math' as math;
import 'dart:typed_data';
import 'package:flutter/material.dart';
import '../../../../core/theme/app_colors.dart';

class SpectrumChart extends StatelessWidget {
  final Float32List spectrumDb;
  final int? cutoffHz;
  final int sampleRate;
  final double? cutoffSlopeDbOct;
  final double height;

  const SpectrumChart({
    super.key,
    required this.spectrumDb,
    this.cutoffHz,
    this.sampleRate = 44100,
    this.cutoffSlopeDbOct,
    this.height = 180,
  });

  @override
  Widget build(BuildContext context) {
    return Container(
      height: height,
      decoration: BoxDecoration(
        color: AppColors.backgroundAbyssal,
        borderRadius: BorderRadius.circular(10),
        border: Border.all(color: AppColors.surfaceBorder),
      ),
      child: ClipRRect(
        borderRadius: BorderRadius.circular(10),
        child: CustomPaint(
          size: Size.infinite,
          painter: _SpectrumPainter(
            spectrumDb: spectrumDb,
            cutoffHz: cutoffHz,
            sampleRate: sampleRate,
            cutoffSlopeDbOct: cutoffSlopeDbOct,
          ),
        ),
      ),
    );
  }
}

class _SpectrumPainter extends CustomPainter {
  final Float32List spectrumDb;
  final int? cutoffHz;
  final int sampleRate;
  final double? cutoffSlopeDbOct;

  _SpectrumPainter({
    required this.spectrumDb,
    this.cutoffHz,
    required this.sampleRate,
    this.cutoffSlopeDbOct,
  });

  @override
  void paint(Canvas canvas, Size size) {
    if (size.width <= 0 || size.height <= 0) return;

    final gridPaint = Paint()
      ..color = AppColors.surfaceBorder.withOpacity(0.4)
      ..strokeWidth = 1.0;

    final textStyle = TextStyle(
      color: AppColors.textDimmed.withOpacity(0.7),
      fontSize: 10,
      fontFamily: 'Segoe UI',
    );

    // Draw horizontal dB grid lines (-12, -36, -60, -90, -120 dB)
    final dbLevels = [-12.0, -36.0, -60.0, -90.0];
    for (final db in dbLevels) {
      final y = _dbToY(db, size.height);
      canvas.drawLine(Offset(0, y), Offset(size.width, y), gridPaint);

      final textSpan = TextSpan(text: '${db.toInt()} dB', style: textStyle);
      final tp = TextPainter(text: textSpan, textDirection: TextDirection.ltr)..layout();
      tp.paint(canvas, Offset(6, y - 12));
    }

    // Draw vertical frequency grid lines (4k, 8k, 12k, 16k, 20k)
    final nyquist = (sampleRate / 2).toDouble();
    final freqMarkers = [4000, 8000, 12000, 16000, 20000];
    for (final f in freqMarkers) {
      if (f < nyquist) {
        final x = (f / nyquist) * size.width;
        canvas.drawLine(Offset(x, 0), Offset(x, size.height), gridPaint);

        final textSpan = TextSpan(text: '${f ~/ 1000}k', style: textStyle);
        final tp = TextPainter(text: textSpan, textDirection: TextDirection.ltr)..layout();
        tp.paint(canvas, Offset(x + 3, size.height - 14));
      }
    }

    if (spectrumDb.isEmpty) return;

    // Draw spectrum curve and filled gradient
    final path = Path();
    final fillPath = Path();

    final n = spectrumDb.length;
    for (int i = 0; i < n; i++) {
      final x = (i / (n - 1)) * size.width;
      final db = spectrumDb[i].clamp(-120.0, 0.0);
      final y = _dbToY(db.toDouble(), size.height);

      if (i == 0) {
        path.moveTo(x, y);
        fillPath.moveTo(x, size.height);
        fillPath.lineTo(x, y);
      } else {
        path.lineTo(x, y);
        fillPath.lineTo(x, y);
      }
    }

    fillPath.lineTo(size.width, size.height);
    fillPath.close();

    // Fill gradient
    final fillGradient = LinearGradient(
      begin: Alignment.topCenter,
      end: Alignment.bottomCenter,
      colors: [
        AppColors.electricCyan.withOpacity(0.25),
        AppColors.electricCyan.withOpacity(0.02),
      ],
    );
    final fillPaint = Paint()
      ..shader = fillGradient.createShader(Rect.fromLTWH(0, 0, size.width, size.height))
      ..style = PaintingStyle.fill;
    canvas.drawPath(fillPath, fillPaint);

    // Stroke curve
    final curvePaint = Paint()
      ..color = AppColors.electricCyan
      ..strokeWidth = 2.0
      ..style = PaintingStyle.stroke;
    canvas.drawPath(path, curvePaint);

    // Draw Cutoff Marker if present
    if (cutoffHz != null && cutoffHz! > 0 && cutoffHz! < nyquist) {
      final cutoffX = (cutoffHz! / nyquist) * size.width;

      final isBrickwall = (cutoffSlopeDbOct ?? 0) >= 50.0;
      final cutoffColor = isBrickwall ? AppColors.verdictTranscode : AppColors.verdictLikely;

      final cutoffLinePaint = Paint()
        ..color = cutoffColor
        ..strokeWidth = 1.5
        ..style = PaintingStyle.stroke;

      // Draw dashed line
      double dashY = 0;
      while (dashY < size.height) {
        canvas.drawLine(
          Offset(cutoffX, dashY),
          Offset(cutoffX, math.min(dashY + 5, size.height)),
          cutoffLinePaint,
        );
        dashY += 9;
      }

      // Cutoff tag badge
      final badgeText = '${(cutoffHz! / 1000).toStringAsFixed(1)} kHz corte';
      final badgeSpan = TextSpan(
        text: badgeText,
        style: const TextStyle(
          color: Colors.white,
          fontSize: 10,
          fontWeight: FontWeight.bold,
        ),
      );
      final badgeTp = TextPainter(text: badgeSpan, textDirection: TextDirection.ltr)..layout();
      final badgeRect = RRect.fromRectAndRadius(
        Rect.fromLTWH(
          math.min(cutoffX + 4, size.width - badgeTp.width - 12),
          8,
          badgeTp.width + 8,
          badgeTp.height + 4,
        ),
        const Radius.circular(4),
      );
      canvas.drawRRect(badgeRect, Paint()..color = cutoffColor.withOpacity(0.85));
      badgeTp.paint(canvas, Offset(badgeRect.left + 4, badgeRect.top + 2));
    }
  }

  double _dbToY(double db, double height) {
    // 0 dB is at top (y = 0), -120 dB is at bottom (y = height)
    final clamped = db.clamp(-120.0, 0.0);
    final norm = (clamped - 0.0) / -120.0; // 0.0 for 0 dB, 1.0 for -120 dB
    return norm * height;
  }

  @override
  bool shouldRepaint(covariant _SpectrumPainter oldDelegate) {
    return oldDelegate.spectrumDb != spectrumDb ||
        oldDelegate.cutoffHz != cutoffHz ||
        oldDelegate.cutoffSlopeDbOct != cutoffSlopeDbOct;
  }
}