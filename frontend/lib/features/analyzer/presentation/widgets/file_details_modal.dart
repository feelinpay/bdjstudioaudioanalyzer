import 'dart:io';
import 'package:flutter/material.dart';
import '../../../../core/ffi/api.dart';
import '../../../../core/theme/app_colors.dart';
import '../utils/friendly_verdict_helper.dart';
import 'spectrum_chart.dart';

/// Modal de detalles de archivo en MODO ÚNICO UNIFICADO.
/// Presenta en una sola vista fluida y completa:
/// 1. Veredicto humano directo (falso vs real, confianza).
/// 2. Comparativa inmediata (Calidad declarada vs Calidad real detectada).
/// 3. Espectro dinámico interactivo con línea de corte visual.
/// 4. Metadatos del contenedor y métricas acústicas.
/// 5. Evidencias forenses y salvaguardas.
class FileDetailsModal extends StatelessWidget {
  final FileReportFfi report;

  const FileDetailsModal({super.key, required this.report});

  void _openFileFolder(String filePath) {
    if (Platform.isWindows) {
      Process.run('explorer.exe', ['/select,', filePath]);
    } else if (Platform.isMacOS) {
      Process.run('open', ['-R', filePath]);
    } else if (Platform.isLinux) {
      Process.run('xdg-open', [File(filePath).parent.path]);
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

  String _formatCutoffKind(CutoffKindFfi? kind) {
    if (kind == null) return '-';
    switch (kind) {
      case CutoffKindFfi.brickwallCutoff:
        return 'Corte digital abrupto (Brickwall)';
      case CutoffKindFfi.fullSpectrum:
        return 'Espectro completo sin corte';
      case CutoffKindFfi.naturalRolloff:
        return 'Atenuación acústica natural (Roll-off)';
    }
  }

  @override
  Widget build(BuildContext context) {
    final friendly = FriendlyVerdictHelper.fromCode(
      report.verdictCode,
      bandwidthHz: report.effectiveBandwidthHz?.toDouble(),
    );
    final fileName = report.path.split(RegExp(r'[/\\]')).last;

    return Dialog(
      backgroundColor: AppColors.surfaceModal,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(16),
        side: const BorderSide(color: AppColors.surfaceBorder, width: 1.5),
      ),
      insetPadding: const EdgeInsets.symmetric(horizontal: 32, vertical: 20),
      child: Container(
        width: 920,
        height: 720,
        padding: const EdgeInsets.all(20),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            // Cabecera Principal
            Row(
              children: [
                Container(
                  padding: const EdgeInsets.all(8),
                  decoration: BoxDecoration(
                    color: friendly.color.withOpacity(0.15),
                    borderRadius: BorderRadius.circular(10),
                    border: Border.all(color: friendly.color.withOpacity(0.4)),
                  ),
                  child: Icon(friendly.icon, color: friendly.color, size: 24),
                ),
                const SizedBox(width: 12),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        fileName,
                        style: const TextStyle(
                          color: AppColors.textPrimary,
                          fontSize: 16,
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
                          fontSize: 11,
                        ),
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                      ),
                    ],
                  ),
                ),
                IconButton(
                  tooltip: 'Abrir ubicación del archivo',
                  icon: const Icon(Icons.folder_open_rounded, color: AppColors.electricCyan, size: 20),
                  onPressed: () => _openFileFolder(report.path),
                ),
                IconButton(
                  tooltip: 'Cerrar',
                  onPressed: () => Navigator.of(context).pop(),
                  icon: const Icon(Icons.close_rounded, color: AppColors.textMuted),
                ),
              ],
            ),
            const SizedBox(height: 12),

