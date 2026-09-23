use bdja_core::types::{Codec, FileReport, FormatFacts, QualityMetrics, Verdict, ENGINE_REV};
use bdja_decode::decode_audio_file;
use bdja_dsp::run_dsp_analysis;
use bdja_verdict::evaluate_verdict;
use std::path::Path;

pub fn analyze_single_file(path: &Path) -> Result<FileReport, String> {
    let metadata = match std::fs::metadata(path) {
        Ok(m) => m,
        Err(e) => return Err(format!("No se pudo leer metadatos de {:?}: {}", path, e)),
    };
    let file_size = metadata.len();
    let path_str = path.to_string_lossy().to_string();

    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        analyze_single_file_inner(path, file_size, &path_str)
    }));

    match res {
        Ok(r) => r,
        Err(panic_err) => {
            let msg = if let Some(s) = panic_err.downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = panic_err.downcast_ref::<String>() {
                s.clone()
            } else {
                "Pánico desconocido en el análisis de audio".to_string()
            };
            Ok(FileReport {
                file_id: 0,
                path: path_str,
                file_size,
                engine_rev: ENGINE_REV,
                facts: FormatFacts {
                    container: "CORRUPTED".to_string(),
                    codec: "PANIC_RECOVERED".to_string(),
                    codec_type: Codec::Unknown,
                    sample_rate: 0,
                    bit_depth: None,
                    channels: 0,
                    duration_ms: 0,
                    container_bitrate_kbps: None,
                    is_lossless_declared: false,
                },
                verdict: Verdict::Inconclusive,
                confidence: 0.5,
                score_llr: 0.0,
                effective_bandwidth_hz: None,
                cutoff_slope_db_oct: None,
                cutoff_kind: None,
                evidences: Vec::new(),
                quality: QualityMetrics {
                    true_peak_dbtp: None,
                    lufs_integrated: None,
                    clipped_samples: 0,
                    dc_offset: None,
                    dynamic_range_db: None,
                    stereo_correlation: None,
                },
                guards_triggered: vec![format!("Pánico capturado: {}", msg)],
                verdict_summary: format!("Error crítico evitado al procesar el archivo: {}", msg),
                average_spectrum_db: vec![-120.0; 256],
            })
        }
    }
}

