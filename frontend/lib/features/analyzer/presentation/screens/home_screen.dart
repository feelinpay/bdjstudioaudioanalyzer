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
import '../utils/friendly_verdict_helper.dart';

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
  final Set<String> _selectedPaths = {};

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

  void _toggleSelect(String path, bool selected) {
    setState(() {
      if (selected) {
        _selectedPaths.add(path);
      } else {
        _selectedPaths.remove(path);
      }
    });
  }

  void _toggleSelectAll(bool selectAll) {
    setState(() {
      if (selectAll) {
        _selectedPaths.addAll(_filteredReports.map((r) => r.path));
      } else {
        _selectedPaths.clear();
      }
    });
  }

  void _selectOnlyFakes() {
    setState(() {
      _selectedPaths.clear();
      for (final r in _allReports) {
        if (r.verdictCode == 'ProbableTranscode') {
          _selectedPaths.add(r.path);
        }
      }
    });
    if (_selectedPaths.isEmpty && mounted) {
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(
          content: Text('No hay archivos falsificados o inflados en la lista actual.'),
          backgroundColor: AppColors.verdictVerified,
          duration: Duration(seconds: 2),
        ),
      );
    }
  }

  Future<void> _confirmDeleteSingle(String path) async {
    final fileName = path.split(RegExp(r'[/\\]')).last;
    final confirm = await showDialog<bool>(
      context: context,
      builder: (ctx) => AlertDialog(
        backgroundColor: AppColors.surfaceElevated,
        title: const Row(
          children: [
            Icon(Icons.warning_amber_rounded, color: AppColors.verdictTranscode, size: 24),
            SizedBox(width: 8),
            Text('¿Eliminar archivo del disco?', style: TextStyle(color: Colors.white, fontSize: 16)),
          ],
        ),
        content: Text(
          'Se eliminará permanentemente del almacenamiento:\n\n$fileName\n\nRuta: $path\n\n¿Estás seguro de continuar?',
          style: const TextStyle(color: AppColors.textSecondary, fontSize: 13),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(ctx).pop(false),
            child: const Text('Cancelar', style: TextStyle(color: AppColors.textMuted)),
          ),
          ElevatedButton(
            style: ElevatedButton.styleFrom(backgroundColor: AppColors.verdictTranscode),
            onPressed: () => Navigator.of(ctx).pop(true),
            child: const Text('Eliminar', style: TextStyle(color: Colors.white, fontWeight: FontWeight.bold)),
          ),
        ],
      ),
    );

    if (confirm == true) {
      try {
        final f = File(path);
        if (f.existsSync()) {
          f.deleteSync();
        }
        setState(() {
          _allReports.removeWhere((r) => r.path == path);
          _selectedPaths.remove(path);
        });
        if (mounted) {
          ScaffoldMessenger.of(context).showSnackBar(
            SnackBar(
              content: Text('Archivo eliminado: $fileName'),
              backgroundColor: AppColors.verdictTranscode,
              duration: const Duration(seconds: 2),
            ),
          );
        }
      } catch (e) {
        if (mounted) {
          ScaffoldMessenger.of(context).showSnackBar(
            SnackBar(
              content: Text('Error al eliminar $fileName: $e'),
              backgroundColor: AppColors.verdictTranscode,
            ),
          );
        }
      }
    }
  }

  Future<void> _confirmDeleteSelected() async {
    final count = _selectedPaths.length;
    if (count == 0) return;

    final confirm = await showDialog<bool>(
      context: context,
      builder: (ctx) => AlertDialog(
        backgroundColor: AppColors.surfaceElevated,
        title: Row(
          children: [
            const Icon(Icons.delete_forever_rounded, color: AppColors.verdictTranscode, size: 26),
            const SizedBox(width: 8),
            Text('¿Eliminar $count canciones?', style: const TextStyle(color: Colors.white, fontSize: 16)),
          ],
        ),
        content: Text(
          'Vas a eliminar definitivamente $count archivos del disco o memoria USB.\n\nEsta acción NO se puede deshacer.\n¿Deseas borrarlos ahora?',
          style: const TextStyle(color: AppColors.textSecondary, fontSize: 13),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(ctx).pop(false),
            child: const Text('Cancelar', style: TextStyle(color: AppColors.textMuted)),
          ),
          ElevatedButton(
            style: ElevatedButton.styleFrom(backgroundColor: AppColors.verdictTranscode),
            onPressed: () => Navigator.of(ctx).pop(true),
            child: Text('Eliminar $count archivos', style: const TextStyle(color: Colors.white, fontWeight: FontWeight.bold)),
          ),
        ],
      ),
    );

    if (confirm == true) {
      int deleted = 0;
      final toRemove = List<String>.from(_selectedPaths);
      for (final p in toRemove) {
        try {
          final f = File(p);
          if (f.existsSync()) {
            f.deleteSync();
          }
          deleted++;
        } catch (e) {
          debugPrint('Error eliminando $p: $e');
        }
      }
      setState(() {
        _allReports.removeWhere((r) => _selectedPaths.contains(r.path));
        _selectedPaths.clear();
      });
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Text('$deleted canciones eliminadas del almacenamiento.'),
            backgroundColor: AppColors.verdictTranscode,
            duration: const Duration(seconds: 3),
          ),
        );
      }
    }
  }

  Future<void> _moveSelectedReports() async {
    if (_selectedPaths.isEmpty) return;
    final destDir = await getDirectoryPath();
    if (destDir == null || !mounted) return;

    int moved = 0;
    final toMove = List<String>.from(_selectedPaths);
    for (final oldPath in toMove) {
      try {
        final file = File(oldPath);
        if (file.existsSync()) {
          final name = oldPath.split(RegExp(r'[/\\]')).last;
          final newPath = '$destDir\\$name';
          file.renameSync(newPath);
          moved++;
          final idx = _allReports.indexWhere((r) => r.path == oldPath);
          if (idx >= 0) {
            final old = _allReports[idx];
            _allReports[idx] = old.copyWithPath(newPath);
          }
        }
      } catch (e) {
        debugPrint('Error moviendo $oldPath: $e');
      }
    }
    setState(() {
      _selectedPaths.clear();
    });
    if (mounted) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text('$moved canciones movidas a: $destDir'),
          backgroundColor: AppColors.verdictVerified,
          duration: const Duration(seconds: 3),
          action: SnackBarAction(
            label: 'Abrir Carpeta',
            textColor: Colors.white,
            onPressed: () {
              Process.run('explorer.exe', [destDir]);
            },
          ),
        ),
      );
    }
  }

  Future<void> _renameSelectedWithQuality() async {
    if (_selectedPaths.isEmpty) return;
    final count = _selectedPaths.length;

    final confirm = await showDialog<bool>(
      context: context,
      builder: (ctx) => AlertDialog(
        backgroundColor: AppColors.surfaceElevated,
        title: const Row(
          children: [
            Icon(Icons.drive_file_rename_outline_rounded, color: AppColors.verdictSuspicious, size: 24),
            SizedBox(width: 8),
            Text('Renombrar con Calidad Real', style: TextStyle(color: Colors.white, fontSize: 16)),
          ],
        ),
        content: Text(
          'Se antepondrá la etiqueta de calidad real al nombre de $count canciones.\n\nEjemplos:\n• Canción.mp3 ➔ [128k] Canción.mp3\n• Pista.wav ➔ [Fake-128k] Pista.wav\n• Audio.wav ➔ [Lossless] Audio.wav\n\nEsto te permitirá ver la calidad de inmediato en Rekordbox, Serato o VirtualDJ.\n¿Deseas renombrarlas?',
          style: const TextStyle(color: AppColors.textSecondary, fontSize: 13),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(ctx).pop(false),
            child: const Text('Cancelar', style: TextStyle(color: AppColors.textMuted)),
          ),
          ElevatedButton(
            style: ElevatedButton.styleFrom(backgroundColor: AppColors.verdictSuspicious),
            onPressed: () => Navigator.of(ctx).pop(true),
            child: const Text('Renombrar', style: TextStyle(color: Colors.black, fontWeight: FontWeight.bold)),
          ),
        ],
      ),
    );

    if (confirm != true) return;

    int renamed = 0;
    final toRename = List<String>.from(_selectedPaths);
    for (final p in toRename) {
      try {
        final file = File(p);
        if (file.existsSync()) {
          final dir = file.parent.path;
          final baseName = p.split(RegExp(r'[/\\]')).last;
          final rep = _allReports.firstWhere((r) => r.path == p);

          String prefix = '[Real]';
          if (rep.verdictCode == 'ProbableTranscode') {
            final bw = rep.effectiveBandwidthHz ?? 0;
            if (bw <= 16500) {
              prefix = '[Fake-128k]';
            } else if (bw <= 18500) {
              prefix = '[Fake-192k]';
            } else {
              prefix = '[Fake-256k]';
            }
          } else if (rep.verdictCode == 'LosslessVerified' || rep.verdictCode == 'LikelyLossless') {
            prefix = '[Lossless]';
          } else if (rep.verdictCode == 'Inconclusive') {
            prefix = '[NoConcluyente]';
          } else if (rep.facts.containerBitrateKbps != null) {
            prefix = '[${rep.facts.containerBitrateKbps}k]';
          }

          if (!baseName.startsWith('[')) {
            final newName = '$prefix $baseName';
            final newPath = '$dir\\$newName';
            file.renameSync(newPath);
            renamed++;

            final idx = _allReports.indexWhere((r) => r.path == p);
            if (idx >= 0) {
              final old = _allReports[idx];
              _allReports[idx] = old.copyWithPath(newPath);
            }
          }
        }
      } catch (e) {
        debugPrint('Error renombrando $p: $e');
      }
    }

    setState(() {
      _selectedPaths.clear();
    });

    if (mounted) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text('$renamed canciones renombradas con su calidad real.'),
          backgroundColor: AppColors.verdictVerified,
          duration: const Duration(seconds: 3),
        ),
      );
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
                                selectedPaths: _selectedPaths,
                                onToggleSelect: _toggleSelect,
                                onToggleSelectAll: _toggleSelectAll,
                                onSelectOnlyFakes: _selectOnlyFakes,
                                onDeleteSingle: _confirmDeleteSingle,
                                onSelectReport: (r) {
                                  showDialog(
                                    context: context,
                                    builder: (_) => FileDetailsModal(report: r),
                                  );
                                },
                              ),
                            ),
                            if (_selectedPaths.isNotEmpty) ...[
                              const SizedBox(height: 12),
                              _buildBatchActionBar(),
                            ],
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
    return LayoutBuilder(
      builder: (_, constraints) {
        final isCompact = constraints.maxWidth < 1050;
        final isVeryCompact = constraints.maxWidth < 850;

        return Container(
          height: 62,
          padding: const EdgeInsets.symmetric(horizontal: 14),
          decoration: const BoxDecoration(
            color: AppColors.surfaceElevated,
            border: Border(bottom: BorderSide(color: AppColors.surfaceBorder)),
          ),
          child: Row(
            children: [
              ClipRRect(
                borderRadius: BorderRadius.circular(8),
                child: Image.asset(
                  'assets/images/logo.png',
                  width: 34,
                  height: 34,
                  fit: BoxFit.contain,
                  errorBuilder: (_, __, ___) => const Icon(
                    Icons.graphic_eq_rounded,
                    color: AppColors.electricCyan,
                    size: 32,
                  ),
                ),
              ),
              const SizedBox(width: 10),
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
                          fontSize: 14,
                          letterSpacing: 0.9,
                        ),
                      ),
                      const SizedBox(width: 5),
                      const Text(
                        'AUDIO ANALYZER',
                        style: TextStyle(
                          color: AppColors.electricCyan,
                          fontWeight: FontWeight.w900,
                          fontSize: 14,
                          letterSpacing: 0.9,
                        ),
                      ),
                      const SizedBox(width: 6),
                      Container(
                        padding: const EdgeInsets.symmetric(horizontal: 5, vertical: 1.5),
                        decoration: BoxDecoration(
                          color: AppColors.electricCyan.withOpacity(0.15),
                          borderRadius: BorderRadius.circular(4),
                          border: Border.all(color: AppColors.electricCyan.withOpacity(0.3)),
                        ),
                        child: Text(
                          'v1.0 (Rev ${engineRevision()})',
                          style: const TextStyle(
                            color: AppColors.electricCyan,
                            fontSize: 9.5,
                            fontWeight: FontWeight.bold,
                          ),
                        ),
                      ),
                    ],
                  ),
                  if (!isVeryCompact) ...[
                    const SizedBox(height: 2),
                    const Text(
                      'Verifica la calidad real de tu música · 100% Offline y Seguro',
                      style: TextStyle(color: AppColors.textDimmed, fontSize: 10.5),
                    ),
                  ],
                ],
              ),
              const Spacer(),
              // Botones de acción principales
              ElevatedButton.icon(
                icon: const Icon(Icons.audio_file_rounded, size: 14),
                label: Text(isCompact ? 'Archivos' : 'Agregar Archivos',
                    style: const TextStyle(fontWeight: FontWeight.bold, fontSize: 11)),
                style: ElevatedButton.styleFrom(
                  backgroundColor: AppColors.electricCyan,
                  foregroundColor: AppColors.backgroundAbyssal,
                  padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 7),
                ),
                onPressed: _isAnalyzing ? null : _pickAndAnalyzeFiles,
              ),
              const SizedBox(width: 6),
              OutlinedButton.icon(
                icon: const Icon(Icons.folder_open_rounded, size: 14),
                label: Text(isCompact ? 'Carpeta' : 'Agregar Carpeta',
                    style: const TextStyle(fontSize: 11, fontWeight: FontWeight.bold)),
                style: OutlinedButton.styleFrom(
                  foregroundColor: AppColors.textPrimary,
                  side: const BorderSide(color: AppColors.surfaceBorder),
                  backgroundColor: AppColors.surfaceElevated,
                  padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 7),
                ),
                onPressed: _isAnalyzing ? null : _pickAndScanDirectory,
              ),
              if (_volumes.any((v) => v.isRemovable)) ...[
                const SizedBox(width: 6),
                ElevatedButton.icon(
                  icon: const Icon(Icons.usb_rounded, size: 14),
                  label: Text(
                    isCompact
                        ? 'USB (${_volumes.firstWhere((v) => v.isRemovable).label.isEmpty ? _volumes.firstWhere((v) => v.isRemovable).path : _volumes.firstWhere((v) => v.isRemovable).label})'
                        : 'Escanear USB (${_volumes.firstWhere((v) => v.isRemovable).label.isEmpty ? _volumes.firstWhere((v) => v.isRemovable).path : _volumes.firstWhere((v) => v.isRemovable).label})',
                    style: const TextStyle(fontWeight: FontWeight.bold, fontSize: 11),
                  ),
                  style: ElevatedButton.styleFrom(
                    backgroundColor: AppColors.electricCyan.withOpacity(0.18),
                    foregroundColor: AppColors.electricCyan,
                    side: const BorderSide(color: AppColors.electricCyan),
                    padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 7),
                  ),
                  onPressed: _isAnalyzing ? null : () => _scanFolder(_volumes.firstWhere((v) => v.isRemovable).path),
                ),
              ],
              const SizedBox(width: 8),
              Tooltip(
                message: 'Licencia SPP3 Verificada (Offline)',
                child: Container(
                  padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 5),
                  decoration: BoxDecoration(
                    color: AppColors.verdictVerified.withOpacity(0.12),
                    borderRadius: BorderRadius.circular(16),
                    border: Border.all(color: AppColors.verdictVerified.withOpacity(0.4)),
                  ),
                  child: Row(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      const Icon(Icons.verified_rounded, color: AppColors.verdictVerified, size: 15),
                      if (!isCompact) ...[
                        const SizedBox(width: 5),
                        const Text(
                          'SPP3 Offline',
                          style: TextStyle(
                            color: AppColors.verdictVerified,
                            fontSize: 11,
                            fontWeight: FontWeight.bold,
                          ),
                        ),
                      ],
                    ],
                  ),
                ),
              ),
              const SizedBox(width: 6),
              IconButton(
                tooltip: 'Diagnóstico del motor',
                icon: const Icon(Icons.health_and_safety_outlined, color: AppColors.textSecondary, size: 19),
                padding: EdgeInsets.zero,
                constraints: const BoxConstraints(minWidth: 28, minHeight: 28),
            onPressed: () async {
              final diag = await diagnostics();
              if (!mounted) return;
              showDialog(
                context: context,
                builder: (dialogCtx) => AlertDialog(
                  backgroundColor: AppColors.surfaceModal,
                  title: const Text('Diagnóstico del Motor Nativo', style: TextStyle(color: AppColors.textPrimary)),
                  content: Text(diag, style: const TextStyle(color: AppColors.textSecondary, fontFamily: 'Consolas')),
                  actions: [
                    TextButton(onPressed: () => Navigator.pop(dialogCtx), child: const Text('Cerrar')),
                  ],
                ),
              );
            },
          ),
        ],
      ),
    );
      },
    );
  }

  Widget _buildScanningProgress() {
    final pct = _totalToAnalyze > 0 ? (_analyzedCount / _totalToAnalyze).clamp(0.0, 1.0) : 0.0;
    final failuresText = _countInconclusive > 0 ? ' · $_countInconclusive fallos/ilegibles' : '';
    final progressText = _totalToAnalyze > 0
        ? 'Analizando audio... $_analyzedCount de $_totalToAnalyze (${(pct * 100).toStringAsFixed(0)}%)$failuresText'
        : (_analyzedCount > 0
            ? 'Analizando audio... $_analyzedCount pistas procesadas$failuresText'
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
                      'DISCOS Y MEMORIAS USB',
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
                        'VELOCIDAD DE ANÁLISIS',
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
                          _buildThrottleOption('turbo', 'Rápido', Icons.bolt_rounded),
                          const SizedBox(width: 4),
                          _buildThrottleOption('normal', 'Equilibrado', Icons.speed_rounded),
                          const SizedBox(width: 4),
                          _buildThrottleOption('silent', 'Silencioso', Icons.nightlight_round),
                        ],
                      ),
                    ],
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
        child: InkWell(
          onTap: _isAnalyzing ? null : _pickAndAnalyzeFiles,
          borderRadius: BorderRadius.circular(20),
          child: Container(
            constraints: const BoxConstraints(maxWidth: 620, maxHeight: 340),
            padding: const EdgeInsets.symmetric(horizontal: 32, vertical: 28),
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
                  child: const Icon(Icons.cloud_upload_rounded, color: AppColors.electricCyan, size: 36),
                ),
                const SizedBox(height: 18),
                const Text(
                  'Arrastra y suelta aquí tus canciones o carpetas',
                  style: TextStyle(
                    color: AppColors.textPrimary,
                    fontSize: 17,
                    fontWeight: FontWeight.bold,
                  ),
                  textAlign: TextAlign.center,
                ),
                const SizedBox(height: 8),
                const Text(
                  'Analiza la calidad real de cualquier formato de audio.\nDetecta pistas de baja fidelidad o archivos inflados a WAV/FLAC.',
                  style: TextStyle(
                    color: AppColors.textDimmed,
                    fontSize: 12.5,
                    height: 1.4,
                  ),
                  textAlign: TextAlign.center,
                ),
                const SizedBox(height: 18),
                Container(
                  padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 7),
                  decoration: BoxDecoration(
                    color: AppColors.surfaceCard,
                    borderRadius: BorderRadius.circular(20),
                    border: Border.all(color: AppColors.surfaceBorder),
                  ),
                  child: const Row(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      Icon(Icons.touch_app_rounded, size: 14, color: AppColors.electricCyan),
                      SizedBox(width: 6),
                      Text(
                        'Haz clic en este recuadro o usa los botones superiores para examinar',
                        style: TextStyle(color: AppColors.textSecondary, fontSize: 11.5),
                      ),
                    ],
                  ),
                ),
                const SizedBox(height: 12),
                const Text(
                  'Compatible con WAV, FLAC, AIFF, MP3, AAC, ALAC, OGG · 100% Offline',
                  style: TextStyle(
                    color: AppColors.textDimmed,
                    fontSize: 11,
                  ),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }

  Widget _buildBatchActionBar() {
    final count = _selectedPaths.length;
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
      decoration: BoxDecoration(
        color: AppColors.surfaceCard,
        borderRadius: BorderRadius.circular(10),
        border: Border.all(color: AppColors.electricCyan.withOpacity(0.5)),
        boxShadow: [
          BoxShadow(
            color: AppColors.electricCyan.withOpacity(0.08),
            blurRadius: 10,
            offset: const Offset(0, 2),
          ),
        ],
      ),
      child: Row(
        children: [
          const Icon(Icons.check_circle_rounded, color: AppColors.electricCyan, size: 20),
          const SizedBox(width: 8),
          Text(
            '$count ${count == 1 ? "canción seleccionada" : "canciones seleccionadas"}',
            style: const TextStyle(
              color: AppColors.textPrimary,
              fontWeight: FontWeight.bold,
              fontSize: 13,
            ),
          ),
          const Spacer(),
          // Mover a carpeta
          ElevatedButton.icon(
            icon: const Icon(Icons.drive_file_move_rounded, size: 16),
            label: const Text('Mover a Carpeta...', style: TextStyle(fontSize: 12)),
            style: ElevatedButton.styleFrom(
              backgroundColor: AppColors.surfaceElevated,
              foregroundColor: AppColors.electricCyan,
              side: BorderSide(color: AppColors.electricCyan.withOpacity(0.5)),
              padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
            ),
            onPressed: _moveSelectedReports,
          ),
          const SizedBox(width: 8),
          // Renombrar con calidad real
          ElevatedButton.icon(
            icon: const Icon(Icons.drive_file_rename_outline_rounded, size: 16),
            label: const Text('Renombrar con Calidad', style: TextStyle(fontSize: 12)),
            style: ElevatedButton.styleFrom(
              backgroundColor: AppColors.surfaceElevated,
              foregroundColor: AppColors.verdictSuspicious,
              side: BorderSide(color: AppColors.verdictSuspicious.withOpacity(0.5)),
              padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
            ),
            onPressed: _renameSelectedWithQuality,
          ),
          const SizedBox(width: 8),
          // Eliminar del disco
          ElevatedButton.icon(
            icon: const Icon(Icons.delete_forever_rounded, size: 16),
            label: const Text('Eliminar del Disco', style: TextStyle(fontSize: 12, fontWeight: FontWeight.bold)),
            style: ElevatedButton.styleFrom(
              backgroundColor: AppColors.verdictTranscode,
              foregroundColor: Colors.white,
              padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 8),
            ),
            onPressed: _confirmDeleteSelected,
          ),
          const SizedBox(width: 12),
          // Desmarcar
          IconButton(
            icon: const Icon(Icons.close_rounded, size: 18, color: AppColors.textMuted),
            tooltip: 'Desmarcar todas',
            onPressed: () => setState(() => _selectedPaths.clear()),
          ),
        ],
      ),
    );
  }

  Widget _buildSummaryCards() {
    return LayoutBuilder(
      builder: (context, constraints) {
        final isNarrow = constraints.maxWidth < 920;
        if (isNarrow) {
          return Column(
            children: [
              Row(
                children: [
                  _summaryCard('TOTAL', '${_allReports.length}', AppColors.textPrimary, Icons.library_music_rounded),
                  const SizedBox(width: 8),
                  _summaryCard('CALIDAD REAL', '$_countLossless', AppColors.verdictVerified, Icons.verified_rounded),
                  const SizedBox(width: 8),
                  _summaryCard('FALSO / INFLADO', '$_countTranscode', AppColors.verdictTranscode, Icons.dangerous_rounded),
                ],
              ),
              const SizedBox(height: 8),
              Row(
                children: [
                  _summaryCard('SOSPECHOSAS', '$_countSuspicious', AppColors.verdictSuspicious, Icons.warning_amber_rounded),
                  const SizedBox(width: 8),
                  _summaryCard('NO CONCLUYENTE', '$_countInconclusive', AppColors.verdictInconclusive, Icons.help_outline_rounded),
                  const SizedBox(width: 8),
                  _summaryCard('MP3 ESTÁNDAR', '$_countDeclared', AppColors.verdictDeclared, Icons.music_note_rounded),
                ],
              ),
            ],
          );
        }

        return Row(
          children: [
            _summaryCard('TOTAL ANALIZADAS', '${_allReports.length}', AppColors.textPrimary, Icons.library_music_rounded),
            const SizedBox(width: 8),
            _summaryCard('CALIDAD REAL', '$_countLossless', AppColors.verdictVerified, Icons.verified_rounded),
            const SizedBox(width: 8),
            _summaryCard('FALSO / INFLADO', '$_countTranscode', AppColors.verdictTranscode, Icons.dangerous_rounded),
            const SizedBox(width: 8),
            _summaryCard('SOSPECHOSAS', '$_countSuspicious', AppColors.verdictSuspicious, Icons.warning_amber_rounded),
            const SizedBox(width: 8),
            _summaryCard('NO CONCLUYENTE', '$_countInconclusive', AppColors.verdictInconclusive, Icons.help_outline_rounded),
            const SizedBox(width: 8),
            _summaryCard('MP3 ESTÁNDAR', '$_countDeclared', AppColors.verdictDeclared, Icons.music_note_rounded),
          ],
        );
      },
    );
  }

  Widget _summaryCard(String title, String value, Color color, IconData icon) {
    return Expanded(
      child: Container(
        padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 8),
        decoration: BoxDecoration(
          color: AppColors.surfaceElevated,
          borderRadius: BorderRadius.circular(10),
          border: Border.all(color: AppColors.surfaceBorder),
        ),
        child: Row(
          children: [
            Icon(icon, color: color, size: 18),
            const SizedBox(width: 8),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                mainAxisSize: MainAxisSize.min,
                children: [
                  Text(
                    title,
                    style: const TextStyle(color: AppColors.textDimmed, fontSize: 9, fontWeight: FontWeight.bold),
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                  ),
                  const SizedBox(height: 2),
                  Text(
                    value,
                    style: TextStyle(color: color, fontSize: 16, fontWeight: FontWeight.w900),
                    maxLines: 1,
                  ),
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
        // Chips en scroll horizontal fluido para que nunca estrangulen la búsqueda
        Expanded(
          child: SingleChildScrollView(
            scrollDirection: Axis.horizontal,
            child: Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                _filterChip('ALL', 'Todas (${_allReports.length})'),
                const SizedBox(width: 6),
                _filterChip('LosslessVerified', 'Calidad Real ($_countLossless)'),
                const SizedBox(width: 6),
                _filterChip('ProbableTranscode', 'Falsos / Inflados ($_countTranscode)'),
                const SizedBox(width: 6),
                _filterChip('Suspicious', 'Sospechosas ($_countSuspicious)'),
                const SizedBox(width: 6),
                _filterChip('Inconclusive', 'No Concluyente ($_countInconclusive)'),
                const SizedBox(width: 6),
                _filterChip('DeclaredLossy', 'MP3 ($_countDeclared)'),
              ],
            ),
          ),
        ),
        const SizedBox(width: 10),
        // Buscador con ancho controlado
        SizedBox(
          width: 200,
          height: 34,
          child: Container(
            padding: const EdgeInsets.symmetric(horizontal: 8),
            decoration: BoxDecoration(
              color: AppColors.surfaceElevated,
              borderRadius: BorderRadius.circular(8),
              border: Border.all(color: AppColors.surfaceBorder),
            ),
            child: Row(
              children: [
                const Icon(Icons.search_rounded, color: AppColors.textDimmed, size: 15),
                const SizedBox(width: 6),
                Expanded(
                  child: TextField(
                    onChanged: (val) => setState(() => _searchQuery = val),
                    style: const TextStyle(color: AppColors.textPrimary, fontSize: 11.5),
                    decoration: const InputDecoration(
                      hintText: 'Buscar pista...',
                      hintStyle: TextStyle(color: AppColors.textDimmed, fontSize: 11),
                      border: InputBorder.none,
                      isDense: true,
                      contentPadding: EdgeInsets.zero,
                    ),
                  ),
                ),
                if (_searchQuery.isNotEmpty)
                  IconButton(
                    icon: const Icon(Icons.clear_rounded, size: 13, color: AppColors.textDimmed),
                    onPressed: () => setState(() => _searchQuery = ''),
                    padding: EdgeInsets.zero,
                    constraints: const BoxConstraints(),
                  ),
              ],
            ),
          ),
        ),
        const SizedBox(width: 8),
        IconButton(
          tooltip: 'Exportar reporte a Excel (CSV)',
          icon: const Icon(Icons.table_view_rounded, color: AppColors.electricCyan, size: 18),
          padding: EdgeInsets.zero,
          constraints: const BoxConstraints(minWidth: 28, minHeight: 28),
          onPressed: _allReports.isEmpty ? null : _exportCsv,
        ),
        IconButton(
          tooltip: 'Exportar reporte técnico (JSON)',
          icon: const Icon(Icons.code_rounded, color: AppColors.electricCyan, size: 18),
          padding: EdgeInsets.zero,
          constraints: const BoxConstraints(minWidth: 28, minHeight: 28),
          onPressed: _allReports.isEmpty ? null : _exportJson,
        ),
        IconButton(
          tooltip: 'Limpiar lista',
          icon: const Icon(Icons.delete_outline_rounded, color: AppColors.textDimmed, size: 18),
          padding: EdgeInsets.zero,
          constraints: const BoxConstraints(minWidth: 28, minHeight: 28),
          onPressed: _allReports.isEmpty
              ? null
              : () {
                  setState(() {
                    _allReports.clear();
                    _selectedPaths.clear();
                  });
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
