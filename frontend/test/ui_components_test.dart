import 'dart:typed_data';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:bdj_studio_audio_analyzer/features/analyzer/presentation/widgets/spectrum_chart.dart';
import 'package:bdj_studio_audio_analyzer/features/analyzer/presentation/widgets/results_table.dart';

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
}
