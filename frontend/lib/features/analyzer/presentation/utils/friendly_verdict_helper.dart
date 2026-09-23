import 'package:flutter/material.dart';
import '../../../../core/ffi/api.dart';

class FriendlyVerdict {
  final String title;
  final String badgeText;
  final String explanation;
  final String recommendation;
  final IconData icon;
  final Color color;
  final Color backgroundColor;
  final bool isDanger;
  final bool isSafe;

  const FriendlyVerdict({
    required this.title,
    required this.badgeText,
    required this.explanation,
    required this.recommendation,
    required this.icon,
    required this.color,
    required this.backgroundColor,
    this.isDanger = false,
    this.isSafe = false,
  });
}

class FriendlyVerdictHelper {
  FriendlyVerdictHelper._();

  static FriendlyVerdict fromCode(String code, {double? bandwidthHz}) {
    switch (code) {
      case 'LosslessVerified':
        return const FriendlyVerdict(
          title: 'Calidad Real (Lossless Original)',
          badgeText: 'Calidad Real',
          explanation:
              'Audio 100% original con fidelidad de estudio / CD. Contiene el rango completo de agudos y armónicos sin ninguna compresión destructiva.',
          recommendation:
              'Totalmente recomendado para DJs profesionales, clubes, festivales y equipos de alta fidelidad.',
          icon: Icons.verified_rounded,
          color: Color(0xFF00F2FE),
          backgroundColor: Color(0x2200F2FE),
          isSafe: true,
        );

      case 'LikelyLossless':
        return const FriendlyVerdict(
          title: 'Calidad Real (Lossless Auténtico)',
          badgeText: 'Calidad Real',
          explanation:
              'Audio auténtico de alta fidelidad. Cumple con los estándares de calidad sin pérdida de sonido.',
          recommendation:
              'Apto para eventos en vivo, producción y colecciones musicales exigentes.',
          icon: Icons.check_circle_rounded,
          color: Color(0xFF38BDF8),
          backgroundColor: Color(0x2238BDF8),
          isSafe: true,
        );

      case 'ProbableTranscode':
        return const FriendlyVerdict(
          title: 'Falso / Inflado (¡Calidad Real Inferior!)',
          badgeText: 'Falso / Inflado',
          explanation:
              '¡Cuidado! Este archivo fue inflado: originalmente era un audio de menor calidad (típicamente MP3 de 128 o 192 kbps) que fue guardado en un formato superior (como WAV, FLAC o MP3 320k) para fingir alta fidelidad. Suena como un audio de baja calidad pero ocupa mucho más espacio.',
          recommendation:
              'No pagues precio de alta fidelidad por este archivo. Si te lo vendieron como máster original o WAV de estudio, te engañaron.',
          icon: Icons.dangerous_rounded,
          color: Color(0xFFF43F5E),
          backgroundColor: Color(0x28F43F5E),
          isDanger: true,
        );

      case 'Suspicious':
        return const FriendlyVerdict(
          title: 'Sospechoso (Calidad Recortada)',
          badgeText: 'Sospechoso',
          explanation:
              'El audio presenta cortes abruptos en frecuencias agudas o indicios de compresión digital previa. Puede tratarse de una mezcla con fuentes de baja calidad o una compresión previa.',
          recommendation:
              'Se recomienda revisar el archivo antes de pincharlo en un evento grande o reclamar al proveedor.',
          icon: Icons.warning_amber_rounded,
          color: Color(0xFFFB923C),
          backgroundColor: Color(0x22FB923C),
          isDanger: false,
        );

      case 'Inconclusive':
        return const FriendlyVerdict(
          title: 'Calidad No Concluyente / Indeterminado',
          badgeText: 'No Concluyente',
          explanation:
              'El espectro no alcanza los 20 kHz máximos, pero la caída de frecuencias es suave y natural, sin cortes artificiales de MP3. Ocurre con grabaciones suaves, analógicas o producciones con pocos agudos.',
          recommendation:
              'Se sugiere revisar el espectrograma o escuchar la pista antes de decidir si descartarla.',
          icon: Icons.help_outline_rounded,
          color: Color(0xFFFBBF24),
          backgroundColor: Color(0x22FBBF24),
          isSafe: false,
        );

      case 'DeclaredLossy':
        return const FriendlyVerdict(
          title: 'Audio Comprimido Legítimo',
          badgeText: 'Coincide',
          explanation:
              'El archivo se guardó legítimamente en formato comprimido (MP3 / AAC). Su calidad acústica real coincide con lo que declara la etiqueta.',
          recommendation:
              'Perfecto para escucha personal en el teléfono o automóvil. Si necesitas calidad de club o estudio, busca la versión en WAV/FLAC original.',
          icon: Icons.music_note_rounded,
          color: Color(0xFF94A3B8),
          backgroundColor: Color(0x2294A3B8),
        );

      default:
        return const FriendlyVerdict(
          title: 'Veredicto no disponible',
          badgeText: 'Sin clasificar',
          explanation: 'No se pudo determinar con certeza la procedencia del archivo.',
          recommendation: 'Verifica la integridad del archivo en el reproductor.',
          icon: Icons.help_outline_rounded,
          color: Color(0xFF94A3B8),
          backgroundColor: Color(0x2294A3B8),
        );
    }
  }

