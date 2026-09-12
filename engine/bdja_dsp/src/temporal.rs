pub struct TemporalAnalysis {
    pub block_grid_peak_ratio: f64,
    pub detected_block_size: Option<u32>,
    pub pre_echo_score: f64,
    pub temporal_variance: f64,
}

pub fn analyze_temporal(
    windows_1024: &[Vec<f32>],
    windows_8192: &[Vec<f32>],
) -> TemporalAnalysis {
    let mut block_grid_peak_ratio = 0.0;
    let mut detected_block_size = None;

    // 1. E05: Block grid detection via autocorrelation
    // Flatten windows into continuous sample segment or use concatenated short windows
    let mut high_freq_envelope = Vec::new();
    for win in windows_8192.iter().take(4) {
        // High-pass filter via simple difference: x[n] - x[n-1]
        for i in 1..win.len() {
            let hf = (win[i] - win[i - 1]).abs();
            high_freq_envelope.push(hf);
        }
    }

    if high_freq_envelope.len() >= 2304 {
        let n = high_freq_envelope.len().min(8192);
        let slice = &high_freq_envelope[..n];
        let mean: f32 = slice.iter().sum::<f32>() / n as f32;

        let autocorr = |lag: usize| -> f64 {
            if lag >= n {
                return 0.0;
            }
            let mut sum = 0.0;
            for i in 0..(n - lag) {
                sum += (slice[i] - mean) as f64 * (slice[i + lag] - mean) as f64;
            }
            sum / (n - lag) as f64
        };

        let r0 = autocorr(0).max(1e-9);

        // Check MP3 lags: 576 and 1152
        let r_576 = autocorr(576) / r0;
        let r_1152 = autocorr(1152) / r0;

        // Check AAC lags: 1024
        let r_1024 = autocorr(1024) / r0;

        // Background baseline around lags
        let bg_576 = (autocorr(550) + autocorr(600)) / (2.0 * r0);
        let bg_1024 = (autocorr(980) + autocorr(1060)) / (2.0 * r0);

        let peak_mp3 = (r_576 - bg_576).max(r_1152 - bg_576);
        let peak_aac = r_1024 - bg_1024;

        if peak_mp3 > 0.08 && peak_mp3 > peak_aac {
            block_grid_peak_ratio = peak_mp3.max(0.0);
            detected_block_size = Some(576);
        } else if peak_aac > 0.08 {
            block_grid_peak_ratio = peak_aac.max(0.0);
            detected_block_size = Some(1024);
        }
    }

    // 2. E06: Pre-echo (HF energy before transients in fast 1024 windows)
    let mut max_pre_echo = 0.0;
    for win in windows_1024 {
        if win.len() >= 512 {
            // Check transient in second half
            let energy_pre: f32 = win[..256].iter().map(|s| s * s).sum();
            let energy_post: f32 = win[256..512].iter().map(|s| s * s).sum();

            if energy_post > 0.05 && energy_pre > 0.0001 {
                let ratio = (energy_pre / energy_post) as f64;
                // If pre-attack has strange HF smear
                if ratio > 0.15 && ratio < 0.85 {
                    if ratio > max_pre_echo {
                        max_pre_echo = ratio;
                    }
                }
            }
        }
    }

    // 3. E14: Temporal consistency (variance across windows)
    let mut window_energies = Vec::new();
    for win in windows_8192 {
        let e: f32 = win.iter().map(|s| s * s).sum();
        window_energies.push(e as f64);
    }

    let temporal_variance = if window_energies.len() > 1 {
        let mean = window_energies.iter().sum::<f64>() / window_energies.len() as f64;
        let var = window_energies.iter().map(|e| (e - mean).powi(2)).sum::<f64>()
            / window_energies.len() as f64;
        if mean > 1e-6 {
            (var.sqrt() / mean).min(5.0)
        } else {
            0.0
        }
    } else {
        0.0
    };

    TemporalAnalysis {
        block_grid_peak_ratio,
        detected_block_size,
        pre_echo_score: max_pre_echo,
        temporal_variance,
    }
}