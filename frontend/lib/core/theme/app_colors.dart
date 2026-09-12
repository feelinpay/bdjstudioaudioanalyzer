import 'package:flutter/material.dart';

/// Paleta oficial para BDJ Studio Audio Analyzer.
/// Extraída y calibrada directamente del logo oficial (logo.png):
/// Fondo Abisal (#050811) + Cyan Neón Eléctrico (#00F2FE) + Acento Azul Espectral (#4FACFE).
class AppColors {
  AppColors._();

  // Superficies principales
  static const Color background = Color(0xFF050811);
  static const Color surface = Color(0xFF0B101E);
  static const Color surfaceElevated = Color(0xFF131B30);
  static const Color surfaceAlt = Color(0xFF0E1526);

  // Aliases semánticos de superficies
  static const Color backgroundAbyssal = background;
  static const Color surfaceBorder = borderSubtle;
  static const Color surfaceModal = surfaceElevated;
  static const Color surfaceCard = surface;

  // Bordes y divisores
  static const Color borderSubtle = Color(0xFF182238);
  static const Color borderStrong = Color(0xFF26375A);
  static const Color borderGlow = Color(0x3300F2FE);

  // Acentos de marca (Identidad del Logo)
  static const Color primary = Color(0xFF00F2FE);
  static const Color primaryHover = Color(0xFF26F5FE);
  static const Color secondary = Color(0xFF4FACFE);
  static const Color accent = Color(0xFF00E5FF);
  static const Color electricCyan = primary;

  static const LinearGradient primaryGradient = LinearGradient(
    colors: [Color(0xFF00F2FE), Color(0xFF4FACFE)],
    begin: Alignment.topLeft,
    end: Alignment.bottomRight,
  );

  static const LinearGradient surfaceGradient = LinearGradient(
    colors: [Color(0xFF0B101E), Color(0xFF0E162B)],
    begin: Alignment.topCenter,
    end: Alignment.bottomCenter,
  );

  // Tipografía
  static const Color textPrimary = Color(0xFFF8FAFC);
  static const Color textSecondary = Color(0xFF94A3B8);
  static const Color textMuted = Color(0xFF64748B);
  static const Color textDimmed = textMuted;

  // Semántica de Veredictos de Audio (§08 PLAN_ARQUITECTURA.md)
  static const Color verdictLossless = Color(0xFF00F2FE);      // Lossless verificado (Cyan Neón)
  static const Color verdictVerified = verdictLossless;
  static const Color verdictLikely = Color(0xFF38BDF8);        // Probablemente lossless (Azul Cielo)
  static const Color verdictInconclusive = Color(0xFFFBBF24);  // Inconcluso (Ámbar)
  static const Color verdictSuspicious = Color(0xFFFB923C);    // Sospechoso (Naranja)
  static const Color verdictTranscode = Color(0xFFF43F5E);     // Probable transcode (Carmesí de alerta)
  static const Color verdictDeclaredLossy = Color(0xFF94A3B8); // Con pérdida declarado (Gris)
  static const Color verdictDeclared = verdictDeclaredLossy;

  // Estados generales
  static const Color success = Color(0xFF10B981);
  static const Color warning = Color(0xFFF59E0B);
  static const Color error = Color(0xFFEF4444);
}
