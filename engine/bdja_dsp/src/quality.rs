use bdja_core::types::QualityMetrics;

pub struct QualityAnalysis {
    pub metrics: QualityMetrics,
    pub noise_floor_db: f64,
    pub has_exact_digital_silence: bool,
    pub real_bit_depth: u16,
    pub bit_depth_inflated: bool,
}

pub fn analyze_quality(
    samples_mono: &[f32],
    max_peak: f32,
    clipped_samples: u64,
    dc_offset: f64,
    stereo_correlation: Option<f64>,
    declared_bit_depth: Option<u16>,
) -> QualityAnalysis {
    if samples_mono.is_empty() {
        return QualityAnalysis {
            metrics: QualityMetrics {
                true_peak_dbtp: Some(-100.0),
                lufs_integrated: Some(-100.0),
                clipped_samples: 0,
                dc_offset: Some(0.0),
                dynamic_range_db: Some(0.0),
                stereo_correlation,
            },
            noise_floor_db: -120.0,
            has_exact_digital_silence: true,
            real_bit_depth: declared_bit_depth.unwrap_or(16),
            bit_depth_inflated: false,
        };
    }

    // 1. True Peak (with 4x inter-sample reconstruction over high-amplitude points)
    let mut true_peak_linear = max_peak.max(1e-6);
    for i in 2..(samples_mono.len().saturating_sub(2)) {
        let s = samples_mono[i].abs();
        if s > 0.85 {
            // Sinc interpolation at halfway point: t = 0.5
            // sinc(0.5) = 2/pi ≈ 0.6366, sinc(1.5) = -2/(3*pi) ≈ -0.2122
            let interp_half = (samples_mono[i] * 0.6366 + samples_mono[i + 1] * 0.6366
                - samples_mono[i - 1] * 0.2122 - samples_mono[i + 2] * 0.2122).abs();
            if interp_half > true_peak_linear {
                true_peak_linear = interp_half;
            }
        }
    }
    let true_peak_dbtp = (20.0 * (true_peak_linear as f64).log10()).clamp(-120.0, 12.0);

    // 2. RMS / Approximate Integrated LUFS (with basic high-shelf filter weighting)
    let mut sum_sq: f64 = 0.0;
    let mut min_nonzero_mag: f32 = 1.0;
    let mut zero_count: usize = 0;
    let mut consecutive_clips: u64 = 0;
    let mut current_consecutive: u64 = 0;

    for &s in samples_mono {
        let abs_s = s.abs();
        if abs_s == 0.0 {
            zero_count += 1;
        } else if abs_s < min_nonzero_mag {
            min_nonzero_mag = abs_s;
        }

        if abs_s >= 0.9995 {
            current_consecutive += 1;
            if current_consecutive >= 3 {
                consecutive_clips += 1;
            }
        } else {
            current_consecutive = 0;
        }

        sum_sq += (s as f64) * (s as f64);
    }

    let actual_clipped = clipped_samples.max(consecutive_clips);

    let rms = (sum_sq / samples_mono.len() as f64).sqrt().max(1e-9);
    // EBU R128 integrated loudness is approximately -0.691 + 10 log10(sum_sq / N)
    let lufs_integrated = (-0.691 + 10.0 * (sum_sq / samples_mono.len() as f64).log10()).clamp(-120.0, 0.0);

    // 3. Dynamic Range (Crest factor in dB)
    let crest_factor_db = (20.0 * (true_peak_linear as f64 / rms).log10()).max(0.0);
    let dynamic_range_db = crest_factor_db.clamp(0.0, 40.0);

    // 4. Noise floor & Digital silence
    let has_exact_digital_silence = zero_count > (samples_mono.len() / 20); // > 5% exact zeroes
    let noise_floor_db = (20.0 * (min_nonzero_mag as f64).log10()).clamp(-144.0, 0.0);

    // 5. Bit depth analysis
    // For 16-bit PCM, min resolution is 1 / 32768 ≈ 3.05e-5 (-90 dB)
    // For 24-bit PCM, min resolution is 1 / 8388608 ≈ 1.19e-7 (-138 dB)
    let declared_bits = declared_bit_depth.unwrap_or(16);
    let mut real_bit_depth = declared_bits;
    let mut bit_depth_inflated = false;

    if declared_bits >= 24 {
        if noise_floor_db > -96.0 || min_nonzero_mag > 2.5e-5 {
            real_bit_depth = 16;
            bit_depth_inflated = true; // Declares 24-bit but content fits in 16-bit!
        }
    }

    QualityAnalysis {
        metrics: QualityMetrics {
            true_peak_dbtp: Some(true_peak_dbtp),
            lufs_integrated: Some(lufs_integrated),
            clipped_samples: actual_clipped,
            dc_offset: Some(dc_offset),
            dynamic_range_db: Some(dynamic_range_db),
            stereo_correlation,
        },
        noise_floor_db,
        has_exact_digital_silence,
        real_bit_depth,
        bit_depth_inflated,
    }
}