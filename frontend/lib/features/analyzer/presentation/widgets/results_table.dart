import 'dart:io';
import 'package:flutter/material.dart';
import '../../../../core/ffi/api.dart';
import '../../../../core/theme/app_colors.dart';
import '../utils/friendly_verdict_helper.dart';
import 'file_details_modal.dart';

class ResultsTable extends StatelessWidget {
  final List<FileReportFfi> reports;
  final Function(FileReportFfi)? onSelectReport;
  final Set<String> selectedPaths;
  final Function(String path, bool selected)? onToggleSelect;
  final Function(bool selectAll)? onToggleSelectAll;
  final Function()? onSelectOnlyFakes;
  final Function(String path)? onDeleteSingle;

  const ResultsTable({
    super.key,
    required this.reports,
    this.onSelectReport,
    this.selectedPaths = const {},
    this.onToggleSelect,
    this.onToggleSelectAll,
    this.onSelectOnlyFakes,
    this.onDeleteSingle,
  });

  void _openFileFolder(String filePath) {
    if (Platform.isWindows) {
      Process.run('explorer.exe', ['/select,', filePath]);
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

    final allSelected = reports.isNotEmpty && selectedPaths.length == reports.length;
    final noneSelected = selectedPaths.isEmpty;
    final bool? headerCheckboxValue = allSelected
        ? true
        : (noneSelected ? false : null);

    return Container(
      decoration: BoxDecoration(
        color: AppColors.surfaceElevated,
        borderRadius: BorderRadius.circular(12),
        border: Border.all(color: AppColors.surfaceBorder),
      ),
      child: LayoutBuilder(
        builder: (context, constraints) {
          final tableWidth = constraints.maxWidth < 860 ? 860.0 : constraints.maxWidth;

          return SingleChildScrollView(
            scrollDirection: Axis.horizontal,
            child: SizedBox(
              width: tableWidth,
              height: constraints.maxHeight,
              child: Column(
                children: [
                  // Table Header
                  Container(
                    padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 8),
                    decoration: const BoxDecoration(
                      color: AppColors.surfaceCard,
                      borderRadius: BorderRadius.vertical(top: Radius.circular(11)),
                      border: Border(bottom: BorderSide(color: AppColors.surfaceBorder)),
                    ),
                    child: Row(
                      children: [
                        // Master Checkbox
                        SizedBox(
                          width: 28,
                          child: Checkbox(
                            value: headerCheckboxValue,
                            tristate: true,
                            activeColor: AppColors.electricCyan,
                            checkColor: Colors.black,
                            side: const BorderSide(color: AppColors.surfaceBorder, width: 1.5),
                            onChanged: (val) {
                              if (onToggleSelectAll != null) {
                                onToggleSelectAll!(val == true);
                              }
                            },
                          ),
                        ),
                        const SizedBox(width: 8),

                        // Col 1: Estado / Veracidad con botón Falsos integrado
                        Expanded(
                          flex: 3,
                          child: Row(
                            children: [
                              const Text(
                                'ESTADO / VERACIDAD',
                                style: TextStyle(color: AppColors.textDimmed, fontSize: 10, fontWeight: FontWeight.bold),
                              ),
                              if (onSelectOnlyFakes != null) ...[
                                const SizedBox(width: 6),
                                Tooltip(
                                  message: 'Marcar solo las pistas falsas o infladas',
                                  child: InkWell(
                                    onTap: onSelectOnlyFakes,
                                    borderRadius: BorderRadius.circular(4),
                                    child: Container(
                                      padding: const EdgeInsets.symmetric(horizontal: 5, vertical: 1.5),
                                      decoration: BoxDecoration(
                                        color: AppColors.verdictTranscode.withOpacity(0.15),
                                        borderRadius: BorderRadius.circular(4),
                                        border: Border.all(color: AppColors.verdictTranscode.withOpacity(0.4)),
                                      ),
                                      child: const Text(
                                        'FALSOS',
                                        style: TextStyle(
                                          color: AppColors.verdictTranscode,
                                          fontSize: 9,
                                          fontWeight: FontWeight.bold,
                                        ),
                                      ),
                                    ),
                                  ),
                                ),
                              ],
                            ],
                          ),
                        ),

                        // Col 2: Pista / Archivo
                        const Expanded(
                          flex: 4,
                          child: Text(
                            'PISTA / ARCHIVO',
                            style: TextStyle(color: AppColors.textDimmed, fontSize: 10, fontWeight: FontWeight.bold),
                          ),
                        ),

                        // Col 3: Calidad Declarada
                        const Expanded(
                          flex: 3,
                          child: Text(
                            'CALIDAD DECLARADA',
                            style: TextStyle(color: AppColors.textDimmed, fontSize: 10, fontWeight: FontWeight.bold),
                          ),
                        ),

                        // Col 4: Calidad Real Detectada
                        const Expanded(
                          flex: 3,
                          child: Text(
                            'CALIDAD REAL DETECTADA',
                            style: TextStyle(color: AppColors.textDimmed, fontSize: 10, fontWeight: FontWeight.bold),
                          ),
                        ),

                        // Col 5: Corte de Frecuencias
                        const Expanded(
                          flex: 2,
                          child: Text(
                            'CORTE FREC.',
                            style: TextStyle(color: AppColors.textDimmed, fontSize: 10, fontWeight: FontWeight.bold),
                          ),
                        ),

                        // Col 6: Acciones
                        const SizedBox(
                          width: 95,
                          child: Text(
                            'ACCIONES',
                            style: TextStyle(color: AppColors.textDimmed, fontSize: 10, fontWeight: FontWeight.bold),
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
                        final friendly = FriendlyVerdictHelper.fromCode(
                          r.verdictCode,
                          bandwidthHz: r.effectiveBandwidthHz?.toDouble(),
                        );
                        final declared = FriendlyVerdictHelper.getDeclaredQuality(r.facts);
                        final realQuality = FriendlyVerdictHelper.getDetectedRealQuality(
                          r.effectiveBandwidthHz,
                          r.facts,
                          r.verdictCode,
                        );
                        final cutoffStr = FriendlyVerdictHelper.getCutoffDisplay(r.effectiveBandwidthHz);
                        final badgeText = FriendlyVerdictHelper.getVeracityBadgeText(r);
                        final isSelected = selectedPaths.contains(r.path);

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
                          child: Container(
                            color: isSelected ? AppColors.electricCyan.withOpacity(0.07) : null,
                            padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 7),
                            child: Row(
                              children: [
                                // Checkbox de selección individual
                                SizedBox(
                                  width: 28,
                                  child: Checkbox(
                                    value: isSelected,
                                    activeColor: AppColors.electricCyan,
                                    checkColor: Colors.black,
                                    side: const BorderSide(color: AppColors.surfaceBorder, width: 1.5),
                                    onChanged: (val) {
                                      if (onToggleSelect != null) {
                                        onToggleSelect!(r.path, val ?? false);
                                      }
                                    },
                                  ),
                                ),
                                const SizedBox(width: 8),

                                // Col 1: Estado / Veracidad
                                Expanded(
                                  flex: 3,
                                  child: Align(
                                    alignment: Alignment.centerLeft,
                                    child: Container(
                                      padding: const EdgeInsets.symmetric(horizontal: 7, vertical: 3),
                                      decoration: BoxDecoration(
                                        color: friendly.backgroundColor,
                                        borderRadius: BorderRadius.circular(5),
                                        border: Border.all(color: friendly.color.withOpacity(0.55)),
                                      ),
                                      child: Row(
                                        mainAxisSize: MainAxisSize.min,
                                        children: [
                                          Icon(friendly.icon, color: friendly.color, size: 13),
                                          const SizedBox(width: 4),
                                          Flexible(
                                            child: Text(
                                              badgeText,
                                              style: TextStyle(
                                                color: friendly.color,
                                                fontSize: 10.5,
                                                fontWeight: FontWeight.bold,
                                              ),
                                              maxLines: 1,
                                              overflow: TextOverflow.ellipsis,
                                            ),
                                          ),
                                        ],
                                      ),
                                    ),
                                  ),
                                ),

                                // Col 2: Pista / Archivo
                                Expanded(
                                  flex: 4,
                                  child: Column(
                                    crossAxisAlignment: CrossAxisAlignment.start,
                                    mainAxisSize: MainAxisSize.min,
                                    children: [
                                      Text(
                                        fileName,
                                        style: const TextStyle(
                                          color: AppColors.textPrimary,
                                          fontSize: 12.5,
                                          fontWeight: FontWeight.w600,
                                        ),
                                        maxLines: 1,
                                        overflow: TextOverflow.ellipsis,
                                      ),
                                      const SizedBox(height: 1.5),
                                      Text(
                                        '${_formatDuration(r.facts.durationMs)} · ${_formatBytes(r.fileSize)} · ${r.path}',
                                        style: const TextStyle(
                                          color: AppColors.textDimmed,
                                          fontSize: 10,
                                        ),
                                        maxLines: 1,
                                        overflow: TextOverflow.ellipsis,
                                      ),
                                    ],
                                  ),
                                ),

                                // Col 3: Calidad Declarada
                                Expanded(
                                  flex: 3,
                                  child: Text(
                                    declared,
                                    style: const TextStyle(
                                      color: AppColors.textSecondary,
                                      fontSize: 11,
                                      fontWeight: FontWeight.w500,
                                    ),
                                    maxLines: 1,
                                    overflow: TextOverflow.ellipsis,
                                  ),
                                ),

                                // Col 4: Calidad Real Detectada
                                Expanded(
                                  flex: 3,
                                  child: Text(
                                    realQuality,
                                    style: TextStyle(
                                      color: friendly.isDanger
                                          ? AppColors.verdictTranscode
                                          : (friendly.isSafe ? AppColors.verdictVerified : AppColors.textPrimary),
                                      fontSize: 11,
                                      fontWeight: FontWeight.bold,
                                    ),
                                    maxLines: 1,
                                    overflow: TextOverflow.ellipsis,
                                  ),
                                ),

                                // Col 5: Corte de Frecuencias
                                Expanded(
                                  flex: 2,
                                  child: Text(
                                    cutoffStr,
                                    style: TextStyle(
                                      color: (r.effectiveBandwidthHz ?? 0) < 18000
                                          ? AppColors.verdictTranscode
                                          : AppColors.electricCyan,
                                      fontSize: 11,
                                      fontFamily: 'Consolas',
                                      fontWeight: FontWeight.bold,
                                    ),
                                    maxLines: 1,
                                    overflow: TextOverflow.ellipsis,
                                  ),
                                ),

                                // Col 6: Acciones (Compacto para evitar desbordamiento)
                                SizedBox(
                                  width: 95,
                                  child: Row(
                                    mainAxisAlignment: MainAxisAlignment.center,
                                    children: [
                                      IconButton(
                                        icon: const Icon(Icons.analytics_rounded, size: 16),
                                        color: AppColors.electricCyan,
                                        tooltip: 'Ver análisis espectral',
                                        padding: EdgeInsets.zero,
                                        constraints: const BoxConstraints(minWidth: 26, minHeight: 26),
                                        onPressed: () {
                                          showDialog(
                                            context: context,
                                            builder: (_) => FileDetailsModal(report: r),
                                          );
                                        },
                                      ),
                                      const SizedBox(width: 4),
                                      IconButton(
                                        icon: const Icon(Icons.folder_open_rounded, size: 16),
                                        color: AppColors.textMuted,
                                        tooltip: 'Abrir en Windows',
                                        padding: EdgeInsets.zero,
                                        constraints: const BoxConstraints(minWidth: 26, minHeight: 26),
                                        onPressed: () => _openFileFolder(r.path),
                                      ),
                                      const SizedBox(width: 4),
                                      IconButton(
                                        icon: const Icon(Icons.delete_outline_rounded, size: 16),
                                        color: AppColors.verdictTranscode.withOpacity(0.85),
                                        tooltip: 'Eliminar del disco',
                                        padding: EdgeInsets.zero,
                                        constraints: const BoxConstraints(minWidth: 26, minHeight: 26),
                                        onPressed: () {
                                          if (onDeleteSingle != null) {
                                            onDeleteSingle!(r.path);
                                          }
                                        },
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
            ),
          );
        },
      ),
    );
  }
}