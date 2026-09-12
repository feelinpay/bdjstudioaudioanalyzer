import 'dart:async';
import 'dart:io';
import 'package:desktop_drop/desktop_drop.dart';
import 'package:file_selector/file_selector.dart';
import 'package:flutter/material.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart';
import 'package:path_provider/path_provider.dart';
import '../../../../core/ffi/api.dart';
import '../../../../core/licensing/license_manager.dart';
import '../../../../core/theme/app_colors.dart';
import '../widgets/file_details_modal.dart';
import '../widgets/results_table.dart';

class HomeScreen extends StatefulWidget {
  final LicenseManager licenseManager;
  final VoidCallback onDeactivate;
  final VoidCallback? onRevokeLicense;

  const HomeScreen({
    super.key,
    required this.licenseManager,
    required this.onDeactivate,
    this.onRevokeLicense,
  });

  @override
  State<HomeScreen> createState() => _HomeScreenState();
}

class _HomeScreenState extends State<HomeScreen> {
  List<VolumeInfoFfi> _volumes = [];
  bool _isLoadingVolumes = false;

  final List<FileReportFfi> _allReports = [];
  bool _isAnalyzing = false;
  String _currentAnalyzingPath = '';
  int _analyzedCount = 0;
  int _totalToAnalyze = 0;
  PlatformInt64? _currentJobId;
  Timer? _pollTimer;
  bool _cancelRequested = false;

  bool _isDragging = false;
  String _selectedFilter = 'ALL';
  String _searchQuery = '';
  String _throttleMode = 'normal';

  @override
  void initState() {
    super.initState();
    _loadVolumes();
    _loadSavedReports();
  }

  @override
  void dispose() {
    _pollTimer?.cancel();
    super.dispose();
  }

  Future<void> _loadSavedReports() async {
    try {
      final saved = await querySavedReports(limit: 5000, offset: 0);
      if (mounted && saved.isNotEmpty) {
        setState(() {
          for (final r in saved) {
            if (!_allReports.any((x) => x.path == r.path)) {
              _allReports.add(r);
            }
          }
        });
      }
    } catch (e) {
      debugPrint('Error cargando reportes guardados: $e');
    }
  }

  Future<void> _loadVolumes() async {
    setState(() => _isLoadingVolumes = true);
    try {
      final vols = await listSystemVolumes();
      if (mounted) {
        setState(() {
          _volumes = vols;
          _isLoadingVolumes = false;
        });
      }
    } catch (e) {
      if (mounted) {
        setState(() => _isLoadingVolumes = false);
      }
    }
  }

  Future<void> _cancelCurrentScan() async {
    _cancelRequested = true;
    _pollTimer?.cancel();
    final jobId = _currentJobId;
    if (jobId != null) {
      try {
        await cancelScanJob(jobId: jobId);
      } catch (e) {
        debugPrint('Error cancelando escaneo: $e');
      }
    }
    if (mounted) {
      setState(() {
        _isAnalyzing = false;
        _currentAnalyzingPath = 'Escaneo cancelado';
        _currentJobId = null;
      });
    }
  }

  Future<void> _analyzeFiles(List<String> paths) async {
    if (paths.isEmpty || _isAnalyzing) return;

    setState(() {
      _isAnalyzing = true;
      _cancelRequested = false;
      _analyzedCount = 0;
      _totalToAnalyze = paths.length;
      _currentAnalyzingPath = 'Iniciando análisis...';
    });

    for (int i = 0; i < paths.length; i++) {
      if (!mounted || _cancelRequested) break;
      final p = paths[i];
      setState(() {
        _currentAnalyzingPath = p;
        _analyzedCount = i + 1;
      });

      try {
        final report = await analyzeFile(path: p);
        if (mounted && !_cancelRequested) {
          setState(() {
            final idx = _allReports.indexWhere((r) => r.path == report.path);
            if (idx >= 0) {
              _allReports[idx] = report;
            } else {
              _allReports.insert(0, report);
            }
          });
        }
      } catch (e) {
        debugPrint('Error analizando $p: $e');
      }
    }

    if (mounted) {
      setState(() {
        _isAnalyzing = false;
        _currentAnalyzingPath = '';
      });
    }
  }