fn analyze_single_file_inner(
    path: &Path,
    file_size: u64,
    path_str: &str,
) -> Result<FileReport, String> {
    // 1. Decode & Forensic header extraction
    let decoded = match decode_audio_file(path) {
        Ok(d) => d,
        Err(e) => {
            let ext = path
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("UNKNOWN")
                .to_lowercase();
            let ext_upper = ext.to_uppercase();

            let (codec, guard, summary) = match &e {
                bdja_decode::DecodeError::Unsupported(msg) => (
                    format!("No soportado ({})", ext),
                    format!(
                        "Formato de audio no soportado actualmente por el motor ({})",
                        ext
                    ),
                    format!(
                        "Formato de audio ({}) reconocido en la biblioteca pero no soportado por el decodificador: {}",
                        ext_upper, msg
                    ),
                ),
                bdja_decode::DecodeError::UnrecognizedFormat(msg) => (
                    format!("No reconocido ({})", ext),
                    format!("Formato o contenedor no reconocido ({})", ext),
                    format!(
                        "El archivo ({}) no pudo ser identificado como ningún contenedor de audio válido: {}",
                        ext_upper, msg
                    ),
                ),
                bdja_decode::DecodeError::CorruptedHeader(msg) => (
                    "Error/Corrupto".to_string(),
                    format!("Cabecera de audio dañada o corrupta ({})", ext),
                    format!(
                        "El archivo ({}) presenta una cabecera corrupta o malformada: {}",
                        ext_upper, msg
                    ),
                ),
                bdja_decode::DecodeError::DecoderInit(msg) => (
                    "Error/Corrupto".to_string(),
                    format!("Fallo al inicializar decodificador ({})", ext),
                    format!(
                        "No se pudo inicializar el decodificador para {}: flujo de audio dañado o inválido ({})",
                        ext_upper, msg
                    ),
                ),
                bdja_decode::DecodeError::Io(err) => (
                    "Error/I-O".to_string(),
                    format!("Error de lectura I/O: {}", err),
                    format!("No se pudo leer el archivo de audio (error de I/O): {}", err),
                ),
                bdja_decode::DecodeError::ZeroLength => (
                    "Error/Vacío".to_string(),
                    "Archivo vacío o de longitud cero".to_string(),
                    "El archivo de audio está vacío (0 bytes)".to_string(),
                ),
                bdja_decode::DecodeError::FileTooLarge(size) => (
                    "Error/Tamaño".to_string(),
                    format!("Archivo excede el límite máximo de 2 GB ({} bytes)", size),
                    "El archivo excede el tamaño máximo permitido de 2 GB".to_string(),
                ),
                bdja_decode::DecodeError::DurationExceeded(dur) => (
                    "Error/Duración".to_string(),
                    format!("Duración excede el límite máximo de 3 horas ({} ms)", dur),
                    "El archivo excede la duración máxima permitida de 3 horas".to_string(),
                ),
                bdja_decode::DecodeError::NoAudioTrack => (
                    "Error/SinAudio".to_string(),
                    "No se encontraron pistas de audio en el archivo".to_string(),
                    "El archivo no contiene pistas de audio decodificables".to_string(),
                ),
                bdja_decode::DecodeError::PacketDecode(msg)
                | bdja_decode::DecodeError::Symphonia(msg) => (
                    "Error/Corrupto".to_string(),
                    format!("Fallo de decodificación (posible archivo corrupto): {}", msg),
                    format!(
                        "No se pudo decodificar el flujo de audio (posible archivo corrupto o truncado): {}",
                        msg
                    ),
                ),
            };

            return Ok(FileReport {
                file_id: 0,
                path: path_str.to_string(),
                file_size,
                engine_rev: ENGINE_REV,
                facts: FormatFacts {
                    container: ext_upper,
                    codec,
                    codec_type: Codec::Unknown,
                    sample_rate: 0,
                    bit_depth: None,
                    channels: 0,
                    duration_ms: 0,
                    container_bitrate_kbps: None,
                    is_lossless_declared: false,
                },
                verdict: Verdict::Inconclusive,
                confidence: 0.0,
                score_llr: 0.0,
                effective_bandwidth_hz: None,
                cutoff_slope_db_oct: None,
                cutoff_kind: None,
                evidences: Vec::new(),
                quality: QualityMetrics {
                    true_peak_dbtp: None,
                    lufs_integrated: None,
                    clipped_samples: 0,
                    dc_offset: None,
                    dynamic_range_db: None,
                    stereo_correlation: None,
                },
                guards_triggered: vec![guard],
                verdict_summary: summary,
                average_spectrum_db: vec![-120.0; 256],
            });
        }
    };

    // 2. DSP Analysis & 14 Evidences
    let dsp_out = run_dsp_analysis(
        &decoded.facts,
        &decoded.windows_8192,
        &decoded.windows_1024,
        &decoded.left_channel_samples,
        &decoded.right_channel_samples,
        decoded.max_peak,
        decoded.clipped_samples,
        decoded.dc_offset,
        decoded.forensic.has_lossy_encoder_signature,
        decoded.forensic.encoder_string.as_deref(),
        decoded.forensic.has_dj_metadata,
        decoded.forensic.is_extension_mismatch,
    );

    // 3. Verdict Evaluation
    let verdict_out = evaluate_verdict(
        &decoded.facts,
        &dsp_out.evidences,
        &dsp_out.guards_triggered,
        dsp_out.is_strong_evidence_present,
    );

    Ok(FileReport {
        file_id: 0,
        path: path_str.to_string(),
        file_size,
        engine_rev: ENGINE_REV,
        facts: decoded.facts,
        verdict: verdict_out.verdict,
        confidence: verdict_out.confidence,
        score_llr: verdict_out.score_llr,
        effective_bandwidth_hz: Some(dsp_out.effective_bandwidth_hz),
        cutoff_slope_db_oct: Some(dsp_out.cutoff_slope_db_oct),
        cutoff_kind: Some(dsp_out.cutoff_kind),
        evidences: dsp_out.evidences,
        quality: dsp_out.quality,
        guards_triggered: dsp_out.guards_triggered,
        verdict_summary: verdict_out.summary,
        average_spectrum_db: dsp_out.average_spectrum_db,
    })
}

pub fn inspect_file_spectral_drops(
    path: &std::path::Path,
) -> Result<Vec<bdja_dsp::SpectralDropPoint>, String> {
    let decoded = bdja_decode::decode_audio_file(path).map_err(|e| e.to_string())?;
    let fft = bdja_dsp::fft::get_fft_processor();
    let mut power_spectra = Vec::with_capacity(decoded.windows_8192.len());
    for win in &decoded.windows_8192 {
        power_spectra.push(fft.power_spectrum_8192(win));
    }
    Ok(bdja_dsp::measure_spectral_drops(
        &power_spectra,
        decoded.facts.sample_rate,
    ))
}

