import 'package:flutter/cupertino.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../../../../core/licensing/license_manager.dart';
import '../../../../core/theme/app_colors.dart';

class ActivationScreen extends StatefulWidget {
  final LicenseManager licenseManager;
  final VoidCallback onActivated;

  const ActivationScreen({
    super.key,
    required this.licenseManager,
    required this.onActivated,
  });

  @override
  State<ActivationScreen> createState() => _ActivationScreenState();
}

class _ActivationScreenState extends State<ActivationScreen> {
  final TextEditingController _tokenController = TextEditingController();
  String? _hwid;
  bool _loadingHwid = true;
  bool _isActivating = false;
  String? _errorMessage;

  @override
  void initState() {
    super.initState();
    _loadHwid();
  }

  Future<void> _loadHwid() async {
    try {
      final hwid = await widget.licenseManager.getHardwareFingerprint();
      if (mounted) {
        setState(() {
          _hwid = hwid;
          _loadingHwid = false;
        });
      }
    } catch (e) {
      if (mounted) {
        setState(() {
          _errorMessage = 'No se pudo generar el identificador de hardware: ';
          _loadingHwid = false;
        });
      }
    }
  }

  Future<void> _activate() async {
    final token = _tokenController.text.trim();
    if (token.isEmpty) {
      setState(() => _errorMessage = 'Por favor ingresa la clave de licencia SPP3.');
      return;
    }

    setState(() {
      _isActivating = true;
      _errorMessage = null;
    });

    final res = await widget.licenseManager.activateLicense(token);

    if (!mounted) return;

    res.fold(
      (failure) {
        setState(() {
          _errorMessage = failure.message;
          _isActivating = false;
        });
      },
      (success) {
        setState(() => _isActivating = false);
        widget.onActivated();
      },
    );
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      backgroundColor: AppColors.background,
      body: Center(
        child: SingleChildScrollView(
          padding: const EdgeInsets.symmetric(horizontal: 24, vertical: 36),
          child: ConstrainedBox(
            constraints: const BoxConstraints(maxWidth: 580),
            child: Container(
              padding: const EdgeInsets.all(36),
              decoration: BoxDecoration(
                color: AppColors.surface,
                borderRadius: BorderRadius.circular(20),
                border: Border.all(color: AppColors.borderSubtle),
                boxShadow: const [
                  BoxShadow(
                    color: Color(0x33000000),
                    blurRadius: 30,
                    offset: Offset(0, 10),
                  ),
                ],
              ),
              child: Column(
                mainAxisSize: MainAxisSize.min,
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  // Logo + Título
                  Center(
                    child: Container(
                      width: 80,
                      height: 80,
                      decoration: BoxDecoration(
                        borderRadius: BorderRadius.circular(16),
                        boxShadow: const [
                          BoxShadow(
                            color: AppColors.borderGlow,
                            blurRadius: 20,
                            spreadRadius: 2,
                          ),
                        ],
                      ),
                      clipBehavior: Clip.antiAlias,
                      child: Image.asset(
                        'assets/images/logo.png',
                        fit: BoxFit.cover,
                      ),
                    ),
                  ),
                  const SizedBox(height: 20),
                  const Text(
                    'BDJ Studio Audio Analyzer',
                    textAlign: TextAlign.center,
                    style: TextStyle(
                      fontSize: 22,
                      fontWeight: FontWeight.w800,
                      color: AppColors.textPrimary,
                      letterSpacing: 0.5,
                    ),
                  ),
                  const SizedBox(height: 6),
                  const Text(
                    'Control de Calidad y Detección de Falsos Lossless',
                    textAlign: TextAlign.center,
                    style: TextStyle(
                      fontSize: 13,
                      color: AppColors.textSecondary,
                    ),
                  ),
                  const SizedBox(height: 32),

                  // Paso 1: HWID
                  const Text(
                    'PASO 1 · ID DE ESTE EQUIPO (HWID)',
                    style: TextStyle(
                      fontSize: 11,
                      fontWeight: FontWeight.w700,
                      color: AppColors.primary,
                      letterSpacing: 1.2,
                    ),
                  ),
                  const SizedBox(height: 8),
                  Container(
                    padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
                    decoration: BoxDecoration(
                      color: AppColors.surfaceAlt,
                      borderRadius: BorderRadius.circular(10),
                      border: Border.all(color: AppColors.borderSubtle),
                    ),
                    child: Row(
                      children: [
                        const Icon(
                          CupertinoIcons.device_laptop,
                          color: AppColors.primary,
                          size: 20,
                        ),
                        const SizedBox(width: 12),
                        Expanded(
                          child: _loadingHwid
                              ? const Text(
                                  'Calculando identificador...',
                                  style: TextStyle(color: AppColors.textMuted),
                                )
                              : SelectableText(
                                  _hwid ?? 'Error de identificación',
                                  style: const TextStyle(
                                    fontFamily: 'Consolas',
                                    fontSize: 15,
                                    fontWeight: FontWeight.w700,
                                    letterSpacing: 1.5,
                                    color: AppColors.textPrimary,
                                  ),
                                ),
                        ),
                        IconButton(
                          icon: const Icon(Icons.copy_rounded, size: 18),
                          color: AppColors.primary,
                          tooltip: 'Copiar HWID',
                          onPressed: _hwid == null
                              ? null
                              : () {
                                  Clipboard.setData(ClipboardData(text: _hwid!));
                                  ScaffoldMessenger.of(context).showSnackBar(
                                    const SnackBar(
                                      content: Text('¡ID de equipo copiado al portapapeles!'),
                                      duration: Duration(seconds: 2),
                                      backgroundColor: AppColors.surfaceElevated,
                                    ),
                                  );
                                },
                        ),
                      ],
                    ),
                  ),
                  const SizedBox(height: 6),
                  const Text(
                    'Copia este código y emite la licencia en BDJ Studio License.',
                    style: TextStyle(fontSize: 12, color: AppColors.textMuted),
                  ),
                  const SizedBox(height: 24),

                  // Paso 2: Clave de Licencia
                  const Text(
                    'PASO 2 · CLAVE DE LICENCIA SPP3',
                    style: TextStyle(
                      fontSize: 11,
                      fontWeight: FontWeight.w700,
                      color: AppColors.primary,
                      letterSpacing: 1.2,
                    ),
                  ),
                  const SizedBox(height: 8),
                  TextField(
                    controller: _tokenController,
                    maxLines: 2,
                    decoration: InputDecoration(
                      hintText: 'Pega aquí tu clave SPP3....',
                      suffixIcon: IconButton(
                        icon: const Icon(Icons.paste_rounded, size: 18),
                        color: AppColors.textSecondary,
                        tooltip: 'Pegar desde portapapeles',
                        onPressed: () async {
                          final data = await Clipboard.getData('text/plain');
                          if (data?.text != null) {
                            _tokenController.text = data!.text!.trim();
                          }
                        },
                      ),
                    ),
                  ),
                  const SizedBox(height: 16),

                  // Mensaje de error
                  if (_errorMessage != null) ...[
                    Container(
                      padding: const EdgeInsets.all(12),
                      decoration: BoxDecoration(
                        color: AppColors.error.withValues(alpha: 0.1),
                        borderRadius: BorderRadius.circular(8),
                        border: Border.all(color: AppColors.error.withValues(alpha: 0.3)),
                      ),
                      child: Row(
                        children: [
                          const Icon(Icons.error_outline, color: AppColors.error, size: 20),
                          const SizedBox(width: 10),
                          Expanded(
                            child: Text(
                              _errorMessage!,
                              style: const TextStyle(fontSize: 13, color: AppColors.error),
                            ),
                          ),
                        ],
                      ),
                    ),
                    const SizedBox(height: 16),
                  ],

                  // Botón Activar
                  ElevatedButton(
                    onPressed: _isActivating ? null : _activate,
                    style: ElevatedButton.styleFrom(
                      padding: const EdgeInsets.symmetric(vertical: 16),
                      backgroundColor: AppColors.primary,
                      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(10)),
                    ),
                    child: _isActivating
                        ? const SizedBox(
                            width: 20,
                            height: 20,
                            child: CircularProgressIndicator(
                              strokeWidth: 2,
                              color: Color(0xFF030814),
                            ),
                          )
                        : const Text(
                            'ACTIVAR BDJ STUDIO AUDIO ANALYZER',
                            style: TextStyle(
                              fontSize: 13,
                              fontWeight: FontWeight.w800,
                              color: Color(0xFF030814),
                              letterSpacing: 1.0,
                            ),
                          ),
                  ),
                  const SizedBox(height: 16),
                  const Center(
                    child: Text(
                      'Licenciamiento 100% fuera de línea (Ed25519) · Sin telemetría',
                      style: TextStyle(fontSize: 11, color: AppColors.textMuted),
                    ),
                  ),
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }
}
