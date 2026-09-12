use std::path::Path;
use bdja_core::types::{Codec, FileReport, FormatFacts, QualityMetrics, Verdict, ENGINE_REV};
use bdja_decode::decode_audio_file;
use bdja_dsp::run_dsp_analysis;
use bdja_verdict::evaluate_verdict;

pub fn analyze_single_file(path: &Path) -> Result<FileReport, String> {
    let metadata = match std::fs::metadata(path) {
        Ok(m) => m,
        Err(e) => return Err(format!("No se pudo leer metadatos de {:?}: {}", path, e)),
    };
    let file_size = metadata.len();
    let path_str = path.to_string_lossy().to_string();

    // 1. Decode & Forensic header extraction
    let decoded = match decode_audio_file(path) {
        Ok(d) => d,
        Err(e) => {
            // If decode failed, produce an Inconclusive report with error explanation
            return Ok(FileReport {
                file_id: 0,
                path: path_str,
                file_size,
                engine_rev: ENGINE_REV,
                facts: FormatFacts {
                    container: "UNKNOWN".to_string(),
                    codec: "UNSUPPORTED".to_string(),
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
                evidences: Vec::new(),
                quality: QualityMetrics {
                    true_peak_dbtp: None,
                    lufs_integrated: None,
                    clipped_samples: 0,
                    dc_offset: None,
                    dynamic_range_db: None,
                    stereo_correlation: None,
                },
                guards_triggered: vec![format!("Fallo de decodificacion: {}", e)],
                verdict_summary: format!("No se pudo decodificar el archivo de audio: {}", e),
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
        path: path_str,
        file_size,
        engine_rev: ENGINE_REV,
        facts: decoded.facts,
        verdict: verdict_out.verdict,
        confidence: verdict_out.confidence,
        score_llr: verdict_out.score_llr,
        effective_bandwidth_hz: Some(dsp_out.effective_bandwidth_hz),
        cutoff_slope_db_oct: Some(dsp_out.cutoff_slope_db_oct),
        evidences: dsp_out.evidences,
        quality: dsp_out.quality,
        guards_triggered: dsp_out.guards_triggered,
        verdict_summary: verdict_out.summary,
        average_spectrum_db: dsp_out.average_spectrum_db,
    })
}