  /// Retorna el formato y calidad que declara el contenedor o metadatos del archivo.
  static String getDeclaredQuality(FormatFactsFfi facts) {
    if (facts.isLosslessDeclared) {
      final bits = facts.bitDepth != null ? '${facts.bitDepth}b' : '16b';
      final sr = (facts.sampleRate / 1000).toStringAsFixed(1);
      return '${facts.container.toUpperCase()} (Lossless · $bits / $sr kHz)';
    } else {
      final kbps = facts.containerBitrateKbps != null ? ' ${facts.containerBitrateKbps} kbps' : '';
      return '${facts.container.toUpperCase()}$kbps';
    }
  }

  /// Retorna la calidad acústica real detectada según el análisis espectral (estilo Fakin' The Funk).
  static String getDetectedRealQuality(int? bandwidthHz, FormatFactsFfi facts, String verdictCode) {
    if (verdictCode == 'ProbableTranscode') {
      if (bandwidthHz != null) {
        if (bandwidthHz <= 16500) {
          return 'Calidad ~128 kbps (Baja)';
        } else if (bandwidthHz <= 18500) {
          return 'Calidad ~192 kbps (Media)';
        } else if (bandwidthHz <= 19800) {
          return 'Calidad ~256 kbps';
        }
      }
      return 'Comprimido Inflado (~128k)';
    }

    if (verdictCode == 'LosslessVerified' || verdictCode == 'LikelyLossless') {
      return 'Lossless Real (Estudio/CD)';
    }

    if (verdictCode == 'Inconclusive') {
      return 'Indeterminado (Suave)';
    }

    if (verdictCode == 'Suspicious') {
      if (bandwidthHz != null) {
        return 'Recortado (${(bandwidthHz / 1000).toStringAsFixed(1)} kHz)';
      }
      return 'Calidad Dudosa';
    }

    if (verdictCode == 'DeclaredLossy') {
      if (bandwidthHz != null) {
        if (bandwidthHz >= 19500) return 'Real ~320 kbps';
        if (bandwidthHz >= 17500) return 'Real ~192-256 kbps';
        if (bandwidthHz >= 15500) return 'Real ~128 kbps';
        return 'Baja (< 128 kbps)';
      }
      return '${facts.containerBitrateKbps ?? 128} kbps';
    }

    return 'No determinado';
  }

  /// Retorna el corte de frecuencias visible en kHz.
  static String getCutoffDisplay(int? bandwidthHz) {
    if (bandwidthHz == null) return '-';
    if (bandwidthHz >= 20500) return '> 20.5 kHz (Total)';
    return '${(bandwidthHz / 1000).toStringAsFixed(1)} kHz';
  }

  /// Retorna el estado sintético de veracidad (Real, Falso, Coincide, etc.)
  static String getVeracityBadgeText(FileReportFfi report) {
    if (report.verdictCode == 'ProbableTranscode') {
      if (report.facts.isLosslessDeclared) {
        return 'Falso Lossless';
      }
      final br = report.facts.containerBitrateKbps ?? 0;
      if (br >= 256) {
        return 'Falso ${br}k';
      }
      return 'Falso / Inflado';
    }

    if (report.verdictCode == 'LosslessVerified' || report.verdictCode == 'LikelyLossless') {
      return 'Genuino (Real)';
    }

    if (report.verdictCode == 'DeclaredLossy') {
      return 'Legítimo (Coincide)';
    }

    if (report.verdictCode == 'Suspicious') {
      return 'Sospechoso';
    }

    if (report.verdictCode == 'Inconclusive') {
      return 'No Concluyente';
    }

    return 'Sin clasificar';
  }
}

extension FileReportFfiCopy on FileReportFfi {
  FileReportFfi copyWithPath(String newPath) {
    return FileReportFfi(
      fileId: fileId,
      path: newPath,
      fileSize: fileSize,
      engineRev: engineRev,
      facts: facts,
      verdictCode: verdictCode,
      verdictName: verdictName,
      confidence: confidence,
      scoreLlr: scoreLlr,
      effectiveBandwidthHz: effectiveBandwidthHz,
      cutoffSlopeDbOct: cutoffSlopeDbOct,
      cutoffKind: cutoffKind,
      evidences: evidences,
      quality: quality,
      guardsTriggered: guardsTriggered,
      verdictSummary: verdictSummary,
      spectrumDb: spectrumDb,
    );
  }
}

