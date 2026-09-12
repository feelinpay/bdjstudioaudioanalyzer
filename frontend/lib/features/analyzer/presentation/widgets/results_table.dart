import 'dart:io';
import 'package:flutter/material.dart';
import '../../../../core/ffi/api.dart';
import '../../../../core/theme/app_colors.dart';
import 'file_details_modal.dart';

class ResultsTable extends StatelessWidget {
  final List<FileReportFfi> reports;
  final Function(FileReportFfi)? onSelectReport;

  const ResultsTable({
    super.key,
    required this.reports,
    this.onSelectReport,
  });

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

  void _openFileFolder(String filePath) {
    if (Platform.isWindows) {
      Process.run('explorer.exe', ['/select,', filePath]);
    }
  }

  @override
  Widget build(BuildContext context) {
    if (reports.isEmpty) {
      return Center(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(
              Icons.audio_file_outlined,
              size: 48,
              color: AppColors.textDimmed.withOpacity(0.5),
            ),
            const SizedBox(height: 12),
            const Text(
              'No hay archivos en la lista para el filtro seleccionado',
              style: TextStyle(
                color: AppColors.textDimmed,
                fontSize: 14,
              ),
            ),
          ],
        ),
      );
    }

    return Container(
      decoration: BoxDecoration(
        color: AppColors.surfaceElevated,
        borderRadius: BorderRadius.circular(12),
        border: Border.all(color: AppColors.surfaceBorder),
      ),
      child: Column(
        children: [
          // Table Header
          Container(
            padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 10),
            decoration: const BoxDecoration(
              color: AppColors.surfaceCard,
              borderRadius: BorderRadius.vertical(top: Radius.circular(11)),
              border: Border(bottom: BorderSide(color: AppColors.surfaceBorder)),
            ),
            child: const Row(
              children: [
                Expanded(
                  flex: 5,
                  child: Text(
                    'ARCHIVO / RUTA',
                    style: TextStyle(color: AppColors.textDimmed, fontSize: 11, fontWeight: FontWeight.bold),
                  ),
                ),
                Expanded(
                  flex: 2,
                  child: Text(
                    'FORMATO REAL',
                    style: TextStyle(color: AppColors.textDimmed, fontSize: 11, fontWeight: FontWeight.bold),
                  ),
                ),
                Expanded(
                  flex: 2,
                  child: Text(
                    'ANCHO BANDA',
                    style: TextStyle(color: AppColors.textDimmed, fontSize: 11, fontWeight: FontWeight.bold),
                  ),
                ),
                Expanded(
                  flex: 3,
                  child: Text(
                    'VEREDICTO',
                    style: TextStyle(color: AppColors.textDimmed, fontSize: 11, fontWeight: FontWeight.bold),
                  ),
                ),
                SizedBox(
                  width: 80,
                  child: Text(
                    'ACCIONES',
                    style: TextStyle(color: AppColors.textDimmed, fontSize: 11, fontWeight: FontWeight.bold),
                    textAlign: TextAlign.center,
                  ),
                ),
              ],
            ),
          ),

          // Table Rows
          Expanded(
            child: ListView.separated(
              itemCount: reports.length,
              separatorBuilder: (_, __) => const Divider(
                height: 1,
                color: AppColors.surfaceBorder,
              ),
              itemBuilder: (context, index) {
                final r = reports[index];
                final fileName = r.path.split(RegExp(r'[/\\]')).last;
                final verdictColor = _getVerdictColor(r.verdictCode);

                return InkWell(
                  onTap: () {
                    if (onSelectReport != null) {
                      onSelectReport!(r);
                    } else {
                      showDialog(
                        context: context,
                        builder: (_) => FileDetailsModal(report: r),
                      );
                    }
                  },
                  hoverColor: AppColors.electricCyan.withOpacity(0.04),
                  child: Padding(
                    padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 10),
                    child: Row(
                      children: [
                        // File info
                        Expanded(
                          flex: 5,
                          child: Column(
                            crossAxisAlignment: CrossAxisAlignment.start,
                            children: [
                              Text(
                                fileName,
                                style: const TextStyle(
                                  color: AppColors.textPrimary,
                                  fontSize: 13,
                                  fontWeight: FontWeight.w600,
                                ),
                                maxLines: 1,
                                overflow: TextOverflow.ellipsis,
                              ),
                              const SizedBox(height: 2),
                              Text(
                                r.path,
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

                        // Real Format
                        Expanded(
                          flex: 2,
                          child: Column(
                            crossAxisAlignment: CrossAxisAlignment.start,
                            children: [
                              Text(
                                r.facts.container,
                                style: const TextStyle(
                                  color: AppColors.textPrimary,
                                  fontSize: 12,
                                  fontWeight: FontWeight.w500,
                                ),
                              ),
                              Text(
                                '${r.facts.sampleRate ~/ 1000} kHz / ${r.facts.bitDepth ?? "16"}b',
                                style: const TextStyle(
                                  color: AppColors.textDimmed,
                                  fontSize: 11,
                                ),
                              ),
                            ],
                          ),
                        ),

                        // Effective Bandwidth
                        Expanded(
                          flex: 2,
                          child: Text(
                            r.effectiveBandwidthHz != null
                                ? '${(r.effectiveBandwidthHz! / 1000).toStringAsFixed(1)} kHz'
                                : '-',
                            style: TextStyle(
                              color: (r.effectiveBandwidthHz ?? 0) < 18000
                                  ? AppColors.verdictTranscode
                                  : AppColors.textPrimary,
                              fontSize: 12.5,
                              fontWeight: FontWeight.bold,
                              fontFamily: 'Consolas',
                            ),
                          ),
                        ),

                        // Verdict Badge
                        Expanded(
                          flex: 3,
                          child: Row(
                            children: [
                              Container(
                                padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 3),
                                decoration: BoxDecoration(
                                  color: verdictColor.withOpacity(0.12),
                                  borderRadius: BorderRadius.circular(6),
                                  border: Border.all(color: verdictColor.withOpacity(0.5)),
                                ),
                                child: Text(
                                  r.verdictName,
                                  style: TextStyle(
                                    color: verdictColor,
                                    fontSize: 11.5,
                                    fontWeight: FontWeight.bold,
                                  ),
                                ),
                              ),
                              const SizedBox(width: 8),
                              Text(
                                '${(r.confidence * 100).toStringAsFixed(0)}%',
                                style: TextStyle(
                                  color: verdictColor.withOpacity(0.8),
                                  fontSize: 11,
                                  fontWeight: FontWeight.bold,
                                ),
                              ),
                            ],
                          ),
                        ),

                        // Actions
                        SizedBox(
                          width: 80,
                          child: Row(
                            mainAxisAlignment: MainAxisAlignment.center,
                            children: [
                              IconButton(
                                icon: const Icon(Icons.info_outline_rounded, size: 18),
                                color: AppColors.electricCyan,
                                tooltip: 'Inspeccionar ficha forense',
                                onPressed: () {
                                  showDialog(
                                    context: context,
                                    builder: (_) => FileDetailsModal(report: r),
                                  );
                                },
                              ),
                              IconButton(
                                icon: const Icon(Icons.folder_open_rounded, size: 18),
                                color: AppColors.textMuted,
                                tooltip: 'Abrir en explorador',
                                onPressed: () => _openFileFolder(r.path),
                              ),
                            ],
                          ),
                        ),
                      ],
                    ),
                  ),
                );
              },
            ),
          ),
        ],
      ),
    );
  }
}