import 'package:flutter/material.dart';
import '../../../../core/ffi/api.dart';
import '../../../../core/theme/app_colors.dart';
import 'spectrum_chart.dart';

class FileDetailsModal extends StatelessWidget {
  final FileReportFfi report;

  const FileDetailsModal({super.key, required this.report});

  Color _getVerdictColor(String code) {
    switch (code) {
      case 'LosslessVerified':
        return AppColors.verdictVerified;
      case 'LikelyLossless':
        return AppColors.verdictLikely;
      case 'Inconclusive':
        return AppColors.verdictInconclusive;
      case 'Suspicious':
        return AppColors.verdictSuspicious;
      case 'ProbableTranscode':
        return AppColors.verdictTranscode;
      default:
        return AppColors.verdictDeclared;
    }
  }

  IconData _getVerdictIcon(String code) {
    switch (code) {
      case 'LosslessVerified':
        return Icons.verified_rounded;
      case 'LikelyLossless':
        return Icons.check_circle_outline_rounded;
      case 'Inconclusive':
        return Icons.help_outline_rounded;
      case 'Suspicious':
        return Icons.warning_amber_rounded;
      case 'ProbableTranscode':
        return Icons.dangerous_rounded;
      default:
        return Icons.info_outline_rounded;
    }
  }

  String _formatDuration(BigInt ms) {
    final s = (ms.toInt() / 1000).round();
    final mins = s ~/ 60;
    final secs = s % 60;
    return '$mins:${secs.toString().padLeft(2, '0')}';
  }

  String _formatBytes(BigInt bytes) {
    final b = bytes.toInt();
    if (b < 1024 * 1024) {
      return '${(b / 1024).toStringAsFixed(1)} KB';
    }
    return '${(b / (1024 * 1024)).toStringAsFixed(2)} MB';
  }

