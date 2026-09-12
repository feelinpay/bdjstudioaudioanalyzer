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
                *s += (2.0 * std::f32::consts::PI * freq * i as f32 / sample_rate as f32 + phase).sin() / 16.0;
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

    // Synthesize audio with natural acoustic gentle slope (-12 dB/oct roll-off starting at 8 kHz)
    let mut windows_8192 = Vec::new();
    for phase_offset in 0..4 {
        let mut win = vec![0.0f32; 8192];
        for f_khz in 1..=18 {
            let freq = f_khz as f32 * 1000.0;
            let amp = if f_khz <= 8 {
                1.0
            } else {
                1.0 / (1.0 + ((f_khz - 8) as f32).powi(2) * 0.4)
            };
            let phase = phase_offset as f32 * 0.5;
            for (i, s) in win.iter_mut().enumerate() {
                *s += amp * (2.0 * std::f32::consts::PI * freq * i as f32 / sample_rate as f32 + phase).sin() / 15.0;
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

    // Natural gentle rolloff must trigger band-limited guard if bandwidth is restricted
    if output.effective_bandwidth_hz < 20000 {
        assert!(
            output.guards_triggered.iter().any(|g| g.contains("ancho de banda limitado")),
            "Should have triggered band-limited guard for gentle slope"
        );
    }
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

    let e07 = output.evidences.iter().find(|e| e.code == bdja_core::types::EvidenceCode::E07);
    assert!(e07.is_some(), "E07 must be present");
    let e07_ev = e07.unwrap();
    assert!(e07_ev.applicable, "E07 must be applicable for stereo");
    assert!(e07_ev.llr > 2.0, "E07 LLR {} must be > 2.0 for intensity stereo collapse", e07_ev.llr);
    assert!(output.is_strong_evidence_present, "Joint stereo collapse must count as strong evidence");
}