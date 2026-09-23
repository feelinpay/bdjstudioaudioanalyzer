use bdja_core::types::{Codec, FormatFacts};
use bdja_dsp::{run_dsp_analysis, FftProcessor};

#[test]
fn test_fft_sine_wave_peak() {
    let fft = FftProcessor::new();
    let sample_rate = 44100.0;
    let freq = 1000.0; // 1 kHz tone

    let mut window = vec![0.0f32; 8192];
    for (i, sample) in window.iter_mut().enumerate() {
        *sample = (2.0 * std::f32::consts::PI * freq * i as f32 / sample_rate).sin();
    }

    let power = fft.power_spectrum_8192(&window);
    let bin_hz = sample_rate / 8192.0;
    let expected_bin = (freq / bin_hz).round() as usize;

    // Find peak bin
    let (max_bin, _) = power
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
        .unwrap();

    assert!(
        (max_bin as isize - expected_bin as isize).abs() <= 2,
        "Peak at bin {}, expected ~{}",
        max_bin,
        expected_bin
    );
}

#[test]
fn test_dsp_full_bandwidth_white_noise() {
    let sample_rate = 44100;
    let mut rng: u32 = 123456789;
    let mut next_rand = || -> f32 {
        rng = rng.wrapping_mul(1103515245).wrapping_add(12345);
        ((rng % 20000) as f32 / 10000.0) - 1.0
    };

    let mut windows_8192 = Vec::new();
    for _ in 0..4 {
        let win: Vec<f32> = (0..8192).map(|_| next_rand()).collect();
        windows_8192.push(win);
    }

    let facts = FormatFacts {
        container: "WAV".to_string(),
        codec: "PCM 16-bit LE".to_string(),
        codec_type: Codec::PcmS16Le,
        sample_rate,
        bit_depth: Some(16),
        channels: 2,
        duration_ms: 60000,
        container_bitrate_kbps: Some(1411),
        is_lossless_declared: true,
    };

    let output = run_dsp_analysis(
        &facts,
        &windows_8192,
        &[],
        &[],
        &[],
        0.8,
        0,
        0.0,
        false,
        None,
        false,
        false,
    );

    // Full bandwidth white noise must reach > 21000 Hz
    assert!(
        output.effective_bandwidth_hz >= 21000,
        "Bandwidth {} should be >= 21000 for white noise",
        output.effective_bandwidth_hz
    );
    assert!(!output.is_strong_evidence_present);
    assert_eq!(output.average_spectrum_db.len(), 256);
}

#[test]
fn test_dsp_brickwall_transcode_detection() {
    let sample_rate = 44100;

    // Synthesize signal with harmonics strictly up to 16 kHz (simulating 128 kbps cutoff)
    let mut windows_8192 = Vec::new();
    for phase_offset in 0..4 {
        let mut win = vec![0.0f32; 8192];
        for f_khz in 1..=16 {
            let freq = f_khz as f32 * 1000.0;
            let phase = phase_offset as f32 * 0.785;
            for (i, s) in win.iter_mut().enumerate() {
                *s += (2.0 * std::f32::consts::PI * freq * i as f32 / sample_rate as f32 + phase)
                    .sin()
                    / 16.0;
            }
        }
        windows_8192.push(win);
    }

    let facts = FormatFacts {
        container: "WAV".to_string(),
        codec: "PCM 16-bit LE".to_string(),
        codec_type: Codec::PcmS16Le,
        sample_rate,
        bit_depth: Some(16),
        channels: 2,
        duration_ms: 180000,
        container_bitrate_kbps: Some(1411),
        is_lossless_declared: true,
    };

    let output = run_dsp_analysis(
        &facts,
        &windows_8192,
        &[],
        &[],
        &[],
        0.8,
        0,
        0.0,
        false,
        None,
        false,
        false,
    );

    // Cutoff should be detected close to 16000 Hz
    assert!(
        output.effective_bandwidth_hz <= 16500 && output.effective_bandwidth_hz >= 15500,
        "Expected cutoff ~16000 Hz, got {}",
        output.effective_bandwidth_hz
    );
}

#[test]
fn test_dsp_natural_band_limited_guard() {
    let sample_rate = 44100;

    // Synthesize audio with natural acoustic gentle slope (-12 dB/oct roll-off) and dither floor
    let mut rng: u32 = 987654321;
    let mut next_dither = || -> f32 {
        rng = rng.wrapping_mul(1103515245).wrapping_add(12345);
        (((rng % 20000) as f32 / 10000.0) - 1.0) * 0.0003 // -70 dBFS dither floor
    };

    let mut windows_8192 = Vec::new();
    for phase_offset in 0..4 {
        let mut win = vec![0.0f32; 8192];
        for f_khz in 1..=22 {
            let freq = f_khz as f32 * 1000.0;
            // Roll-off rápido que decae suavemente hacia el piso antes de 16 kHz
            let amp = if f_khz <= 6 {
                1.0
            } else {
                1.0 / (1.0 + ((f_khz - 6) as f32).powi(2) * 1.5)
            };
            let phase = phase_offset as f32 * 0.5;
            for (i, s) in win.iter_mut().enumerate() {
                *s += amp
                    * (2.0 * std::f32::consts::PI * freq * i as f32 / sample_rate as f32 + phase)
                        .sin()
                    / 15.0
                    + next_dither();
            }
        }
        windows_8192.push(win);
    }

    let facts = FormatFacts {
        container: "WAV".to_string(),
        codec: "PCM 16-bit LE".to_string(),
        codec_type: Codec::PcmS16Le,
        sample_rate,
        bit_depth: Some(16),
        channels: 2,
        duration_ms: 180000,
        container_bitrate_kbps: Some(1411),
        is_lossless_declared: true,
    };

    let output = run_dsp_analysis(
        &facts,
        &windows_8192,
        &[],
        &[],
        &[],
        0.8,
        0,
        0.0,
        false,
        None,
        false,
        false,
    );

    // Natural gentle rolloff must trigger band-limited guard unconditionally
    assert!(
        output.effective_bandwidth_hz < 20000,
        "Bandwidth should be restricted by natural decay, got {}",
        output.effective_bandwidth_hz
    );
    assert!(
        output
            .guards_triggered
            .iter()
            .any(|g| g.contains("ancho de banda limitado")),
        "Should have triggered band-limited guard for gentle slope"
    );
}

