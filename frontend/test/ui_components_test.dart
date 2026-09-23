import 'dart:typed_data';
import 'package:flutter/material.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:bdj_studio_audio_analyzer/core/ffi/api.dart';
import 'package:bdj_studio_audio_analyzer/features/analyzer/presentation/widgets/file_details_modal.dart';
import 'package:bdj_studio_audio_analyzer/features/analyzer/presentation/widgets/spectrum_chart.dart';
import 'package:bdj_studio_audio_analyzer/features/analyzer/presentation/widgets/results_table.dart';
import 'package:bdj_studio_audio_analyzer/features/analyzer/presentation/utils/friendly_verdict_helper.dart';

void main() {
  testWidgets('SpectrumChart renders without error with 256 points', (tester) async {
    final points = Float32List.fromList(List.generate(256, (i) => -20.0 - (i * 0.3)));

    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: SpectrumChart(
            spectrumDb: points,
            cutoffHz: 16000,
            sampleRate: 44100,
            cutoffSlopeDbOct: 78.5,
          ),
        ),
      ),
    );

    expect(find.byType(SpectrumChart), findsOneWidget);
  });

  testWidgets('SpectrumChart renders empty state overlay when spectrum is empty', (tester) async {
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: SpectrumChart(
            spectrumDb: Float32List(0),
            sampleRate: 44100,
          ),
        ),
      ),
    );

    expect(find.byType(SpectrumChart), findsOneWidget);
    expect(find.text('Sin espectro disponible (formato no decodificable o corrupto)'), findsOneWidget);
  });

  testWidgets('ResultsTable renders empty state properly', (tester) async {
    await tester.pumpWidget(
      const MaterialApp(
        home: Scaffold(
          body: ResultsTable(reports: []),
        ),
      ),
    );

    expect(find.text('No hay archivos en la lista para el filtro seleccionado'), findsOneWidget);
  });

  test('FriendlyVerdictHelper maps codes to human friendly texts and states', () {
    final verified = FriendlyVerdictHelper.fromCode('LosslessVerified');
    expect(verified.isSafe, isTrue);
    expect(verified.isDanger, isFalse);
    expect(verified.badgeText, 'Calidad Real');

    final transcode = FriendlyVerdictHelper.fromCode('ProbableTranscode');
    expect(transcode.isDanger, isTrue);
    expect(transcode.isSafe, isFalse);
    expect(transcode.badgeText, 'Falso / Inflado');

    final vintage = FriendlyVerdictHelper.fromCode('Inconclusive');
    expect(vintage.badgeText, 'No Concluyente');

    final suspicious = FriendlyVerdictHelper.fromCode('Suspicious');
    expect(suspicious.badgeText, 'Sospechoso');

    final declared = FriendlyVerdictHelper.fromCode('DeclaredLossy');
    expect(declared.badgeText, 'Coincide');
  });

  testWidgets('ResultsTable renders master checkbox and fakes button when reports exist', (tester) async {
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: ResultsTable(
            reports: const [],
            selectedPaths: const {},
            onSelectOnlyFakes: () {},
          ),
        ),
      ),
    );

    expect(find.text('No hay archivos en la lista para el filtro seleccionado'), findsOneWidget);
  });

  testWidgets('FileDetailsModal renders unified single mode with spectrum and verdict', (tester) async {
    final report = FileReportFfi(
      fileId: 1,
      path: 'C:/Music/test_track.mp3',
      fileSize: BigInt.from(8500000),
      engineRev: 3,
      facts: FormatFactsFfi(
        container: 'mp3',
        codec: 'mp3',
        sampleRate: 44100,
        channels: 2,
        durationMs: BigInt.from(210000),
        containerBitrateKbps: 320,
        isLosslessDeclared: false,
      ),
      verdictCode: 'ProbableTranscode',
      verdictName: 'Transcodificación Probable',
      confidence: 0.98,
      scoreLlr: 14.5,
      effectiveBandwidthHz: 16000,
      cutoffSlopeDbOct: 75.0,
      cutoffKind: CutoffKindFfi.brickwallCutoff,
      evidences: const [
        EvidenceFfi(
          code: 'E01_BRICKWALL',
          value: 16000.0,
          llr: 8.5,
          applicable: true,
          description: 'Corte abrupto en 16000 Hz consistente con MP3 128 kbps',
        ),
      ],
      quality: QualityMetricsFfi(
        clippedSamples: BigInt.zero,
      ),
      guardsTriggered: [],
      verdictSummary: 'Transcode detectado',
      spectrumDb: Float32List(256),
    );

    tester.view.physicalSize = const Size(1200, 900);
    tester.view.devicePixelRatio = 1.0;
    addTearDown(tester.view.resetPhysicalSize);

    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: FileDetailsModal(report: report),
        ),
      ),
    );

    expect(find.byType(FileDetailsModal), findsOneWidget);
    expect(find.text('test_track.mp3'), findsOneWidget);
    expect(find.text('ESPECTRO DE FRECUENCIAS Y CORTE DETECTADO'), findsOneWidget);
    expect(find.text('METADATOS DEL ARCHIVO'), findsOneWidget);
  });
}