  Future<void> _scanFolder(String folderPath) async {
    if (_isAnalyzing) return;

    _pollTimer?.cancel();
    setState(() {
      _isAnalyzing = true;
      _cancelRequested = false;
      _analyzedCount = 0;
      _totalToAnalyze = 0;
      _currentAnalyzingPath = 'Buscando archivos de audio en $folderPath...';
    });

    try {
      final jobId = await startScanJob(
        roots: [folderPath],
        throttleMode: _throttleMode,
        skipCache: false,
      );
      _currentJobId = jobId;

      _pollTimer = Timer.periodic(const Duration(milliseconds: 100), (timer) async {
        try {
          final status = await pollScanJob(jobId: jobId);
          if (!mounted) {
            timer.cancel();
            return;
          }

          setState(() {
            _totalToAnalyze = status.totalFound.toInt();
            _analyzedCount = status.analyzedCount.toInt();
            if (status.currentPath.isNotEmpty) {
              _currentAnalyzingPath = status.currentPath;
            }

            if (status.newReports.isNotEmpty) {
              for (final r in status.newReports) {
                final idx = _allReports.indexWhere((x) => x.path == r.path);
                if (idx >= 0) {
                  _allReports[idx] = r;
                } else {
                  _allReports.insert(0, r);
                }
              }
            }
          });

          if (status.isCompleted || !status.isActive) {
            timer.cancel();
            if (mounted) {
              setState(() {
                _isAnalyzing = false;
                _currentAnalyzingPath = '';
                _currentJobId = null;
              });
            }
          }
        } catch (e) {
          timer.cancel();
          if (mounted) {
            setState(() {
              _isAnalyzing = false;
              _currentAnalyzingPath = '';
              _currentJobId = null;
            });
            ScaffoldMessenger.of(context).showSnackBar(
              SnackBar(
                content: Text('Error en escaneo masivo: $e'),
                backgroundColor: AppColors.verdictTranscode,
              ),
            );
          }
        }
      });
    } catch (e) {
      if (mounted) {
        setState(() {
          _isAnalyzing = false;
          _currentAnalyzingPath = '';
          _currentJobId = null;
        });
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Text('Error al iniciar escaneo: $e'),
            backgroundColor: AppColors.verdictTranscode,
          ),
        );
      }
    }
  }

  Future<void> _pickAndAnalyzeFiles() async {
    const typeGroup = XTypeGroup(
      label: 'Archivos de audio',
      extensions: ['wav', 'flac', 'aif', 'aiff', 'mp3', 'm4a', 'aac', 'ogg', 'alac'],
    );
    final files = await openFiles(acceptedTypeGroups: [typeGroup]);
    if (files.isNotEmpty) {
      final paths = files.map((f) => f.path).toList();
      _analyzeFiles(paths);
    }
  }

  Future<void> _pickAndScanDirectory() async {
    final dirPath = await getDirectoryPath();
    if (dirPath != null && dirPath.isNotEmpty) {
      _scanFolder(dirPath);
    }
  }

  Future<void> _exportCsv() async {
    try {
      final docsDir = await getApplicationDocumentsDirectory();
      final outPath = '${docsDir.path}\\BDJ_Audio_Analyzer_Report_${DateTime.now().millisecondsSinceEpoch}.csv';
      final ok = await exportReportsCsv(outPath: outPath);
      if (ok && mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Text('Reporte CSV exportado con éxito en: $outPath'),
            backgroundColor: AppColors.verdictVerified,
            action: SnackBarAction(
              label: 'Abrir',
              textColor: Colors.white,
              onPressed: () {
                Process.run('explorer.exe', ['/select,', outPath]);
              },
            ),
          ),
        );
      }
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('Error exportando CSV: $e'), backgroundColor: AppColors.verdictTranscode),
        );
      }
    }
  }

  Future<void> _exportJson() async {
    try {
      final docsDir = await getApplicationDocumentsDirectory();
      final outPath = '${docsDir.path}\\BDJ_Audio_Analyzer_Report_${DateTime.now().millisecondsSinceEpoch}.json';
      final ok = await exportReportsJson(outPath: outPath);
      if (ok && mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Text('Reporte JSON exportado con éxito en: $outPath'),
            backgroundColor: AppColors.verdictVerified,
            action: SnackBarAction(
              label: 'Abrir',
              textColor: Colors.white,
              onPressed: () {
                Process.run('explorer.exe', ['/select,', outPath]);
              },
            ),
          ),
        );
      }
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('Error exportando JSON: $e'), backgroundColor: AppColors.verdictTranscode),
        );
      }
    }
  }

  List<FileReportFfi> get _filteredReports {
    return _allReports.where((r) {
      if (_selectedFilter != 'ALL' && r.verdictCode != _selectedFilter) {
        return false;
      }
      if (_searchQuery.isNotEmpty) {
        final q = _searchQuery.toLowerCase();
        final pathLower = r.path.toLowerCase();
        if (!pathLower.contains(q)) return false;
      }
      return true;
    }).toList();
  }

  int get _countLossless => _allReports.where((r) => r.verdictCode == 'LosslessVerified' || r.verdictCode == 'LikelyLossless').length;
  int get _countTranscode => _allReports.where((r) => r.verdictCode == 'ProbableTranscode').length;
  int get _countSuspicious => _allReports.where((r) => r.verdictCode == 'Suspicious').length;
  int get _countInconclusive => _allReports.where((r) => r.verdictCode == 'Inconclusive').length;
  int get _countDeclared => _allReports.where((r) => r.verdictCode == 'DeclaredLossy').length;

  @override
  Widget build(BuildContext context) {
    return DropTarget(
      onDragEntered: (_) => setState(() => _isDragging = true),
      onDragExited: (_) => setState(() => _isDragging = false),
      onDragDone: (details) {
        setState(() => _isDragging = false);
        final paths = details.files.map((f) => f.path).toList();
        if (paths.isNotEmpty) {
          if (FileSystemEntity.isDirectorySync(paths.first)) {
            _scanFolder(paths.first);
          } else {
            _analyzeFiles(paths);
          }
        }
      },
      child: Scaffold(
        backgroundColor: AppColors.backgroundAbyssal,
        body: Column(
          children: [
            _buildHeader(),
            if (_isAnalyzing) _buildScanningProgress(),
            Expanded(
              child: Row(
                children: [
                  _buildSidePanel(),
                  Expanded(
                    child: Padding(
                      padding: const EdgeInsets.all(18),
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.stretch,
                        children: [
                          if (_allReports.isEmpty) _buildBigDropZone(),
                          if (_allReports.isNotEmpty) ...[
                            _buildSummaryCards(),
                            const SizedBox(height: 14),
                            _buildFilterToolbar(),
                            const SizedBox(height: 12),
                            Expanded(
                              child: ResultsTable(
                                reports: _filteredReports,
                                onSelectReport: (r) {
                                  showDialog(
                                    context: context,
                                    builder: (_) => FileDetailsModal(report: r),
                                  );
                                },
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
          ],
        ),
      ),
    );
  }

  Widget _buildHeader() {
    return Container(
      height: 62,
      padding: const EdgeInsets.symmetric(horizontal: 20),
      decoration: const BoxDecoration(
        color: AppColors.surfaceElevated,
        border: Border(bottom: BorderSide(color: AppColors.surfaceBorder)),
      ),
      child: Row(
        children: [
          Image.file(
            File('logo.png'),
            width: 34,
            height: 34,
            errorBuilder: (_, __, ___) => const Icon(
              Icons.graphic_eq_rounded,
              color: AppColors.electricCyan,
              size: 32,
            ),
          ),
          const SizedBox(width: 14),
          Column(
            mainAxisAlignment: MainAxisAlignment.center,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Row(
                children: [
                  const Text(
                    'BDJ STUDIO',
                    style: TextStyle(
                      color: AppColors.textPrimary,
                      fontWeight: FontWeight.w900,
                      fontSize: 14.5,
                      letterSpacing: 1.1,
                    ),
                  ),
                  const SizedBox(width: 6),
                  const Text(
                    'AUDIO ANALYZER',
                    style: TextStyle(
                      color: AppColors.electricCyan,
                      fontWeight: FontWeight.w900,
                      fontSize: 14.5,
                      letterSpacing: 1.1,
                    ),
                  ),
                  const SizedBox(width: 8),
                  Container(
                    padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
                    decoration: BoxDecoration(
                      color: AppColors.electricCyan.withOpacity(0.15),
                      borderRadius: BorderRadius.circular(4),
                      border: Border.all(color: AppColors.electricCyan.withOpacity(0.3)),
                    ),
                    child: Text(
                      'v1.0 (Rev ${engineRevision()})',
                      style: const TextStyle(
                        color: AppColors.electricCyan,
                        fontSize: 10,
                        fontWeight: FontWeight.bold,
                      ),
                    ),
                  ),
                ],
              ),
              const SizedBox(height: 2),
              const Text(
                'Control de calidad forense para DJs · 100% Offline',
                style: TextStyle(color: AppColors.textDimmed, fontSize: 11),
              ),
            ],
          ),
          const Spacer(),
          Container(
            padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 5),
            decoration: BoxDecoration(
              color: AppColors.verdictVerified.withOpacity(0.12),
              borderRadius: BorderRadius.circular(20),
              border: Border.all(color: AppColors.verdictVerified.withOpacity(0.4)),
            ),
            child: const Row(
              children: [
                Icon(Icons.verified_rounded, color: AppColors.verdictVerified, size: 16),
                SizedBox(width: 6),
                Text(
                  'Licencia SPP3 Verificada (Offline)',
                  style: TextStyle(
                    color: AppColors.verdictVerified,
                    fontSize: 11.5,
                    fontWeight: FontWeight.bold,
                  ),
                ),
              ],
            ),
          ),
          const SizedBox(width: 12),
          IconButton(
            tooltip: 'Diagnóstico del motor',
            icon: const Icon(Icons.health_and_safety_outlined, color: AppColors.textSecondary, size: 20),
            onPressed: () async {
              final diag = await diagnostics();
              if (mounted) {
                showDialog(
                  context: context,
                  builder: (_) => AlertDialog(
                    backgroundColor: AppColors.surfaceModal,
                    title: const Text('Diagnóstico del Motor Nativo', style: TextStyle(color: AppColors.textPrimary)),
                    content: Text(diag, style: const TextStyle(color: AppColors.textSecondary, fontFamily: 'Consolas')),
                    actions: [
                      TextButton(onPressed: () => Navigator.pop(context), child: const Text('Cerrar')),
                    ],
                  ),
                );
              }
            },
          ),
        ],
      ),
    );
  }

  Widget _buildScanningProgress() {
    final pct = _totalToAnalyze > 0 ? (_analyzedCount / _totalToAnalyze).clamp(0.0, 1.0) : 0.0;
    final progressText = _totalToAnalyze > 0
        ? 'Analizando audio... $_analyzedCount de $_totalToAnalyze (${(pct * 100).toStringAsFixed(0)}%)'
        : (_analyzedCount > 0
            ? 'Analizando audio... $_analyzedCount pistas procesadas'
            : 'Descubriendo pistas de audio...');

    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 10),
      decoration: BoxDecoration(
        color: AppColors.electricCyan.withOpacity(0.08),
        border: const Border(bottom: BorderSide(color: AppColors.electricCyan, width: 1.5)),
      ),
      child: Row(
        children: [
          const SizedBox(
            width: 18,
            height: 18,
            child: CircularProgressIndicator(strokeWidth: 2.2, color: AppColors.electricCyan),
          ),
          const SizedBox(width: 14),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Row(
                  mainAxisAlignment: MainAxisAlignment.spaceBetween,
                  children: [
                    Text(
                      progressText,
                      style: const TextStyle(color: AppColors.textPrimary, fontSize: 12, fontWeight: FontWeight.bold),
                    ),
                    Flexible(
                      child: Text(
                        _currentAnalyzingPath.split(RegExp(r'[/\\]')).last,
                        style: const TextStyle(color: AppColors.electricCyan, fontSize: 11.5, fontFamily: 'Consolas'),
                        overflow: TextOverflow.ellipsis,
                      ),
                    ),
                  ],
                ),
                const SizedBox(height: 6),
                ClipRRect(
                  borderRadius: BorderRadius.circular(4),
                  child: LinearProgressIndicator(
                    value: _totalToAnalyze > 0 ? pct : null,
                    minHeight: 5,
                    backgroundColor: AppColors.surfaceBorder,
                    valueColor: const AlwaysStoppedAnimation(AppColors.electricCyan),
                  ),
                ),
              ],
            ),
          ),
          const SizedBox(width: 16),
          OutlinedButton(
            style: OutlinedButton.styleFrom(
              foregroundColor: AppColors.verdictTranscode,
              side: const BorderSide(color: AppColors.verdictTranscode),
              padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 6),
            ),
            onPressed: _cancelCurrentScan,
            child: const Text('Cancelar', style: TextStyle(fontSize: 12)),
          ),
        ],
      ),
    );
  }

  Widget _buildSidePanel() {
    return Container(
      width: 280,
      decoration: const BoxDecoration(
        color: AppColors.surfaceCard,
        border: Border(right: BorderSide(color: AppColors.surfaceBorder)),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Padding(
            padding: const EdgeInsets.fromLTRB(16, 16, 16, 10),
            child: Row(
              mainAxisAlignment: MainAxisAlignment.spaceBetween,
              children: [
                const Row(
                  children: [
                    Icon(Icons.storage_rounded, color: AppColors.electricCyan, size: 18),
                    SizedBox(width: 8),
                    Text(
                      'UNIDADES DEL SISTEMA',
                      style: TextStyle(color: AppColors.textSecondary, fontSize: 11, fontWeight: FontWeight.bold, letterSpacing: 0.8),
                    ),
                  ],
                ),
                IconButton(
                  icon: const Icon(Icons.refresh_rounded, size: 16, color: AppColors.textDimmed),
                  onPressed: _isLoadingVolumes ? null : _loadVolumes,
                  tooltip: 'Actualizar unidades',
                ),
              ],
            ),
          ),
          Expanded(
            child: _isLoadingVolumes
                ? const Center(child: CircularProgressIndicator(color: AppColors.electricCyan))
                : ListView.builder(
                    padding: const EdgeInsets.symmetric(horizontal: 12),
                    itemCount: _volumes.length,
                    itemBuilder: (context, index) {
                      final vol = _volumes[index];
                      final isUsb = vol.isRemovable;
                      final totalGb = (vol.totalBytes.toDouble() / (1024 * 1024 * 1024)).toStringAsFixed(1);
                      final freeGb = (vol.freeBytes.toDouble() / (1024 * 1024 * 1024)).toStringAsFixed(1);
                      final usedRatio = vol.totalBytes > BigInt.zero
                          ? ((vol.totalBytes - vol.freeBytes).toDouble() / vol.totalBytes.toDouble()).clamp(0.0, 1.0)
                          : 0.0;

                      return Container(
                        margin: const EdgeInsets.only(bottom: 8),
                        padding: const EdgeInsets.all(12),
                        decoration: BoxDecoration(
                          color: AppColors.surfaceElevated,
                          borderRadius: BorderRadius.circular(10),
                          border: Border.all(
                            color: isUsb ? AppColors.electricCyan.withOpacity(0.5) : AppColors.surfaceBorder,
                          ),
                        ),
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            Row(
                              children: [
                                Icon(
                                  isUsb ? Icons.usb_rounded : Icons.computer_rounded,
                                  color: isUsb ? AppColors.electricCyan : AppColors.textSecondary,
                                  size: 18,
                                ),
                                const SizedBox(width: 8),
                                Expanded(
                                  child: Text(
                                    '${vol.path} (${vol.label.isEmpty ? "Unidad" : vol.label})',
                                    style: const TextStyle(
                                      color: AppColors.textPrimary,
                                      fontWeight: FontWeight.bold,
                                      fontSize: 12.5,
                                    ),
                                    maxLines: 1,
                                    overflow: TextOverflow.ellipsis,
                                  ),
                                ),
                                if (isUsb)
                                  Container(
                                    padding: const EdgeInsets.symmetric(horizontal: 5, vertical: 1),
                                    decoration: BoxDecoration(
                                      color: AppColors.electricCyan.withOpacity(0.2),
                                      borderRadius: BorderRadius.circular(4),
                                    ),
                                    child: const Text(
                                      'USB',
                                      style: TextStyle(color: AppColors.electricCyan, fontSize: 9, fontWeight: FontWeight.bold),
                                    ),
                                  ),
                              ],
                            ),
                            const SizedBox(height: 6),
                            ClipRRect(
                              borderRadius: BorderRadius.circular(3),
                              child: LinearProgressIndicator(
                                value: usedRatio,
                                minHeight: 4,
                                backgroundColor: AppColors.surfaceBorder,
                                valueColor: AlwaysStoppedAnimation(
                                  isUsb ? AppColors.electricCyan : AppColors.textDimmed,
                                ),
                              ),
                            ),
                            const SizedBox(height: 6),
                            Row(
                              mainAxisAlignment: MainAxisAlignment.spaceBetween,
                              children: [
                                Text('$freeGb GB libres de $totalGb GB', style: const TextStyle(color: AppColors.textDimmed, fontSize: 10.5)),
                                InkWell(
                                  onTap: () => _scanFolder(vol.path),
                                  child: const Text(
                                    'Escanear',
                                    style: TextStyle(color: AppColors.electricCyan, fontSize: 11, fontWeight: FontWeight.bold),
                                  ),
                                ),
                              ],
                            ),
                          ],
                        ),
                      );
                    },
                  ),
          ),
          Padding(
            padding: const EdgeInsets.all(14),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                // Selector de intensidad de escaneo (N-10)
                Container(
                  padding: const EdgeInsets.all(8),
                  decoration: BoxDecoration(
                    color: AppColors.surfaceElevated,
                    borderRadius: BorderRadius.circular(8),
                    border: Border.all(color: AppColors.surfaceBorder),
                  ),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      const Text(
                        'INTENSIDAD DE CPU',
                        style: TextStyle(
                          color: AppColors.textDimmed,
                          fontSize: 9.5,
                          fontWeight: FontWeight.bold,
                          letterSpacing: 0.6,
                        ),
                      ),
                      const SizedBox(height: 6),
                      Row(
                        children: [
                          _buildThrottleOption('turbo', 'Turbo', Icons.bolt_rounded),
                          const SizedBox(width: 4),
                          _buildThrottleOption('normal', 'Normal', Icons.speed_rounded),
                          const SizedBox(width: 4),
                          _buildThrottleOption('silent', 'Silencioso', Icons.nightlight_round),
                        ],
                      ),
                    ],
                  ),
                ),
                const SizedBox(height: 10),
                ElevatedButton.icon(
                  icon: const Icon(Icons.folder_open_rounded, size: 18),
                  label: const Text('Escanear Carpeta'),
                  onPressed: _isAnalyzing ? null : _pickAndScanDirectory,
                  style: ElevatedButton.styleFrom(
                    backgroundColor: AppColors.surfaceElevated,
                    foregroundColor: AppColors.textPrimary,
                    side: const BorderSide(color: AppColors.surfaceBorder),
                    padding: const EdgeInsets.symmetric(vertical: 12),
                  ),
                ),
                const SizedBox(height: 8),
                ElevatedButton.icon(
                  icon: const Icon(Icons.audio_file_rounded, size: 18),
                  label: const Text('Seleccionar Archivos'),
                  onPressed: _isAnalyzing ? null : _pickAndAnalyzeFiles,
                  style: ElevatedButton.styleFrom(
                    backgroundColor: AppColors.electricCyan,
                    foregroundColor: AppColors.backgroundAbyssal,
                    padding: const EdgeInsets.symmetric(vertical: 12),
                    textStyle: const TextStyle(fontWeight: FontWeight.bold),
                  ),
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }

  Widget _buildThrottleOption(String mode, String label, IconData icon) {
    final isSelected = _throttleMode == mode;
    return Expanded(
      child: InkWell(
        onTap: _isAnalyzing ? null : () => setState(() => _throttleMode = mode),
        borderRadius: BorderRadius.circular(6),
        child: Container(
          padding: const EdgeInsets.symmetric(vertical: 6),
          decoration: BoxDecoration(
            color: isSelected ? AppColors.electricCyan.withOpacity(0.15) : Colors.transparent,
            borderRadius: BorderRadius.circular(6),
            border: Border.all(
              color: isSelected ? AppColors.electricCyan : AppColors.surfaceBorder.withOpacity(0.5),
            ),
          ),
          child: Column(
            children: [
              Icon(
                icon,
                size: 14,
                color: isSelected ? AppColors.electricCyan : AppColors.textDimmed,
              ),
              const SizedBox(height: 2),
              Text(
                label,
                style: TextStyle(
                  color: isSelected ? AppColors.electricCyan : AppColors.textDimmed,
                  fontSize: 10,
                  fontWeight: isSelected ? FontWeight.bold : FontWeight.normal,
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }

  Widget _buildBigDropZone() {
    return Expanded(
      child: Center(
        child: Container(
          constraints: const BoxConstraints(maxWidth: 620, maxHeight: 380),
          decoration: BoxDecoration(
            color: _isDragging ? AppColors.electricCyan.withOpacity(0.08) : AppColors.surfaceElevated,
            borderRadius: BorderRadius.circular(20),
            border: Border.all(
              color: _isDragging ? AppColors.electricCyan : AppColors.surfaceBorder,
              width: _isDragging ? 2.5 : 1.5,
            ),
          ),
          child: Column(
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              Container(
                width: 72,
                height: 72,
                decoration: BoxDecoration(
                  shape: BoxShape.circle,
                  color: AppColors.electricCyan.withOpacity(0.12),
                  border: Border.all(color: AppColors.electricCyan.withOpacity(0.4)),
                ),
                child: const Icon(Icons.cloud_upload_outlined, color: AppColors.electricCyan, size: 36),
              ),
              const SizedBox(height: 20),
              const Text(
                'Arrastra y suelta aquí tus archivos o carpetas de audio',
                style: TextStyle(
                  color: AppColors.textPrimary,
                  fontSize: 16,
                  fontWeight: FontWeight.bold,
                ),
              ),
              const SizedBox(height: 8),
              const Text(
                'Compatible con WAV, FLAC, AIFF, MP3, AAC, ALAC, OGG · Muestreo forense rápido',
                style: TextStyle(
                  color: AppColors.textDimmed,
                  fontSize: 12.5,
                ),
              ),
              const SizedBox(height: 24),
              Row(
                mainAxisSize: MainAxisSize.min,
                children: [
                  OutlinedButton.icon(
                    icon: const Icon(Icons.file_open_rounded, size: 18),
                    label: const Text('Elegir Archivos'),
                    onPressed: _pickAndAnalyzeFiles,
                    style: OutlinedButton.styleFrom(
                      foregroundColor: AppColors.electricCyan,
                      side: const BorderSide(color: AppColors.electricCyan),
                      padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 12),
                    ),
                  ),
                  const SizedBox(width: 14),
                  ElevatedButton.icon(
                    icon: const Icon(Icons.folder_shared_rounded, size: 18),
                    label: const Text('Elegir Carpeta'),
                    onPressed: _pickAndScanDirectory,
                    style: ElevatedButton.styleFrom(
                      backgroundColor: AppColors.electricCyan,
                      foregroundColor: AppColors.backgroundAbyssal,
                      padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 12),
                      textStyle: const TextStyle(fontWeight: FontWeight.bold),
                    ),
                  ),
                ],
              ),
            ],
          ),
        ),
      ),
    );
  }

  Widget _buildSummaryCards() {
    return Row(
      children: [
        _summaryCard('TOTAL ANALIZADOS', '${_allReports.length}', AppColors.textPrimary, Icons.analytics_outlined),
        const SizedBox(width: 10),
        _summaryCard('LOSSLESS GENUINOS', '$_countLossless', AppColors.verdictVerified, Icons.verified_rounded),
        const SizedBox(width: 10),
        _summaryCard('PROBABLE TRANSCODE', '$_countTranscode', AppColors.verdictTranscode, Icons.dangerous_rounded),
        const SizedBox(width: 10),
        _summaryCard('SOSPECHOSOS', '$_countSuspicious', AppColors.verdictSuspicious, Icons.warning_amber_rounded),
        const SizedBox(width: 10),
        _summaryCard('INCONCLUSOS', '$_countInconclusive', AppColors.verdictInconclusive, Icons.help_outline_rounded),
        const SizedBox(width: 10),
        _summaryCard('DECLARADO LOSSY', '$_countDeclared', AppColors.verdictDeclared, Icons.info_outline_rounded),
      ],
    );
  }

  Widget _summaryCard(String title, String value, Color color, IconData icon) {
    return Expanded(
      child: Container(
        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
        decoration: BoxDecoration(
          color: AppColors.surfaceElevated,
          borderRadius: BorderRadius.circular(10),
          border: Border.all(color: AppColors.surfaceBorder),
        ),
        child: Row(
          children: [
            Icon(icon, color: color, size: 20),
            const SizedBox(width: 10),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(title, style: const TextStyle(color: AppColors.textDimmed, fontSize: 9.5, fontWeight: FontWeight.bold)),
                  const SizedBox(height: 2),
                  Text(value, style: TextStyle(color: color, fontSize: 17, fontWeight: FontWeight.w900)),
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }

  Widget _buildFilterToolbar() {
    return Row(
      children: [
        _filterChip('ALL', 'Todos (${_allReports.length})'),
        const SizedBox(width: 6),
        _filterChip('LosslessVerified', 'Lossless ($countVerified)'),
        const SizedBox(width: 6),
        _filterChip('ProbableTranscode', 'Transcodes ($_countTranscode)'),
        const SizedBox(width: 6),
        _filterChip('Suspicious', 'Sospechosos ($_countSuspicious)'),
        const SizedBox(width: 6),
        _filterChip('Inconclusive', 'Inconcluso ($_countInconclusive)'),
        const SizedBox(width: 6),
        _filterChip('DeclaredLossy', 'Declarado ($_countDeclared)'),
        const SizedBox(width: 16),
        Expanded(
          child: Container(
            height: 36,
            padding: const EdgeInsets.symmetric(horizontal: 10),
            decoration: BoxDecoration(
              color: AppColors.surfaceElevated,
              borderRadius: BorderRadius.circular(8),
              border: Border.all(color: AppColors.surfaceBorder),
            ),
            child: Row(
              children: [
                const Icon(Icons.search_rounded, color: AppColors.textDimmed, size: 16),
                const SizedBox(width: 8),
                Expanded(
                  child: TextField(
                    onChanged: (val) => setState(() => _searchQuery = val),
                    style: const TextStyle(color: AppColors.textPrimary, fontSize: 12.5),
                    decoration: const InputDecoration(
                      hintText: 'Filtrar por nombre o ruta...',
                      hintStyle: TextStyle(color: AppColors.textDimmed, fontSize: 12),
                      border: InputBorder.none,
                      isDense: true,
                      contentPadding: EdgeInsets.zero,
                    ),
                  ),
                ),
                if (_searchQuery.isNotEmpty)
                  IconButton(
                    icon: const Icon(Icons.clear_rounded, size: 14, color: AppColors.textDimmed),
                    onPressed: () => setState(() => _searchQuery = ''),
                    padding: EdgeInsets.zero,
                    constraints: const BoxConstraints(),
                  ),
              ],
            ),
          ),
        ),
        const SizedBox(width: 14),
        IconButton(
          tooltip: 'Exportar reporte CSV',
          icon: const Icon(Icons.table_view_rounded, color: AppColors.electricCyan, size: 20),
          onPressed: _allReports.isEmpty ? null : _exportCsv,
        ),
        IconButton(
          tooltip: 'Exportar reporte JSON',
          icon: const Icon(Icons.code_rounded, color: AppColors.electricCyan, size: 20),
          onPressed: _allReports.isEmpty ? null : _exportJson,
        ),
        IconButton(
          tooltip: 'Limpiar lista',
          icon: const Icon(Icons.delete_outline_rounded, color: AppColors.textDimmed, size: 20),
          onPressed: _allReports.isEmpty
              ? null
              : () {
                  setState(() => _allReports.clear());
                },
        ),
      ],
    );
  }

  int get countVerified => _allReports.where((r) => r.verdictCode == 'LosslessVerified' || r.verdictCode == 'LikelyLossless').length;

  Widget _filterChip(String code, String label) {
    final isSelected = _selectedFilter == code;
    return InkWell(
      onTap: () => setState(() => _selectedFilter = code),
      borderRadius: BorderRadius.circular(6),
      child: Container(
        padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 6),
        decoration: BoxDecoration(
          color: isSelected ? AppColors.electricCyan.withOpacity(0.18) : AppColors.surfaceElevated,
          borderRadius: BorderRadius.circular(6),
          border: Border.all(
            color: isSelected ? AppColors.electricCyan : AppColors.surfaceBorder,
            width: isSelected ? 1.5 : 1.0,
          ),
        ),
        child: Text(
          label,
          style: TextStyle(
            color: isSelected ? AppColors.electricCyan : AppColors.textSecondary,
            fontSize: 11.5,
            fontWeight: isSelected ? FontWeight.bold : FontWeight.normal,
          ),
        ),
      ),
    );
  }
}