#[test]
fn test_dsp_continuous_lossless_attains_verified() {
    let sample_rate = 44100;

    // Synthesize dense musical master: harmonics across the entire spectrum up to 22.05 kHz
    // with natural musical spectral density (-4.5 dB/octave) + analog dither
    let mut rng: u32 = 42424242;
    let mut next_dither = || -> f32 {
        rng = rng.wrapping_mul(1103515245).wrapping_add(12345);
        (((rng % 20000) as f32 / 10000.0) - 1.0) * 0.00003 // 16-bit dither floor ~ -90 dBFS
    };

    let mut windows_8192 = Vec::new();
    // Pre-roll / intro con piso de dither de máster (~ -90 dBFS)
    let mut intro = vec![0.0f32; 8192];
    for s in intro.iter_mut() {
        *s = next_dither();
    }
    windows_8192.push(intro);

    for phase_offset in 0..6 {
        let mut win = vec![0.0f32; 8192];
        for f_idx in 1..=100 {
            let freq = 100.0 + f_idx as f32 * 218.0; // Distribuido densamente hasta 21.9 kHz
            let amp = 1.0 / (1.0 + (freq / 4000.0).powf(0.8)); // Decaimiento musical estándar
            let phase = phase_offset as f32 * 0.3 + f_idx as f32 * 0.1;
            for (i, s) in win.iter_mut().enumerate() {
                *s += amp
                    * (2.0 * std::f32::consts::PI * freq * i as f32 / sample_rate as f32 + phase)
                        .sin()
                    / 35.0
                    + next_dither();
            }
        }
        windows_8192.push(win);
    }

    let facts = FormatFacts {
        container: "FLAC".to_string(),
        codec: "FLAC 16-bit".to_string(),
        codec_type: Codec::Flac,
        sample_rate,
        bit_depth: Some(16),
        channels: 2,
        duration_ms: 240000,
        container_bitrate_kbps: Some(900),
        is_lossless_declared: true,
    };

    let output = run_dsp_analysis(
        &facts,
        &windows_8192,
        &[],
        &[],
        &[],
        0.85,
        0,
        0.0,
        false,
        None,
        false,
        false,
    );

    // Debe medir ancho de banda completo hasta Nyquist (>= 21.000 Hz)
    assert!(
        output.effective_bandwidth_hz >= 21000,
        "Lossless master should reach >= 21000 Hz, got {}",
        output.effective_bandwidth_hz
    );

    // E01 debe haber otorgado el LLR negativo de exoneración (-1.8)
    let e01 = output
        .evidences
        .iter()
        .find(|e| e.code == bdja_core::types::EvidenceCode::E01);
    assert!(e01.is_some(), "E01 evidence must be present");
    assert_eq!(
        e01.unwrap().llr,
        -1.8,
        "E01 LLR must be -1.8 for full lossless bandwidth"
    );

    // Evaluar veredicto con el motor de veredicto
    let verdict_res = bdja_verdict::evaluate_verdict(
        &facts,
        &output.evidences,
        &output.guards_triggered,
        output.is_strong_evidence_present,
    );

    assert_eq!(
        verdict_res.verdict,
        bdja_core::types::Verdict::LosslessVerified,
        "Master genuino debe alcanzar LosslessVerified! Obtenido: {:?}, score: {}, summary: {}",
        verdict_res.verdict,
        verdict_res.score_llr,
        verdict_res.summary
    );
    assert!(
        verdict_res.score_llr <= -4.0,
        "Score LLR {} debe ser <= -4.0 para certificar LosslessVerified",
        verdict_res.score_llr
    );
}

