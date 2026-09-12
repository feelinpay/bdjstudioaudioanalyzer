use bdja_core::types::{Evidence, EvidenceCode, FormatFacts, QualityMetrics};
use crate::fft::get_fft_processor;
use crate::quality::analyze_quality;
use crate::spectrum::analyze_spectrum;
use crate::stereo::analyze_stereo;
use crate::temporal::analyze_temporal;

pub struct DspOutput {
    pub effective_bandwidth_hz: u32,
    pub cutoff_slope_db_oct: f64,
    pub evidences: Vec<Evidence>,
    pub quality: QualityMetrics,
    pub guards_triggered: Vec<String>,
    pub average_spectrum_db: Vec<f32>,
    pub is_strong_evidence_present: bool,
}

pub fn run_dsp_analysis(
    facts: &FormatFacts,
    windows_8192: &[Vec<f32>],
    windows_1024: &[Vec<f32>],
    left_samples: &[f32],
    right_samples: &[f32],
    max_peak: f32,
    clipped_samples: u64,
    dc_offset: f64,
    has_lossy_encoder_signature: bool,
    encoder_tag: Option<&str>,
    has_dj_metadata: bool,
    is_extension_mismatch: bool,
) -> DspOutput {
    let sample_rate = facts.sample_rate;
    let is_stereo = facts.channels >= 2 && !left_samples.is_empty() && !right_samples.is_empty();

    let fft = get_fft_processor();

    // 1. Compute FFT power spectra for all 8192 windows
    let mut power_spectra = Vec::with_capacity(windows_8192.len());
    for win in windows_8192 {
        power_spectra.push(fft.power_spectrum_8192(win));
    }

    // 2. Run module analyses
    let spec = analyze_spectrum(&power_spectra, sample_rate);
    let temp = analyze_temporal(windows_1024, windows_8192);
    let ster = if is_stereo {
        analyze_stereo(left_samples, right_samples, sample_rate)
    } else {
        analyze_stereo(&[], &[], sample_rate)
    };

    // Concatenate a portion of mono samples for quality analysis
    let mut mono_slice = Vec::new();
    for win in windows_8192.iter().take(4) {
        mono_slice.extend_from_slice(win);
    }
    let qual = analyze_quality(
        &mono_slice,
        max_peak,
        clipped_samples,
        dc_offset,
        if is_stereo { Some(ster.global_correlation) } else { None },
        facts.bit_depth,
    );

    // 3. Evaluate Guards against false positives
    let mut guards = Vec::new();

    // Guard: Audio too short (< 20s)
    if facts.duration_ms > 0 && facts.duration_ms < 20000 {
        guards.push("Duracion corta (< 20 s): resolucion estadistica limitada".to_string());
    }

    // Guard: Very quiet or silent track (LUFS < -45)
    if let Some(lufs) = qual.metrics.lufs_integrated {
        if lufs < -45.0 {
            guards.push("Nivel de audio muy bajo (LUFS < -45): posible vacio o pasaje ambiental".to_string());
        }
    }

    // Guard: Low sample rate declared (<= 32000)
    if facts.sample_rate <= 32000 {
        guards.push("Frecuencia de muestreo nativa baja (<= 32 kHz)".to_string());
    }

    // P1 GUARD: Material band-limited de origen (acústico natural, sin agudos pero con roll-off suave)
    let is_natural_band_limited = spec.effective_bandwidth_hz < 20000
        && spec.cutoff_slope_db_oct < 24.0;

    if is_natural_band_limited {
        guards.push("Material acústico con ancho de banda limitado de origen (roll-off suave < 24 dB/oct, sin firmas de compresión digital)".to_string());
    }

    // 4. Construct E01 - E14 Evidences
    let mut evidences = Vec::new();
    let mut strong_evidence_present = false;

    // E01: Ancho de banda efectivo
    let e01_hz = spec.effective_bandwidth_hz as f64;
    let (e01_llr, e01_desc) = if is_natural_band_limited {
        (0.0, format!("Ancho de banda medido en {} Hz compatible con instrumentación o máster analógico de origen (roll-off suave)", spec.effective_bandwidth_hz))
    } else if e01_hz <= 16500.0 {
        (2.2, format!("Corte brusco en {} Hz (compatible con MP3 128 kbps)", spec.effective_bandwidth_hz))
    } else if e01_hz <= 18500.0 {
        (1.6, format!("Corte en {} Hz (compatible con MP3 192 kbps)", spec.effective_bandwidth_hz))
    } else if e01_hz <= 19800.0 {
        (1.0, format!("Corte en {} Hz (compatible con MP3 256/320 kbps)", spec.effective_bandwidth_hz))
    } else if e01_hz >= 20500.0 {
        (-1.8, format!("Espectro completo hasta {} Hz sin corte artificial", spec.effective_bandwidth_hz))
    } else {
        (0.0, format!("Ancho de banda medido: {} Hz", spec.effective_bandwidth_hz))
    };
    evidences.push(Evidence {
        code: EvidenceCode::E01,
        value: Some(e01_hz),
        llr: e01_llr,
        applicable: true,
        description: e01_desc,
    });

    // E02: Pendiente del corte
    let e02_val = spec.cutoff_slope_db_oct;
    let (e02_llr, e02_desc) = if e02_val >= 60.0 {
        (1.5, format!("Pendiente vertical brick-wall de {:.1} dB/oct (filtro digital de encoder)", e02_val))
    } else if e02_val < 24.0 {
        (-1.2, format!("Roll-off suave de {:.1} dB/oct compatible con acustica natural", e02_val))
    } else {
        (0.4, format!("Pendiente de corte intermedia: {:.1} dB/oct", e02_val))
    };
    evidences.push(Evidence {
        code: EvidenceCode::E02,
        value: Some(e02_val),
        llr: e02_llr,
        applicable: e01_hz < 20500.0,
        description: e02_desc,
    });

    // E03: Shelf de 16 kHz
    let e03_val = spec.shelf_16k_drop_db;
    let (e03_llr, e03_desc) = if e03_val >= 18.0 {
        (1.2, format!("Escalon / shelf pronunciado de {:.1} dB a 16 kHz (firma tipica LAME/AAC)", e03_val))
    } else {
        (0.0, format!("Sin escalon artificial a 16 kHz ({:.1} dB)", e03_val))
    };
    evidences.push(Evidence {
        code: EvidenceCode::E03,
        value: Some(e03_val),
        llr: e03_llr,
        applicable: true,
        description: e03_desc,
    });

    // E04: Huecos espectrales (FUERTE)
    let e04_val = spec.spectral_holes_ratio;
    let (e04_llr, e04_desc) = if e04_val >= 0.15 {
        strong_evidence_present = true;
        (2.5, format!("Huecos psicoacusticos marcados ({:.1}% subbandas anuladas en agudos)", e04_val * 100.0))
    } else if e04_val >= 0.08 {
        (1.4, format!("Presencia moderada de huecos espectrales ({:.1}%)", e04_val * 100.0))
    } else {
        (-1.4, format!("Continuidad espectral natural ({:.1}% huecos)", e04_val * 100.0))
    };
    evidences.push(Evidence {
        code: EvidenceCode::E04,
        value: Some(e04_val),
        llr: e04_llr,
        applicable: true,
        description: e04_desc,
    });

    // E05: Rejilla de bloques (FUERTE)
    let e05_val = temp.block_grid_peak_ratio;
    let (e05_llr, e05_desc) = if let Some(bs) = temp.detected_block_size {
        if e05_val >= 0.08 {
            strong_evidence_present = true;
            (2.2, format!("Estructura de tramas MDCT detectada: periodicidad de {} muestras ({:?})", bs, if bs == 576 { "MP3" } else { "AAC" }))
        } else {
            (0.0, "Sin periodicidad de tramas MDCT detectable".to_string())
        }
    } else {
        (0.0, "Sin periodicidad de bloques de compresion".to_string())
    };
    evidences.push(Evidence {
        code: EvidenceCode::E05,
        value: Some(e05_val),
        llr: e05_llr,
        applicable: true,
        description: e05_desc,
    });

    // E06: Pre-eco
    let e06_val = temp.pre_echo_score;
    let (e06_llr, e06_desc) = if e06_val >= 0.25 {
        (0.9, format!("Artefacto de pre-eco detectado previo a transitorios ({:.2})", e06_val))
    } else {
        (0.0, "Sin artefactos notables de pre-eco".to_string())
    };
    evidences.push(Evidence {
        code: EvidenceCode::E06,
        value: Some(e06_val),
        llr: e06_llr,
        applicable: true,
        description: e06_desc,
    });

    // E07: Colapso de joint-stereo (FUERTE)
    let (e07_llr, e07_desc, e07_app) = if !is_stereo {
        (0.0, "No aplicable (audio monofonico)".to_string(), false)
    } else if ster.joint_stereo_collapse_detected {
        strong_evidence_present = true;
        (2.4, format!("Colapso de canales a mono en agudos (> {} Hz, firma Intensity Stereo)", ster.joint_stereo_crossover_hz.unwrap_or(14000)), true)
    } else {
        (0.0, format!("Separacion estereo coherente en todo el espectro (correlacion HF: {:.2})", ster.high_band_correlation), true)
    };
    evidences.push(Evidence {
        code: EvidenceCode::E07,
        value: if is_stereo { Some(ster.high_band_correlation) } else { None },
        llr: e07_llr,
        applicable: e07_app,
        description: e07_desc,
    });

    // E08: Piso de ruido y dither
    let (e08_llr, e08_desc) = if qual.has_exact_digital_silence && e01_hz < 20000.0 {
        (0.8, "Silencio digital absoluto en pasajes de bajo nivel (tipico de decodificacion lossy)".to_string())
    } else if qual.noise_floor_db < -80.0 {
        (-0.8, format!("Piso de ruido coherente con dither analogico ({:.1} dB)", qual.noise_floor_db))
    } else {
        (0.0, format!("Piso de ruido medido: {:.1} dB", qual.noise_floor_db))
    };
    evidences.push(Evidence {
        code: EvidenceCode::E08,
        value: Some(qual.noise_floor_db),
        llr: e08_llr,
        applicable: true,
        description: e08_desc,
    });

    // E09: Bit depth real
    let (e09_llr, e09_desc) = if qual.bit_depth_inflated {
        (1.4, format!("Inflado de bits: declara {} bits pero el contenido dinamico real corresponde a {} bits", facts.bit_depth.unwrap_or(24), qual.real_bit_depth))
    } else {
        (0.0, format!("Profundidad de bits consistente ({} bits)", qual.real_bit_depth))
    };
    evidences.push(Evidence {
        code: EvidenceCode::E09,
        value: Some(qual.real_bit_depth as f64),
        llr: e09_llr,
        applicable: facts.bit_depth.is_some(),
        description: e09_desc,
    });

    // E10: Upsampling
    let (e10_llr, e10_desc) = if spec.upsampling_detected {
        (2.0, format!("Falso high-res: declara {} Hz pero carece completamente de energia por encima de 21.5 kHz", sample_rate))
    } else {
        (0.0, "Frecuencia de muestreo y ancho de banda consistentes".to_string())
    };
    evidences.push(Evidence {
        code: EvidenceCode::E10,
        value: Some(if spec.upsampling_detected { 1.0 } else { 0.0 }),
        llr: e10_llr,
        applicable: sample_rate >= 48000,
        description: e10_desc,
    });

    // E11: Inflado de contenedor
    let (e11_llr, e11_desc) = if facts.is_lossless_declared && e01_hz <= 16500.0 {
        (1.8, format!("Contenedor lossless ({} kbps) pero ancho de banda limitado a {} Hz (equivalente a 128 kbps)", facts.container_bitrate_kbps.unwrap_or(1411), spec.effective_bandwidth_hz))
    } else {
        (0.0, "Bitrate del contenedor acorde al contenido".to_string())
    };
    evidences.push(Evidence {
        code: EvidenceCode::E11,
        value: facts.container_bitrate_kbps.map(|b| b as f64),
        llr: e11_llr,
        applicable: facts.is_lossless_declared,
        description: e11_desc,
    });

    // E12: Metricas de calidad (Informativo)
    evidences.push(Evidence {
        code: EvidenceCode::E12,
        value: qual.metrics.lufs_integrated,
        llr: 0.0,
        applicable: true,
        description: format!(
            "True Peak: {:.1} dBTP, LUFS: {:.1}, Muestras clipeadas: {}",
            qual.metrics.true_peak_dbtp.unwrap_or(0.0),
            qual.metrics.lufs_integrated.unwrap_or(-14.0),
            qual.metrics.clipped_samples
        ),
    });

    // E13: Metadata forense (FUERTE)
    let (e13_llr, e13_desc) = if has_lossy_encoder_signature {
        strong_evidence_present = true;
        let tag = encoder_tag.unwrap_or("Xing/LAME");
        (3.5, format!("Cabecera / firma residual de compresión MP3/AAC encontrada en metadata: {}", tag))
    } else if is_extension_mismatch {
        strong_evidence_present = true;
        (3.5, "Discrepancia crítica: extensión no corresponde al formato binario real".to_string())
    } else if has_dj_metadata {
        (0.0, "Metadatos estándar de software DJ presentes (Serato/Rekordbox). Totalmente legítimo.".to_string())
    } else if let Some(tag) = encoder_tag {
        (0.0, format!("Metadatos de software de exportación legítimos detectados: {}", tag))
    } else {
        (0.0, "Cabeceras y metadata documental limpias".to_string())
    };
    evidences.push(Evidence {
        code: EvidenceCode::E13,
        value: if strong_evidence_present { Some(1.0) } else { Some(0.0) },
        llr: e13_llr,
        applicable: true,
        description: e13_desc,
    });

    // E14: Consistencia temporal
    let (e14_llr, e14_desc) = if temp.temporal_variance < 0.4 && e01_hz < 19000.0 {
        (0.5, format!("Corte constante a lo largo de toda la pista (varianza temporal baja: {:.2})", temp.temporal_variance))
    } else {
        (0.0, format!("Consistencia temporal evaluada (varianza: {:.2})", temp.temporal_variance))
    };
    evidences.push(Evidence {
        code: EvidenceCode::E14,
        value: Some(temp.temporal_variance),
        llr: e14_llr,
        applicable: true,
        description: e14_desc,
    });

    DspOutput {
        effective_bandwidth_hz: spec.effective_bandwidth_hz,
        cutoff_slope_db_oct: spec.cutoff_slope_db_oct,
        evidences,
        quality: qual.metrics,
        guards_triggered: guards,
        average_spectrum_db: spec.average_spectrum_db,
        is_strong_evidence_present: strong_evidence_present,
    }
}