            // Contenido Único Integrado
            Expanded(
              child: SingleChildScrollView(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    // Banner de Veredicto
                    Container(
                      padding: const EdgeInsets.all(16),
                      decoration: BoxDecoration(
                        color: friendly.backgroundColor,
                        borderRadius: BorderRadius.circular(12),
                        border: Border.all(color: friendly.color.withOpacity(0.4), width: 1.5),
                      ),
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Wrap(
                            alignment: WrapAlignment.spaceBetween,
                            crossAxisAlignment: WrapCrossAlignment.center,
                            spacing: 10,
                            runSpacing: 6,
                            children: [
                              Row(
                                mainAxisSize: MainAxisSize.min,
                                children: [
                                  Icon(friendly.icon, color: friendly.color, size: 24),
                                  const SizedBox(width: 10),
                                  Text(
                                    friendly.title,
                                    style: TextStyle(
                                      color: friendly.color,
                                      fontSize: 16.5,
                                      fontWeight: FontWeight.w900,
                                    ),
                                  ),
                                ],
                              ),
                              Container(
                                padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 4),
                                decoration: BoxDecoration(
                                  color: AppColors.backgroundAbyssal,
                                  borderRadius: BorderRadius.circular(6),
                                  border: Border.all(color: friendly.color.withOpacity(0.5)),
                                ),
                                child: Text(
                                  'Confianza: ${(report.confidence * 100).toStringAsFixed(0)}% · LLR: ${report.scoreLlr.toStringAsFixed(1)}',
                                  style: TextStyle(
                                    color: friendly.color,
                                    fontSize: 11,
                                    fontWeight: FontWeight.bold,
                                    fontFamily: 'Consolas',
                                  ),
                                ),
                              ),
                            ],
                          ),
                          const SizedBox(height: 10),
                          Text(
                            friendly.explanation,
                            style: const TextStyle(
                              color: AppColors.textPrimary,
                              fontSize: 13,
                              height: 1.4,
                            ),
                          ),
                          const SizedBox(height: 12),
                          Container(
                            padding: const EdgeInsets.all(10),
                            decoration: BoxDecoration(
                              color: AppColors.backgroundAbyssal.withOpacity(0.5),
                              borderRadius: BorderRadius.circular(8),
                              border: Border.all(color: friendly.color.withOpacity(0.3)),
                            ),
                            child: Row(
                              crossAxisAlignment: CrossAxisAlignment.start,
                              children: [
                                Icon(
                                  friendly.isDanger
                                      ? Icons.gpp_bad_rounded
                                      : (friendly.isSafe ? Icons.gpp_good_rounded : Icons.info_outline_rounded),
                                  color: friendly.color,
                                  size: 18,
                                ),
                                const SizedBox(width: 8),
                                Expanded(
                                  child: Text(
                                    friendly.recommendation,
                                    style: const TextStyle(
                                      color: AppColors.textSecondary,
                                      fontSize: 12,
                                      height: 1.35,
                                    ),
                                  ),
                                ),
                              ],
                            ),
                          ),
                        ],
                      ),
                    ),
                    const SizedBox(height: 14),

                    // Trío de Calidad (Declarada vs Real vs Formato)
                    Row(
                      children: [
                        Expanded(
                          child: _buildInfoTile(
                            icon: Icons.label_important_outline_rounded,
                            label: 'CALIDAD DECLARADA',
                            value: FriendlyVerdictHelper.getDeclaredQuality(report.facts),
                            subtext: report.facts.isLosslessDeclared
                                ? 'Empaquetado como máster'
                                : 'Empaquetado como comprimido',
                          ),
                        ),
                        const SizedBox(width: 10),
                        Expanded(
                          child: _buildInfoTile(
                            icon: Icons.equalizer_rounded,
                            label: 'CALIDAD REAL DETECTADA',
                            value: FriendlyVerdictHelper.getDetectedRealQuality(
                              report.effectiveBandwidthHz,
                              report.facts,
                              report.verdictCode,
                            ),
                            subtext: 'Corte: ${FriendlyVerdictHelper.getCutoffDisplay(report.effectiveBandwidthHz)}',
                            accentColor: friendly.color,
                          ),
                        ),
                        const SizedBox(width: 10),
                        Expanded(
                          child: _buildInfoTile(
                            icon: Icons.access_time_rounded,
                            label: 'FORMATO Y MUESTREO',
                            value: '${report.facts.sampleRate} Hz · ${_formatDuration(report.facts.durationMs)}',
                            subtext: '${_formatBytes(report.fileSize)} · ${report.facts.channels >= 2 ? "Estéreo" : "Mono"}',
                          ),
                        ),
                      ],
                    ),
                    const SizedBox(height: 16),

                    // Sección de Espectro Integrado (Siempre visible en un solo modo)
                    Container(
                      padding: const EdgeInsets.all(14),
                      decoration: BoxDecoration(
                        color: AppColors.surfaceElevated,
                        borderRadius: BorderRadius.circular(12),
                        border: Border.all(color: AppColors.surfaceBorder),
                      ),
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Row(
                            children: [
                              const Icon(Icons.graphic_eq_rounded, color: AppColors.electricCyan, size: 18),
                              const SizedBox(width: 8),
                              const Expanded(
                                child: Text(
                                  'ESPECTRO DE FRECUENCIAS Y CORTE DETECTADO',
                                  style: TextStyle(
                                    color: AppColors.textPrimary,
                                    fontSize: 12,
                                    fontWeight: FontWeight.bold,
                                    letterSpacing: 0.6,
                                  ),
                                  overflow: TextOverflow.ellipsis,
                                ),
                              ),
                              const SizedBox(width: 8),
                              if (report.effectiveBandwidthHz != null)
                                Container(
                                  padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 3),
                                  decoration: BoxDecoration(
                                    color: friendly.color.withOpacity(0.15),
                                    borderRadius: BorderRadius.circular(6),
                                    border: Border.all(color: friendly.color.withOpacity(0.4)),
                                  ),
                                  child: Text(
                                    'Límite de audio: ${(report.effectiveBandwidthHz! / 1000).toStringAsFixed(1)} kHz',
                                    style: TextStyle(
                                      color: friendly.color,
                                      fontSize: 11.5,
                                      fontWeight: FontWeight.bold,
                                    ),
                                  ),
                                ),
                            ],
                          ),
                          const SizedBox(height: 10),
                          SpectrumChart(
                            spectrumDb: report.spectrumDb,
                            cutoffHz: report.effectiveBandwidthHz,
                            sampleRate: report.facts.sampleRate,
                            cutoffSlopeDbOct: report.cutoffSlopeDbOct,
                            cutoffKind: report.cutoffKind,
                            height: 170,
                          ),
                          const SizedBox(height: 8),
                          Wrap(
                            alignment: WrapAlignment.spaceBetween,
                            crossAxisAlignment: WrapCrossAlignment.center,
                            spacing: 12,
                            runSpacing: 4,
                            children: [
                              Text(
                                'Naturaleza: ${_formatCutoffKind(report.cutoffKind)}',
                                style: const TextStyle(color: AppColors.textDimmed, fontSize: 11),
                              ),
                              if (report.cutoffSlopeDbOct != null)
                                Text(
                                  'Pendiente de caída: ${report.cutoffSlopeDbOct!.toStringAsFixed(1)} dB/oct',
                                  style: const TextStyle(color: AppColors.textDimmed, fontSize: 11),
                                ),
                            ],
                          ),
                        ],
                      ),
                    ),
                    const SizedBox(height: 14),

                    // Dos Columnas: Metadatos y Métricas Acústicas
                    Row(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        // Metadatos
                        Expanded(
                          child: Container(
                            padding: const EdgeInsets.all(12),
                            decoration: BoxDecoration(
                              color: AppColors.surfaceElevated,
                              borderRadius: BorderRadius.circular(10),
                              border: Border.all(color: AppColors.surfaceBorder),
                            ),
                            child: Column(
                              crossAxisAlignment: CrossAxisAlignment.start,
                              children: [
                                const Text(
                                  'METADATOS DEL ARCHIVO',
                                  style: TextStyle(
                                    color: AppColors.electricCyan,
                                    fontSize: 11,
                                    fontWeight: FontWeight.bold,
                                    letterSpacing: 0.5,
                                  ),
                                ),
                                const SizedBox(height: 8),
                                _metaRow('Contenedor real', report.facts.container),
                                _metaRow('Códec decodificado', report.facts.codec),
                                _metaRow('Frecuencia de muestreo', '${report.facts.sampleRate} Hz'),
                                _metaRow('Profundidad de bits', '${report.facts.bitDepth ?? "N/A"} bits'),
                                _metaRow('Canales', '${report.facts.channels} (${report.facts.channels >= 2 ? "Estéreo" : "Mono"})'),
                                _metaRow('Bitrate contenedor', '${report.facts.containerBitrateKbps ?? "N/A"} kbps'),
                              ],
                            ),
                          ),
                        ),
                        const SizedBox(width: 10),

                        // Métricas Acústicas
                        Expanded(
                          child: Container(
                            padding: const EdgeInsets.all(12),
                            decoration: BoxDecoration(
                              color: AppColors.surfaceElevated,
                              borderRadius: BorderRadius.circular(10),
                              border: Border.all(color: AppColors.surfaceBorder),
                            ),
                            child: Column(
                              crossAxisAlignment: CrossAxisAlignment.start,
                              children: [
                                const Text(
                                  'MÉTRICAS ACÚSTICAS Y DINÁMICAS',
                                  style: TextStyle(
                                    color: AppColors.electricCyan,
                                    fontSize: 11,
                                    fontWeight: FontWeight.bold,
                                    letterSpacing: 0.5,
                                  ),
                                ),
                                const SizedBox(height: 8),
                                _metaRow('True Peak', '${report.quality.truePeakDbtp?.toStringAsFixed(1) ?? "-"} dBTP'),
                                _metaRow('LUFS integrado', '${report.quality.lufsIntegrated?.toStringAsFixed(1) ?? "-"} LUFS'),
                                _metaRow('Muestras clipeadas', report.quality.clippedSamples.toString()),
                                _metaRow('Rango dinámico', '${report.quality.dynamicRangeDb?.toStringAsFixed(1) ?? "-"} dB'),
                                _metaRow('Correlación estéreo', report.quality.stereoCorrelation?.toStringAsFixed(2) ?? '-'),
                                _metaRow('DC Offset', report.quality.dcOffset?.toStringAsFixed(5) ?? '0.0'),
                              ],
                            ),
                          ),
                        ),
                      ],
                    ),

                    // Evidencias Forenses (si existen)
                    if (report.evidences.isNotEmpty) ...[
                      const SizedBox(height: 14),
                      Container(
                        padding: const EdgeInsets.all(12),
                        decoration: BoxDecoration(
                          color: AppColors.surfaceElevated,
                          borderRadius: BorderRadius.circular(10),
                          border: Border.all(color: AppColors.surfaceBorder),
                        ),
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            const Text(
                              'EVIDENCIAS FORENSES DETECTADAS',
                              style: TextStyle(
                                color: AppColors.textSecondary,
                                fontSize: 11,
                                fontWeight: FontWeight.bold,
                                letterSpacing: 0.5,
                              ),
                            ),
                            const SizedBox(height: 8),
                            ...report.evidences.map((ev) {
                              final isPositive = ev.llr > 0.5;
                              final isNegative = ev.llr < -0.5;
                              final evColor = isPositive
                                  ? AppColors.verdictTranscode
                                  : (isNegative ? AppColors.verdictVerified : AppColors.textDimmed);

                              return Padding(
                                padding: const EdgeInsets.only(bottom: 6),
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
                                          fontSize: 10.5,
                                          fontWeight: FontWeight.bold,
                                          fontFamily: 'Consolas',
                                        ),
                                      ),
                                    ),
                                    const SizedBox(width: 8),
                                    Expanded(
                                      child: Text(
                                        ev.description,
                                        style: const TextStyle(color: AppColors.textPrimary, fontSize: 11.5),
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
                          ],
                        ),
                      ),
                    ],

                    // Salvaguardas (si existen)
                    if (report.guardsTriggered.isNotEmpty) ...[
                      const SizedBox(height: 10),
                      Container(
                        padding: const EdgeInsets.all(10),
                        decoration: BoxDecoration(
                          color: AppColors.verdictInconclusive.withOpacity(0.1),
                          borderRadius: BorderRadius.circular(8),
                          border: Border.all(color: AppColors.verdictInconclusive.withOpacity(0.3)),
                        ),
                        child: Row(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            const Icon(Icons.shield_outlined, color: AppColors.verdictInconclusive, size: 18),
                            const SizedBox(width: 8),
                            Expanded(
                              child: Column(
                                crossAxisAlignment: CrossAxisAlignment.start,
                                children: [
                                  const Text(
                                    'SALVAGUARDAS CONTRA FALSOS POSITIVOS ACTIVADAS',
                                    style: TextStyle(
                                      color: AppColors.verdictInconclusive,
                                      fontSize: 10.5,
                                      fontWeight: FontWeight.bold,
                                    ),
                                  ),
                                  const SizedBox(height: 3),
                                  ...report.guardsTriggered.map(
                                    (g) => Text('• $g', style: const TextStyle(color: AppColors.textSecondary, fontSize: 11.5)),
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
            const SizedBox(height: 12),

            // Barra Inferior de Acciones
            Row(
              mainAxisAlignment: MainAxisAlignment.end,
              children: [
                OutlinedButton.icon(
                  icon: const Icon(Icons.folder_open_rounded, size: 16),
                  label: const Text('Abrir Carpeta'),
                  onPressed: () => _openFileFolder(report.path),
                  style: OutlinedButton.styleFrom(
                    foregroundColor: AppColors.textPrimary,
                    side: const BorderSide(color: AppColors.surfaceBorder),
                    padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 10),
                  ),
                ),
                const SizedBox(width: 10),
                ElevatedButton(
                  onPressed: () => Navigator.of(context).pop(),
                  style: ElevatedButton.styleFrom(
                    backgroundColor: AppColors.electricCyan,
                    foregroundColor: AppColors.backgroundAbyssal,
                    padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 10),
                    textStyle: const TextStyle(fontWeight: FontWeight.bold),
                  ),
                  child: const Text('Entendido / Cerrar'),
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }

  Widget _buildInfoTile({
    required IconData icon,
    required String label,
    required String value,
    required String subtext,
    Color? accentColor,
  }) {
    return Container(
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: AppColors.surfaceElevated,
        borderRadius: BorderRadius.circular(10),
        border: Border.all(color: accentColor?.withOpacity(0.5) ?? AppColors.surfaceBorder),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Icon(icon, color: accentColor ?? AppColors.electricCyan, size: 16),
              const SizedBox(width: 6),
              Text(
                label,
                style: const TextStyle(color: AppColors.textDimmed, fontSize: 10.5, fontWeight: FontWeight.bold),
              ),
            ],
          ),
          const SizedBox(height: 6),
          Text(
            value,
            style: TextStyle(
              color: accentColor ?? AppColors.textPrimary,
              fontSize: 14,
              fontWeight: FontWeight.bold,
            ),
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
          ),
          const SizedBox(height: 2),
          Text(
            subtext,
            style: const TextStyle(color: AppColors.textDimmed, fontSize: 10.5),
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
          ),
        ],
      ),
    );
  }

  Widget _metaRow(String label, String value) {
    return Padding(
      padding: const EdgeInsets.only(bottom: 5),
      child: Row(
        mainAxisAlignment: MainAxisAlignment.spaceBetween,
        children: [
          Text(
            label,
            style: const TextStyle(color: AppColors.textDimmed, fontSize: 11.5),
          ),
          Flexible(
            child: Text(
              value,
              style: const TextStyle(color: AppColors.textPrimary, fontSize: 11.5, fontWeight: FontWeight.w600),
              textAlign: TextAlign.end,
              overflow: TextOverflow.ellipsis,
            ),
          ),
        ],
      ),
    );
  }
}