#[test]
fn test_dsp_joint_stereo_collapse() {
    let sample_rate = 44100;
    let n = 8192;
    let mut left = vec![0.0f32; n];
    let mut right = vec![0.0f32; n];

    // Low frequencies (decorrelated stereo):
    for i in 0..n {
        let t = i as f32 / sample_rate as f32;
        left[i] = (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.5;
        right[i] = (2.0 * std::f32::consts::PI * 880.0 * t).sin() * 0.5;
    }

    // High frequencies (> 12 kHz) (collapsed into identical mono channel, intensity stereo):
    for i in 0..n {
        let t = i as f32 / sample_rate as f32;
        let hf = (2.0 * std::f32::consts::PI * 14000.0 * t).sin() * 0.3;
        left[i] += hf;
        right[i] += hf; // Identical!
    }

    let facts = FormatFacts {
        container: "WAV".to_string(),
        codec: "PCM 16-bit LE".to_string(),
        codec_type: Codec::PcmS16Le,
        sample_rate,
        bit_depth: Some(16),
        channels: 2,
        duration_ms: 180000,
        container_bitrate_kbps: Some(1411),
        is_lossless_declared: true,
    };

    let output = run_dsp_analysis(
        &facts,
        &[left.clone()],
        &[],
        &left,
        &right,
        0.8,
        0,
        0.0,
        false,
        None,
        false,
        false,
    );

    let e07 = output
        .evidences
        .iter()
        .find(|e| e.code == bdja_core::types::EvidenceCode::E07);
    assert!(e07.is_some(), "E07 must be present");
    let e07_ev = e07.unwrap();
    assert!(e07_ev.applicable, "E07 must be applicable for stereo");
    assert!(
        e07_ev.llr > 2.0,
        "E07 LLR {} must be > 2.0 for intensity stereo collapse",
        e07_ev.llr
    );
    assert!(
        output.is_strong_evidence_present,
        "Joint stereo collapse must count as strong evidence"
    );
}

#[test]
fn test_dsp_mp3_320_transcode_is_convicted() {
    // P0-9 & AUDIT v3.0: Un MP3 320 convertido a WAV (corte a ~20.5 kHz) NUNCA debe salir LosslessVerified
    let sample_rate = 44100;

    // Sintetizar mezcla con armónicos densos hasta 20.5 kHz y silencio/filtro digital por encima
    let mut windows_8192 = Vec::new();
    for phase_offset in 0..4 {
        let mut win = vec![0.0f32; 8192];
        for f_idx in 1..=80 {
            let freq = 200.0 + f_idx as f32 * 253.0; // Armónicos densos hasta 20.440 Hz
            let phase = phase_offset as f32 * 0.4 + f_idx as f32 * 0.2;
            let amp = 1.0 / (1.0 + (freq / 3500.0).powf(0.8));
            for (i, s) in win.iter_mut().enumerate() {
                *s += amp
                    * (2.0 * std::f32::consts::PI * freq * i as f32 / sample_rate as f32 + phase)
                        .sin()
                    / 30.0;
            }
        }
        windows_8192.push(win);
    }

    let facts = FormatFacts {
        container: "WAV".to_string(),
        codec: "PCM 16-bit LE".to_string(),
        codec_type: Codec::PcmS16Le,
        sample_rate,
        bit_depth: Some(16),
        channels: 2,
        duration_ms: 180000,
        container_bitrate_kbps: Some(1411),
        is_lossless_declared: true, // Declarado engañosamente como WAV sin pérdida
    };

    let output = run_dsp_analysis(
        &facts,
        &windows_8192,
        &[],
        &[],
        &[],
        0.85,
        0,
        0.0,
        false,
        None,
        false,
        false,
    );

    // 1. Debe haber detectado el corte brickwall
    let e01 = output
        .evidences
        .iter()
        .find(|e| e.code == bdja_core::types::EvidenceCode::E01)
        .unwrap();
    assert!(
        e01.llr > 0.0,
        "E01 debe penalizar con LLR positivo (+1.2), obtenido: {}",
        e01.llr
    );

    // 2. E02 debe ser inaplicable ante corte brick-wall (B-6: no duplicar E01)
    let e02 = output
        .evidences
        .iter()
        .find(|e| e.code == bdja_core::types::EvidenceCode::E02)
        .unwrap();
    assert!(
        !e02.applicable,
        "E02 debe ser inaplicable ante corte brick-wall para no duplicar E01"
    );
    assert_eq!(
        e02.llr, 0.0,
        "E02 debe aportar LLR=0.0 ante corte brick-wall"
    );

    // 3. Evaluar veredicto
    let verdict_res = bdja_verdict::evaluate_verdict(
        &facts,
        &output.evidences,
        &output.guards_triggered,
        output.is_strong_evidence_present,
    );

    // Aserción positiva (§08 pendientes): Debe condenarse positivamente
    assert_ne!(
        verdict_res.verdict,
        bdja_core::types::Verdict::LosslessVerified,
        "CRÍTICO P0-9: MP3 320 no puede ser calificado como LosslessVerified! Score: {}",
        verdict_res.score_llr
    );
    assert_ne!(
        verdict_res.verdict,
        bdja_core::types::Verdict::LikelyLossless,
        "MP3 320 no puede ser calificado como LikelyLossless! Score: {}",
        verdict_res.score_llr
    );
    assert!(
        verdict_res.score_llr >= 2.7,
        "Score LLR {} debe ser >= 2.7 para un transcode de 320 kbps",
        verdict_res.score_llr
    );
    assert!(
        verdict_res.verdict == bdja_core::types::Verdict::ProbableTranscode || verdict_res.verdict == bdja_core::types::Verdict::Suspicious,
        "MP3 320 debe ser clasificado positivamente como ProbableTranscode o Suspicious! Obtenido: {:?}",
        verdict_res.verdict
    );
}

#[test]
fn test_dsp_mp3_192_transcode_positive_conviction() {
    // §08: Validación positiva para MP3 192 transcodeado a WAV (corte a ~19.0 kHz)
    let sample_rate = 44100;

    let mut windows_8192 = Vec::new();
    for phase_offset in 0..4 {
        let mut win = vec![0.0f32; 8192];
        for f_idx in 1..=75 {
            let freq = 200.0 + f_idx as f32 * 250.0; // Armónicos densos hasta 18.950 Hz
            let phase = phase_offset as f32 * 0.4 + f_idx as f32 * 0.2;
            let amp = 1.0 / (1.0 + (freq / 3000.0).powf(0.8));
            for (i, s) in win.iter_mut().enumerate() {
                *s += amp
                    * (2.0 * std::f32::consts::PI * freq * i as f32 / sample_rate as f32 + phase)
                        .sin()
                    / 30.0;
            }
        }
        windows_8192.push(win);
    }

    let facts = FormatFacts {
        container: "WAV".to_string(),
        codec: "PCM 16-bit LE".to_string(),
        codec_type: Codec::PcmS16Le,
        sample_rate,
        bit_depth: Some(16),
        channels: 2,
        duration_ms: 180000,
        container_bitrate_kbps: Some(1411),
        is_lossless_declared: true,
    };

    let output = run_dsp_analysis(
        &facts,
        &windows_8192,
        &[],
        &[],
        &[],
        0.85,
        0,
        0.0,
        false,
        None,
        false,
        false,
    );

    let e01 = output
        .evidences
        .iter()
        .find(|e| e.code == bdja_core::types::EvidenceCode::E01)
        .unwrap();
    assert!(
        e01.llr >= 1.6,
        "E01 debe aportar >= 1.6 para MP3 192, obtenido: {}",
        e01.llr
    );

    let verdict_res = bdja_verdict::evaluate_verdict(
        &facts,
        &output.evidences,
        &output.guards_triggered,
        output.is_strong_evidence_present,
    );

    assert!(
        verdict_res.score_llr >= 2.7,
        "Score LLR {} debe ser >= 2.7 para MP3 192",
        verdict_res.score_llr
    );
    assert!(
        verdict_res.verdict == bdja_core::types::Verdict::ProbableTranscode
            || verdict_res.verdict == bdja_core::types::Verdict::Suspicious,
        "MP3 192 debe ser condenado como ProbableTranscode o Suspicious! Obtenido: {:?}",
        verdict_res.verdict
    );
}

#[test]
fn test_dsp_aac_256_transcode_positive_conviction() {
    // §08: Validación positiva para AAC 256 transcodeado a WAV (corte a ~19.5 kHz)
    let sample_rate = 44100;

    let mut windows_8192 = Vec::new();
    for phase_offset in 0..4 {
        let mut win = vec![0.0f32; 8192];
        for f_idx in 1..=77 {
            let freq = 200.0 + f_idx as f32 * 253.0; // Armónicos hasta 19.680 Hz
            let phase = phase_offset as f32 * 0.3 + f_idx as f32 * 0.15;
            let amp = 1.0 / (1.0 + (freq / 3500.0).powf(0.8));
            for (i, s) in win.iter_mut().enumerate() {
                *s += amp
                    * (2.0 * std::f32::consts::PI * freq * i as f32 / sample_rate as f32 + phase)
                        .sin()
                    / 30.0;
            }
        }
        windows_8192.push(win);
    }

    let facts = FormatFacts {
        container: "WAV".to_string(),
        codec: "PCM 16-bit LE".to_string(),
        codec_type: Codec::PcmS16Le,
        sample_rate,
        bit_depth: Some(16),
        channels: 2,
        duration_ms: 180000,
        container_bitrate_kbps: Some(1411),
        is_lossless_declared: true,
    };

    let output = run_dsp_analysis(
        &facts,
        &windows_8192,
        &[],
        &[],
        &[],
        0.85,
        0,
        0.0,
        false,
        None,
        false,
        false,
    );

    let verdict_res = bdja_verdict::evaluate_verdict(
        &facts,
        &output.evidences,
        &output.guards_triggered,
        output.is_strong_evidence_present,
    );

    assert!(
        verdict_res.score_llr >= 2.7,
        "Score LLR {} debe ser >= 2.7 para AAC 256",
        verdict_res.score_llr
    );
    assert!(
        verdict_res.verdict == bdja_core::types::Verdict::ProbableTranscode
            || verdict_res.verdict == bdja_core::types::Verdict::Suspicious,
        "AAC 256 debe ser condenado como ProbableTranscode o Suspicious! Obtenido: {:?}",
        verdict_res.verdict
    );
}

#[test]
fn test_dsp_dark_mix_mp3_128_detected() {
    // §03 v3.0: Mezcla oscura (-15 dB/oct) con corte de MP3 128 (16 kHz) debe ser detectada
    let sample_rate = 44100;

    let mut windows_8192 = Vec::new();
    for phase_offset in 0..4 {
        let mut win = vec![0.0f32; 8192];
        for f_khz in 1..=16 {
            let freq = f_khz as f32 * 1000.0;
            // Mezcla oscura con roll-off pronunciado
            let amp = 1.0 / (1.0 + (freq / 1500.0).powf(1.8));
            let phase = phase_offset as f32 * 0.5;
            for (i, s) in win.iter_mut().enumerate() {
                *s += amp
                    * (2.0 * std::f32::consts::PI * freq * i as f32 / sample_rate as f32 + phase)
                        .sin()
                    / 15.0;
            }
        }
        windows_8192.push(win);
    }

    let facts = FormatFacts {
        container: "WAV".to_string(),
        codec: "PCM 16-bit LE".to_string(),
        codec_type: Codec::PcmS16Le,
        sample_rate,
        bit_depth: Some(16),
        channels: 2,
        duration_ms: 180000,
        container_bitrate_kbps: Some(1411),
        is_lossless_declared: true,
    };

    let output = run_dsp_analysis(
        &facts,
        &windows_8192,
        &[],
        &[],
        &[],
        0.8,
        0,
        0.0,
        false,
        None,
        false,
        false,
    );

    // Debe detectar el corte a ~16 kHz
    assert!(
        output.effective_bandwidth_hz <= 16500 && output.effective_bandwidth_hz >= 15500,
        "Corte en mezcla oscura debe ser detectado a ~16000 Hz, obtenido: {}",
        output.effective_bandwidth_hz
    );
    let e01 = output
        .evidences
        .iter()
        .find(|e| e.code == bdja_core::types::EvidenceCode::E01)
        .unwrap();
    assert_eq!(
        e01.llr, 2.2,
        "E01 debe aportar LLR +2.2 para corte a 16 kHz"
    );
}

#[test]
fn test_dsp_dark_mix_lossless_exonerated() {
    // §03 v3.0: Mezcla oscura genuina (sin corte brick-wall) debe exonerarse con la guarda
    let sample_rate = 44100;

    let mut rng: u32 = 99999;
    let mut next_dither = || -> f32 {
        rng = rng.wrapping_mul(1103515245).wrapping_add(12345);
        (((rng % 20000) as f32 / 10000.0) - 1.0) * 0.00002 // dither floor ~ -94 dBFS
    };

    let mut windows_8192 = Vec::new();
    for phase_offset in 0..4 {
        let mut win = vec![0.0f32; 8192];
        for f_khz in 1..=22 {
            let freq = f_khz as f32 * 1000.0;
            // Roll-off acústico pronunciado continuo (-18 dB/oct) sin salto brickwall
            let amp = 1.0 / (1.0 + (freq / 1500.0).powf(3.0));
            let phase = phase_offset as f32 * 0.5;
            for (i, s) in win.iter_mut().enumerate() {
                *s += amp
                    * (2.0 * std::f32::consts::PI * freq * i as f32 / sample_rate as f32 + phase)
                        .sin()
                    / 15.0
                    + next_dither();
            }
        }
        windows_8192.push(win);
    }

    let facts = FormatFacts {
        container: "FLAC".to_string(),
        codec: "FLAC 16-bit".to_string(),
        codec_type: Codec::Flac,
        sample_rate,
        bit_depth: Some(16),
        channels: 2,
        duration_ms: 180000,
        container_bitrate_kbps: Some(850),
        is_lossless_declared: true,
    };

    let output = run_dsp_analysis(
        &facts,
        &windows_8192,
        &[],
        &[],
        &[],
        0.8,
        0,
        0.0,
        false,
        None,
        false,
        false,
    );

    // E01 no debe penalizar (LLR = 0.0)
    let e01 = output
        .evidences
        .iter()
        .find(|e| e.code == bdja_core::types::EvidenceCode::E01)
        .unwrap();
    assert_eq!(
        e01.llr, 0.0,
        "Mezcla oscura sin corte artificial no debe penalizarse en E01"
    );

    // Debe activar la guarda de material acústico limitado
    assert!(
        output
            .guards_triggered
            .iter()
            .any(|g| g.contains("ancho de banda limitado")),
        "Debe activar la guarda de material acústico limitado"
    );
}

#[test]
fn test_dsp_resampled_96k_master_is_not_convicted_as_transcode() {
    let sample_rate = 96000;

    // Máster de 44.1 kHz subido a 96 kHz: contenido musical hasta 22.05 kHz,
    // y caída abrupta después (corte de Nyquist de 44.1k en un contenedor de 96k)
    let mut rng: u32 = 987654321;
    let mut next_dither = || -> f32 {
        rng = rng.wrapping_mul(1103515245).wrapping_add(12345);
        (((rng % 20000) as f32 / 10000.0) - 1.0) * 0.00001
    };

    let mut windows_8192 = Vec::new();
    for phase_offset in 0..6 {
        let mut win = vec![0.0f32; 8192];
        // Armónicos densos hasta 21.8 kHz
        for f_idx in 1..=80 {
            let freq = 100.0 + f_idx as f32 * 270.0; // hasta ~21.7 kHz
            let amp = 1.0 / (1.0 + (freq / 4000.0).powf(0.8));
            let phase = phase_offset as f32 * 0.3 + f_idx as f32 * 0.1;
            for (i, s) in win.iter_mut().enumerate() {
                *s += amp
                    * (2.0 * std::f32::consts::PI * freq * i as f32 / sample_rate as f32 + phase)
                        .sin()
                    / 30.0
                    + next_dither();
            }
        }
        windows_8192.push(win);
    }

    let facts = FormatFacts {
        container: "FLAC".to_string(),
        codec: "FLAC 24-bit".to_string(),
        codec_type: Codec::Flac,
        sample_rate,
        bit_depth: Some(24),
        channels: 2,
        duration_ms: 240000,
        container_bitrate_kbps: Some(2800),
        is_lossless_declared: true,
    };

    let output = run_dsp_analysis(
        &facts,
        &windows_8192,
        &[],
        &[],
        &[],
        0.8,
        0,
        0.0,
        false,
        None,
        false,
        false,
    );

    let e01 = output
        .evidences
        .iter()
        .find(|e| e.code == bdja_core::types::EvidenceCode::E01)
        .unwrap();
    let e10 = output
        .evidences
        .iter()
        .find(|e| e.code == bdja_core::types::EvidenceCode::E10)
        .unwrap();
    let e11 = output
        .evidences
        .iter()
        .find(|e| e.code == bdja_core::types::EvidenceCode::E11)
        .unwrap();

    // E01 y E11 deben ser neutrales respecto a transcode
    assert_eq!(
        e01.llr, 0.0,
        "E01 debe ser neutral (0.0 LLR) ante remuestreo de tasa estándar"
    );
    assert_eq!(
        e11.llr, 0.0,
        "E11 debe ser neutral (0.0 LLR) ante remuestreo de tasa estándar"
    );

    // E10 debe acusar Falso Hi-Res / Upsampling positivamente
    assert!(e10.llr >= 2.0, "E10 debe detectar falso Hi-Res");

    // Y el veredicto final no debe condenar como transcode de MP3
    let verdict_out = bdja_verdict::evaluate_verdict(
        &facts,
        &output.evidences,
        &output.guards_triggered,
        output.is_strong_evidence_present,
    );

    assert_ne!(
        verdict_out.verdict,
        bdja_core::types::Verdict::ProbableTranscode,
        "Un máster remuestreado a 96k NUNCA debe condenarse como ProbableTranscode de MP3"
    );
}

#[test]
fn test_dsp_mp3_128_resampled_to_96k_is_convicted_as_transcode() {
    let sample_rate = 96000;

    // MP3 128 kbps (corte en ~16 kHz) remuestreado a 96 kHz
    let mut windows_8192 = Vec::new();
    for phase_offset in 0..4 {
        let mut win = vec![0.0f32; 8192];
        for f_khz in 1..=16 {
            let freq = f_khz as f32 * 1000.0;
            let phase = phase_offset as f32 * 0.785;
            for (i, s) in win.iter_mut().enumerate() {
                *s += (2.0 * std::f32::consts::PI * freq * i as f32 / sample_rate as f32 + phase)
                    .sin()
                    / 16.0;
            }
        }
        windows_8192.push(win);
    }

    let facts = FormatFacts {
        container: "FLAC".to_string(),
        codec: "FLAC 24-bit".to_string(),
        codec_type: Codec::Flac,
        sample_rate,
        bit_depth: Some(24),
        channels: 2,
        duration_ms: 240000,
        container_bitrate_kbps: Some(2800),
        is_lossless_declared: true,
    };

    let output = run_dsp_analysis(
        &facts,
        &windows_8192,
        &[],
        &[],
        &[],
        0.8,
        0,
        0.0,
        true, // Tiene firma LAME detectada
        Some("LAME3.99r"),
        false,
        false,
    );

    let e01 = output
        .evidences
        .iter()
        .find(|e| e.code == bdja_core::types::EvidenceCode::E01)
        .unwrap();

    // No debe ser clasificado como remuestreo puro de estudio (E01 debe condenar)
    assert!(
        e01.llr >= 1.5,
        "E01 debe condenar corte de 16 kHz como transcode (obtenido: {})",
        e01.llr
    );
    assert!(
        output.guards_triggered.is_empty(),
        "No debe haber guardas de remuestreo bloqueando la condena"
    );

    let verdict_out = bdja_verdict::evaluate_verdict(
        &facts,
        &output.evidences,
        &output.guards_triggered,
        output.is_strong_evidence_present,
    );

    assert_eq!(
        verdict_out.verdict,
        bdja_core::types::Verdict::ProbableTranscode,
        "Un MP3 128 subido a 96k DEBE ser condenado como ProbableTranscode. Obtenido: {:?}, score: {}",
        verdict_out.verdict,
        verdict_out.score_llr
    );
}

#[test]
fn test_dsp_e05_inapplicable_when_full_spectrum() {
    let sample_rate = 44100;
    let mut windows_8192 = Vec::new();

    let mut rng: u32 = 42424242;
    let mut next_dither = || -> f32 {
        rng = rng.wrapping_mul(1103515245).wrapping_add(12345);
        (((rng % 20000) as f32 / 10000.0) - 1.0) * 0.00003
    };

    // Generar ventanas con espectro continuo hasta Nyquist (22.05 kHz)
    // e inyectar periodicidad a 576 muestras para verificar que E05 se neutraliza
    for phase_offset in 0..6 {
        let mut win = vec![0.0f32; 8192];
        for f_idx in 1..=100 {
            let freq = 100.0 + f_idx as f32 * 218.0; // Distribuido densamente hasta 21.9 kHz
            let amp = 1.0 / (1.0 + (freq / 4000.0).powf(0.8));
            let phase = phase_offset as f32 * 0.3 + f_idx as f32 * 0.1;
            for (i, s) in win.iter_mut().enumerate() {
                let mod_576 = if i % 576 < 288 { 0.005 } else { -0.005 };
                *s += amp
                    * (2.0 * std::f32::consts::PI * freq * i as f32 / sample_rate as f32 + phase)
                        .sin()
                    / 35.0
                    + mod_576
                    + next_dither();
            }
        }
        windows_8192.push(win);
    }

    let facts = FormatFacts {
        container: "WAV".to_string(),
        codec: "PCM 16-bit".to_string(),
        codec_type: Codec::PcmS16Le,
        sample_rate,
        bit_depth: Some(16),
        channels: 2,
        duration_ms: 10000,
        container_bitrate_kbps: Some(1411),
        is_lossless_declared: true,
    };

    let output = run_dsp_analysis(
        &facts,
        &windows_8192,
        &[],
        &[],
        &[],
        0.8,
        0,
        0.0,
        false,
        None,
        false,
        false,
    );

    let e05 = output
        .evidences
        .iter()
        .find(|e| e.code == bdja_core::types::EvidenceCode::E05)
        .unwrap();

    assert!(
        !e05.applicable,
        "E05 debe ser INAPLICABLE cuando el espectro es continuo hasta Nyquist (FullSpectrum)"
    );
    assert_eq!(
        e05.llr, 0.0,
        "E05 no debe sumar LLR positivo ante FullSpectrum"
    );

    let verdict_out = bdja_verdict::evaluate_verdict(
        &facts,
        &output.evidences,
        &output.guards_triggered,
        output.is_strong_evidence_present,
    );

    assert_ne!(
        verdict_out.verdict,
        bdja_core::types::Verdict::Inconclusive,
        "El material con espectro continuo no debe caer en Inconclusive debido a E05"
    );
}

#[test]
fn test_dsp_adaptive_nyquist_cliff_detection_high_frequencies() {
    let sample_rate = 44100;
    let mut windows_8192 = Vec::new();

    // Simular un corte empinado a 19.6 kHz (caso latin_transcode_06)
    let cutoff_hz = 19633.0f32;
    for _ in 0..6 {
        let mut win = vec![0.0f32; 8192];
        for (i, s) in win.iter_mut().enumerate() {
            let t = i as f32 / sample_rate as f32;
            let mut val = 0.0f32;
            for f_idx in 1..=80 {
                let freq = 200.0 + f_idx as f32 * (cutoff_hz / 85.0);
                if freq <= cutoff_hz {
                    val += (2.0 * std::f32::consts::PI * freq * t).sin() / 40.0;
                }
            }
            *s = val;
        }
        windows_8192.push(win);
    }

    let facts = FormatFacts {
        container: "WAV".to_string(),
        codec: "PCM 16-bit".to_string(),
        codec_type: Codec::PcmS16Le,
        sample_rate,
        bit_depth: Some(16),
        channels: 2,
        duration_ms: 10000,
        container_bitrate_kbps: Some(1411),
        is_lossless_declared: true,
    };

    let output = run_dsp_analysis(
        &facts,
        &windows_8192,
        &[],
        &[],
        &[],
        0.8,
        0,
        0.0,
        false,
        None,
        false,
        false,
    );

    let e01 = output
        .evidences
        .iter()
        .find(|e| e.code == bdja_core::types::EvidenceCode::E01)
        .unwrap();

    assert!(
        e01.llr >= 1.4,
        "E01 debe condenar el corte brickwall a 19.6 kHz (obtenido: {})",
        e01.llr
    );
    assert!(
        output.cutoff_slope_db_oct >= 45.0,
        "El corte a 19.6 kHz debe ser clasificado con pendiente abrupta (obtenido: {} dB/oct)",
        output.cutoff_slope_db_oct
    );
}

#[test]
fn test_dsp_e02_slope_governs_natural_rolloff_penalty() {
    // B-4: Pendiente de 86.7 dB/oct en NaturalRolloff no debe recibir -1.0 de exoneración
    let _facts = bdja_core::types::FormatFacts {
        container: "WAV".to_string(),
        codec: "PCM".to_string(),
        codec_type: bdja_core::types::Codec::PcmS16Le,
        sample_rate: 44100,
        bit_depth: Some(16),
        channels: 2,
        duration_ms: 10000,
        container_bitrate_kbps: Some(1411),
        is_lossless_declared: true,
    };

    // Crear un fake SpectrumAnalysis con NaturalRolloff y 86.7 dB/oct
    let _spec = bdja_dsp::spectrum::SpectrumAnalysis {
        cutoff_kind: bdja_core::types::CutoffKind::NaturalRolloff,
        cutoff_frequency_hz: None,
        effective_bandwidth_hz: 18798,
        cutoff_slope_db_oct: 86.7,
        content_bandwidth_hz: 18798,
        average_spectrum_db: vec![-50.0; 256],
        shelf_16k_drop_db: 0.0,
        spectral_holes_ratio: 0.0,
        upsampling_detected: false,
    };

    // Comprobar la lógica de E02 en pipeline (B-6: solo aplicable en NaturalRolloff)
    let eval_e02 = |kind: bdja_core::types::CutoffKind, slope: f64| -> (f64, String, bool) {
        match kind {
            bdja_core::types::CutoffKind::FullSpectrum => (0.0, String::new(), false),
            bdja_core::types::CutoffKind::BrickwallCutoff => (
                0.0,
                "Corte brick-wall evaluado por E01 (E02 no duplica la evidencia)".to_string(),
                false,
            ),
            bdja_core::types::CutoffKind::NaturalRolloff => {
                if slope <= 24.0 {
                    (-1.0, "Roll-off suave".to_string(), true)
                } else if slope <= 40.0 {
                    (0.0, "Roll-off moderado".to_string(), true)
                } else if slope < 60.0 {
                    (0.8, "Pendiente pronunciada".to_string(), true)
                } else {
                    (1.5, "Pendiente vertical brick-wall".to_string(), true)
                }
            }
        }
    };

    // 1. NaturalRolloff con pendiente empinada (86.7 dB/oct) -> Condena con LLR +1.5
    let (llr_steep, desc_steep, app_steep) =
        eval_e02(bdja_core::types::CutoffKind::NaturalRolloff, 86.7);
    assert!(app_steep);
    assert_eq!(
        llr_steep, 1.5,
        "Pendiente de 86.7 dB/oct en NaturalRolloff debe condenar con +1.5"
    );
    assert!(desc_steep.contains("brick-wall"));

    // 2. NaturalRolloff con pendiente suave (18.0 dB/oct) -> Exonera con LLR -1.0
    let (llr_gentle, _, app_gentle) = eval_e02(bdja_core::types::CutoffKind::NaturalRolloff, 18.0);
    assert!(app_gentle);
    assert_eq!(
        llr_gentle, -1.0,
        "Pendiente acústica de 18 dB/oct debe exonerar con -1.0"
    );

    // 3. BrickwallCutoff -> Inaplicable para no duplicar E01 (B-6)
    let (llr_bw, _, app_bw) = eval_e02(bdja_core::types::CutoffKind::BrickwallCutoff, 95.0);
    assert!(!app_bw, "E02 debe ser inaplicable ante BrickwallCutoff");
    assert_eq!(llr_bw, 0.0, "E02 no debe sumar LLR ante BrickwallCutoff");
}

#[test]
fn test_dsp_mp3_320_nyquist_cliff_not_exonerated_as_full_spectrum() {
    // B-5 / Caso de regresión para latin_transcode_14:
    // MP3 320 kbps con corte a ~20 kHz y piso de cuantización a -65 dBFS.
    // NUNCA debe ser clasificado como FullSpectrum ni alcanzar LosslessVerified.
    let sample_rate = 44100;
    let cutoff_hz = 19950.0;

    let mut windows_8192 = Vec::new();
    for win_idx in 0..6 {
        let mut win = vec![0.0f32; 8192];
        for (i, s) in win.iter_mut().enumerate() {
            let t = (win_idx * 4096 + i) as f32 / sample_rate as f32;
            let mut val = 0.0f32;
            for f_idx in 1..=80 {
                let freq = 200.0 + f_idx as f32 * (cutoff_hz / 80.0);
                if freq <= cutoff_hz {
                    val += (2.0 * std::f32::consts::PI * freq * t).sin() / 35.0;
                }
            }
            // Piso de cuantización residual típico de MP3 a -65 dBFS
            let noise = (((i * 73 + win_idx * 17) % 1000) as f32 / 1000.0 - 0.5) * 0.001;
            *s = val + noise;
        }
        windows_8192.push(win);
    }

    let facts = FormatFacts {
        container: "WAV".to_string(),
        codec: "PCM 16-bit".to_string(),
        codec_type: Codec::PcmS16Le,
        sample_rate,
        bit_depth: Some(16),
        channels: 2,
        duration_ms: 180000,
        container_bitrate_kbps: Some(1411),
        is_lossless_declared: true,
    };

    let output = run_dsp_analysis(
        &facts,
        &windows_8192,
        &[],
        &[],
        &[],
        0.8,
        0,
        0.0,
        false,
        None,
        false,
        false,
    );

    // No debe reportar espectro completo
    let e01 = output
        .evidences
        .iter()
        .find(|e| e.code == bdja_core::types::EvidenceCode::E01)
        .unwrap();
    assert!(
        e01.llr > 0.0,
        "E01 debe penalizar el corte a 20 kHz (obtenido LLR: {})",
        e01.llr
    );

    let verdict_out = bdja_verdict::evaluate_verdict(
        &facts,
        &output.evidences,
        &output.guards_triggered,
        output.is_strong_evidence_present,
    );

    assert_ne!(
        verdict_out.verdict,
        bdja_core::types::Verdict::LosslessVerified,
        "Un MP3 320 jamás debe ser clasificado como LosslessVerified"
    );
    assert!(
        verdict_out.score_llr >= 0.0,
        "El score LLR debe ser no-negativo para un transcode a 320 kbps (obtenido: {})",
        verdict_out.score_llr
    );
}

#[test]
fn test_dsp_average_spectrum_db_downsampling_computes_mean_power() {
    let sample_rate = 44100;

    let mut windows_8192 = Vec::new();
    for win_idx in 0..4 {
        let mut win = vec![0.0f32; 8192];
        for f_khz in 1..=15 {
            let freq = f_khz as f32 * 1000.0;
            let phase = win_idx as f32 * 0.5;
            for (i, s) in win.iter_mut().enumerate() {
                *s += (2.0 * std::f32::consts::PI * freq * i as f32 / sample_rate as f32 + phase)
                    .sin()
                    / 15.0;
            }
        }
        windows_8192.push(win);
    }

    let facts = FormatFacts {
        container: "WAV".to_string(),
        codec: "PCM 16-bit LE".to_string(),
        codec_type: Codec::PcmS16Le,
        sample_rate,
        bit_depth: Some(16),
        channels: 2,
        duration_ms: 5000,
        container_bitrate_kbps: Some(1411),
        is_lossless_declared: true,
    };

    let empty_1024: Vec<Vec<f32>> = Vec::new();
    let output = run_dsp_analysis(
        &facts,
        &windows_8192,
        &empty_1024,
        &windows_8192[0],
        &windows_8192[0],
        0.8,
        0,
        0.0,
        false,
        None,
        false,
        false,
    );

    // 1. CutoffKind expuesto
    assert_eq!(
        output.cutoff_kind,
        bdja_core::types::CutoffKind::BrickwallCutoff
    );

    // 2. average_spectrum_db contiene 256 puntos
    assert_eq!(output.average_spectrum_db.len(), 256);

    // 3. Banda activa (hasta ~15 kHz) vs banda muerta (> 16 kHz)
    let nyquist = sample_rate as f32 / 2.0;
    let idx_10k = ((10000.0 / nyquist) * 256.0) as usize;
    let idx_18k = ((18000.0 / nyquist) * 256.0) as usize;

    let db_passband = output.average_spectrum_db[idx_10k];
    let db_stopband = output.average_spectrum_db[idx_18k];

    // La banda de paso tiene señal plena
    assert!(
        db_passband > -10.0,
        "Passband dB debe tener alta energía, obtenido: {}",
        db_passband
    );

    // La banda atenuada debe registrar la caída profunda (> 50 dB de atenuación)
    assert!(
        db_stopband < -50.0,
        "Stopband dB debe estar atenuado (< -50 dB), obtenido: {}",
        db_stopband
    );
    assert!(
        db_passband - db_stopband > 50.0,
        "La caída espectral promedio debe ser nítida (> 50 dB), obtenido: {}",
        db_passband - db_stopband
    );
}
