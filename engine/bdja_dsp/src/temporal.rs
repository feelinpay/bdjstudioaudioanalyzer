pub struct TemporalAnalysis {
    pub block_grid_peak_ratio: f64,
    pub detected_block_size: Option<u32>,
    pub pre_echo_score: f64,
    pub temporal_variance: f64,
}

pub fn analyze_temporal(windows_1024: &[Vec<f32>], windows_8192: &[Vec<f32>]) -> TemporalAnalysis {
    let mut best_peak_mp3 = 0.0f64;
    let mut best_peak_aac = 0.0f64;

    // 1. E05: Block grid detection via autocorrelation per contiguous window
    // (Calculated independently inside each window to avoid cross-window boundary discontinuities)
    for win in windows_8192 {
        if win.len() < 2048 {
            continue;
        }
        let n = win.len().min(8192);
        let mut hf_env = Vec::with_capacity(n);
        for i in 1..n {
            hf_env.push((win[i] - win[i - 1]).abs());
        }
        let m = hf_env.len();
        if m < 1500 {
            continue;
        }
        let mean = hf_env.iter().sum::<f32>() / m as f32;

        let autocorr = |lag: usize| -> f64 {
            if lag >= m {
                return 0.0;
            }
            let mut sum = 0.0;
            for i in 0..(m - lag) {
                sum += (hf_env[i] - mean) as f64 * (hf_env[i + lag] - mean) as f64;
            }
            sum / (m - lag) as f64
        };

        let r0 = autocorr(0).max(1e-9);
        let r_576 = autocorr(576) / r0;
        let r_1152 = autocorr(1152) / r0;
        let r_1024 = autocorr(1024) / r0;

        let bg_576 = (autocorr(550) + autocorr(600)) / (2.0 * r0);
        let bg_1024 = (autocorr(980) + autocorr(1060)) / (2.0 * r0);

        let p_mp3 = (r_576 - bg_576).max(r_1152 - bg_576);
        let p_aac = r_1024 - bg_1024;

        if p_mp3 > best_peak_mp3 {
            best_peak_mp3 = p_mp3;
        }
        if p_aac > best_peak_aac {
            best_peak_aac = p_aac;
        }
    }

    let mut block_grid_peak_ratio = 0.0;
    let mut detected_block_size = None;
    if best_peak_mp3 > 0.08 && best_peak_mp3 > best_peak_aac {
        block_grid_peak_ratio = best_peak_mp3.max(0.0);
        detected_block_size = Some(576);
    } else if best_peak_aac > 0.08 {
        block_grid_peak_ratio = best_peak_aac.max(0.0);
        detected_block_size = Some(1024);
    }

    // 2. E06: Pre-echo (HF energy before transients, normalized by local window energy)
    let mut max_pre_echo = 0.0;
    for win in windows_1024 {
        if win.len() >= 512 {
            let energy_pre: f32 = win[..256].iter().map(|s| s * s).sum();
            let energy_post: f32 = win[256..512].iter().map(|s| s * s).sum();
            let energy_total = energy_pre + energy_post;

            // Attack condition: significant transient jump in second half
            if energy_total > 1e-6 && energy_post > 3.0 * energy_pre {
                let ratio = (energy_pre / energy_post) as f64;
                // If pre-attack has smear / ringing artifact
                if ratio > 0.15 && ratio < 0.85 && ratio > max_pre_echo {
                    max_pre_echo = ratio;
                }
            }
        }
    }

    // 3. E14: Temporal consistency (high-frequency spectral envelope variance across windows)
    let mut hf_ratios = Vec::new();
    for win in windows_8192 {
        let total_energy: f32 = win.iter().map(|s| s * s).sum();
        if total_energy > 1e-7 {
            // First-order discrete difference acts as high-pass filter (> 10 kHz emphasis)
            let hf_energy: f32 = win.windows(2).map(|w| (w[1] - w[0]) * (w[1] - w[0])).sum();
            hf_ratios.push((hf_energy / total_energy) as f64);
        }
    }

    let temporal_variance = if hf_ratios.len() > 1 {
        let mean = hf_ratios.iter().sum::<f64>() / hf_ratios.len() as f64;
        let var =
            hf_ratios.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / hf_ratios.len() as f64;
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