  @override
  Widget build(BuildContext context) {
    final verdictColor = _getVerdictColor(report.verdictCode);
    final fileName = report.path.split(RegExp(r'[/\\]')).last;

    return Dialog(
      backgroundColor: AppColors.surfaceModal,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(16),
        side: const BorderSide(color: AppColors.surfaceBorder, width: 1.5),
      ),
      insetPadding: const EdgeInsets.symmetric(horizontal: 40, vertical: 24),
      child: Container(
        width: 900,
        height: 720,
        padding: const EdgeInsets.all(24),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            // Top Bar
            Row(
              children: [
                Container(
                  padding: const EdgeInsets.all(8),
                  decoration: BoxDecoration(
                    color: verdictColor.withOpacity(0.15),
                    borderRadius: BorderRadius.circular(10),
                    border: Border.all(color: verdictColor.withOpacity(0.3)),
                  ),
                  child: Icon(_getVerdictIcon(report.verdictCode), color: verdictColor, size: 24),
                ),
                const SizedBox(width: 14),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        fileName,
                        style: const TextStyle(
                          color: AppColors.textPrimary,
                          fontSize: 17,
                          fontWeight: FontWeight.bold,
                        ),
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                      ),
                      const SizedBox(height: 2),
                      Text(
                        report.path,
                        style: const TextStyle(
                          color: AppColors.textDimmed,
                          fontSize: 12,
                        ),
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                      ),
                    ],
                  ),
                ),
                IconButton(
                  onPressed: () => Navigator.of(context).pop(),
                  icon: const Icon(Icons.close_rounded, color: AppColors.textMuted),
                ),
              ],
            ),
            const SizedBox(height: 18),

            // Scrollable Content
            Expanded(
              child: SingleChildScrollView(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    // Verdict Card
                    Container(
                      padding: const EdgeInsets.all(16),
                      decoration: BoxDecoration(
                        color: verdictColor.withOpacity(0.08),
                        borderRadius: BorderRadius.circular(12),
                        border: Border.all(color: verdictColor.withOpacity(0.4)),
                      ),
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Row(
                            children: [
                              Text(
                                report.verdictName.toUpperCase(),
                                style: TextStyle(
                                  color: verdictColor,
                                  fontSize: 16,
                                  fontWeight: FontWeight.w900,
                                  letterSpacing: 1.1,
                                ),
                              ),
                              const Spacer(),
                              Text(
                                'Confianza: ${(report.confidence * 100).toStringAsFixed(0)}%',
                                style: TextStyle(
                                  color: verdictColor,
                                  fontSize: 13,
                                  fontWeight: FontWeight.bold,
                                ),
                              ),
                              const SizedBox(width: 12),
                              Text(
                                'Score LLR: ${report.scoreLlr.toStringAsFixed(1)}',
                                style: const TextStyle(
                                  color: AppColors.textSecondary,
                                  fontSize: 13,
                                  fontFamily: 'Consolas',
                                ),
                              ),
                            ],
                          ),
                          const SizedBox(height: 8),
                          Text(
                            report.verdictSummary,
                            style: const TextStyle(
                              color: AppColors.textPrimary,
                              fontSize: 13.5,
                              height: 1.4,
                            ),
                          ),
                        ],
                      ),
                    ),
                    const SizedBox(height: 20),

                    // Spectrum & Cutoff Analysis Section
                    Row(
                      children: [
                        const Icon(Icons.graphic_eq_rounded, color: AppColors.electricCyan, size: 18),
                        const SizedBox(width: 8),
                        const Text(
                          'ESPECTROGRAMA Y PUNTO DE CORTE',
                          style: TextStyle(
                            color: AppColors.textSecondary,
                            fontSize: 12,
                            fontWeight: FontWeight.bold,
                            letterSpacing: 0.8,
                          ),
                        ),
                        const Spacer(),
                        if (report.effectiveBandwidthHz != null)
                          Text(
                            'Ancho de banda: ${(report.effectiveBandwidthHz! / 1000).toStringAsFixed(1)} kHz',
                            style: const TextStyle(
                              color: AppColors.electricCyan,
                              fontSize: 12,
                              fontWeight: FontWeight.bold,
                            ),
                          ),
                      ],
                    ),
                    const SizedBox(height: 8),
                    SpectrumChart(
                      spectrumDb: report.spectrumDb,
                      cutoffHz: report.effectiveBandwidthHz,
                      sampleRate: report.facts.sampleRate,
                      cutoffSlopeDbOct: report.cutoffSlopeDbOct,
                      height: 160,
                    ),
                    const SizedBox(height: 20),

                    // Two Columns: Container Facts & Quality Metrics
                    Row(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        // Container Truth
                        Expanded(
                          child: Container(
                            padding: const EdgeInsets.all(14),
                            decoration: BoxDecoration(
                              color: AppColors.surfaceElevated,
                              borderRadius: BorderRadius.circular(10),
                              border: Border.all(color: AppColors.surfaceBorder),
                            ),
                            child: Column(
                              crossAxisAlignment: CrossAxisAlignment.start,
                              children: [
                                const Text(
                                  'VERDAD DEL CONTENEDOR',
                                  style: TextStyle(
                                    color: AppColors.electricCyan,
                                    fontSize: 11,
                                    fontWeight: FontWeight.bold,
                                  ),
                                ),
                                const SizedBox(height: 10),
                                _metaRow('Contenedor real', report.facts.container),
                                _metaRow('Códec decodificado', report.facts.codec),
                                _metaRow('Frecuencia de muestreo', '${report.facts.sampleRate} Hz'),
                                _metaRow('Profundidad de bits', '${report.facts.bitDepth ?? "N/A"} bits'),
                                _metaRow('Canales', '${report.facts.channels} (${report.facts.channels >= 2 ? "Estéreo" : "Mono"})'),
                                _metaRow('Duración', _formatDuration(report.facts.durationMs)),
                                _metaRow('Tamaño', _formatBytes(report.fileSize)),
                                _metaRow('Bitrate contenedor', '${report.facts.containerBitrateKbps ?? "N/A"} kbps'),
                              ],
                            ),
                          ),
                        ),
                        const SizedBox(width: 14),

                        // Quality Metrics
                        Expanded(
                          child: Container(
                            padding: const EdgeInsets.all(14),
                            decoration: BoxDecoration(
                              color: AppColors.surfaceElevated,
                              borderRadius: BorderRadius.circular(10),
                              border: Border.all(color: AppColors.surfaceBorder),
                            ),
                            child: Column(
                              crossAxisAlignment: CrossAxisAlignment.start,
                              children: [
                                const Text(
                                  'MÉTRICAS ACÚSTICAS (E12)',
                                  style: TextStyle(
                                    color: AppColors.electricCyan,
                                    fontSize: 11,
                                    fontWeight: FontWeight.bold,
                                  ),
                                ),
                                const SizedBox(height: 10),
                                _metaRow('True Peak', '${report.quality.truePeakDbtp?.toStringAsFixed(1) ?? "-"} dBTP'),
                                _metaRow('LUFS integrado', '${report.quality.lufsIntegrated?.toStringAsFixed(1) ?? "-"} LUFS'),
                                _metaRow('Muestras clipeadas', report.quality.clippedSamples.toString()),
                                _metaRow('DC Offset', report.quality.dcOffset?.toStringAsFixed(5) ?? '0.0'),
                                _metaRow('Rango dinámico', '${report.quality.dynamicRangeDb?.toStringAsFixed(1) ?? "-"} dB'),
                                _metaRow('Correlación estéreo', report.quality.stereoCorrelation?.toStringAsFixed(2) ?? '-'),
                                _metaRow('Pendiente corte', '${report.cutoffSlopeDbOct?.toStringAsFixed(1) ?? "-"} dB/oct'),
                              ],
                            ),
                          ),
                        ),
                      ],
                    ),
                    const SizedBox(height: 20),

                    // Evidences Catalog Section
                    const Row(
                      children: [
                        Icon(Icons.analytics_rounded, color: AppColors.electricCyan, size: 18),
                        SizedBox(width: 8),
                        Text(
                          'CATÁLOGO DE EVIDENCIAS TÉCNICAS (E01 - E14)',
                          style: TextStyle(
                            color: AppColors.textSecondary,
                            fontSize: 12,
                            fontWeight: FontWeight.bold,
                            letterSpacing: 0.8,
                          ),
                        ),
                      ],
                    ),
                    const SizedBox(height: 10),

                    // List of Evidences
                    ...report.evidences.map((ev) {
                      final isPositive = ev.llr > 0.5;
                      final isNegative = ev.llr < -0.5;
                      final evColor = isPositive
                          ? AppColors.verdictTranscode
                          : (isNegative ? AppColors.verdictVerified : AppColors.textDimmed);

                      return Container(
                        margin: const EdgeInsets.only(bottom: 6),
                        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
                        decoration: BoxDecoration(
                          color: AppColors.surfaceElevated.withOpacity(0.5),
                          borderRadius: BorderRadius.circular(8),
                          border: Border.all(color: AppColors.surfaceBorder.withOpacity(0.5)),
                        ),
                        child: Row(
                          children: [
                            Container(
                              padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
                              decoration: BoxDecoration(
                                color: AppColors.backgroundAbyssal,
                                borderRadius: BorderRadius.circular(4),
                                border: Border.all(color: evColor.withOpacity(0.5)),
                              ),
                              child: Text(
                                ev.code,
                                style: TextStyle(
                                  color: evColor,
                                  fontSize: 11,
                                  fontWeight: FontWeight.bold,
                                  fontFamily: 'Consolas',
                                ),
                              ),
                            ),
                            const SizedBox(width: 12),
                            Expanded(
                              child: Text(
                                ev.description,
                                style: const TextStyle(
                                  color: AppColors.textPrimary,
                                  fontSize: 12.5,
                                ),
                              ),
                            ),
                            if (ev.applicable && ev.llr != 0.0)
                              Text(
                                '${ev.llr > 0 ? "+" : ""}${ev.llr.toStringAsFixed(1)} LLR',
                                style: TextStyle(
                                  color: evColor,
                                  fontSize: 11,
                                  fontWeight: FontWeight.bold,
                                  fontFamily: 'Consolas',
                                ),
                              ),
                          ],
                        ),
                      );
                    }),

                    if (report.guardsTriggered.isNotEmpty) ...[
                      const SizedBox(height: 16),
                      Container(
                        padding: const EdgeInsets.all(12),
                        decoration: BoxDecoration(
                          color: AppColors.verdictInconclusive.withOpacity(0.1),
                          borderRadius: BorderRadius.circular(8),
                          border: Border.all(color: AppColors.verdictInconclusive.withOpacity(0.3)),
                        ),
                        child: Row(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            const Icon(Icons.shield_outlined, color: AppColors.verdictInconclusive, size: 20),
                            const SizedBox(width: 10),
                            Expanded(
                              child: Column(
                                crossAxisAlignment: CrossAxisAlignment.start,
                                children: [
                                  const Text(
                                    'SALVAGUARDAS CONTRA FALSOS POSITIVOS ACTIVADAS',
                                    style: TextStyle(
                                      color: AppColors.verdictInconclusive,
                                      fontSize: 11,
                                      fontWeight: FontWeight.bold,
                                    ),
                                  ),
                                  const SizedBox(height: 4),
                                  ...report.guardsTriggered.map(
                                    (g) => Text(
                                      '• $g',
                                      style: const TextStyle(color: AppColors.textSecondary, fontSize: 12),
                                    ),
                                  ),
                                ],
                              ),
                            ),
                          ],
                        ),
                      ),
                    ],
                  ],
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }

  Widget _metaRow(String label, String value) {
    return Padding(
      padding: const EdgeInsets.only(bottom: 6),
      child: Row(
        mainAxisAlignment: MainAxisAlignment.spaceBetween,
        children: [
          Text(
            label,
            style: const TextStyle(color: AppColors.textDimmed, fontSize: 12),
          ),
          Flexible(
            child: Text(
              value,
              style: const TextStyle(color: AppColors.textPrimary, fontSize: 12, fontWeight: FontWeight.w600),
              textAlign: TextAlign.end,
              overflow: TextOverflow.ellipsis,
            ),
          ),
        ],
      ),
    );
  }
}