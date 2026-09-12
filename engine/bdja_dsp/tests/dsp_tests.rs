use bdja_core::types::FormatFacts;
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
        codec: "PCM 16-bit".to_string(),
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
}