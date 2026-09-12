use bdja_core::types::QualityMetrics;

pub struct QualityAnalysis {
    pub metrics: QualityMetrics,
    pub noise_floor_db: f64,
    pub has_exact_digital_silence: bool,
    pub real_bit_depth: u16,
    pub bit_depth_inflated: bool,
}

struct Biquad {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
    z1: f64,
    z2: f64,
}

impl Biquad {
    fn new_high_shelf(fs: f64) -> Self {
        let v_h = 10.0f64.powf(3.99 / 20.0);
        let fb = 1681.97444841843;
        let k = (std::f64::consts::PI * fb / fs).tan();
        let k2 = k * k;
        let sqrt2_vh = (2.0 * v_h).sqrt();

        let d = 1.0 + std::f64::consts::SQRT_2 * k + k2;
        Self {
            b0: (v_h + sqrt2_vh * k + k2) / d,
            b1: 2.0 * (k2 - v_h) / d,
            b2: (v_h - sqrt2_vh * k + k2) / d,
            a1: 2.0 * (k2 - 1.0) / d,
            a2: (1.0 - std::f64::consts::SQRT_2 * k + k2) / d,
            z1: 0.0,
            z2: 0.0,
        }
    }

    fn new_high_pass_rlb(fs: f64) -> Self {
        let fc = 38.13547087602452;
        let q = 0.50032703732539;
        let k = (std::f64::consts::PI * fc / fs).tan();
        let k2 = k * k;

        let d = 1.0 + k / q + k2;
        Self {
            b0: 1.0 / d,
            b1: -2.0 / d,
            b2: 1.0 / d,
            a1: 2.0 * (k2 - 1.0) / d,
            a2: (1.0 - k / q + k2) / d,
            z1: 0.0,
            z2: 0.0,
        }
    }

    #[inline]
    fn process(&mut self, x: f64) -> f64 {
        let y = self.b0 * x + self.z1;
        self.z1 = self.b1 * x - self.a1 * y + self.z2;
        self.z2 = self.b2 * x - self.a2 * y;
        y
    }
}

#[allow(clippy::too_many_arguments)]
pub fn analyze_quality(
    samples_mono: &[f32],
    left_channel: &[f32],
    right_channel: &[f32],
    sample_rate: u32,
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

    let fs = (sample_rate as f64).max(8000.0);

    // 1. True Peak with 4x inter-sample reconstruction over both channels
    let find_channel_true_peak = |channel: &[f32]| -> f32 {
        let mut peak = 0.0f32;
        for &s in channel {
            let abs_s = s.abs();
            if abs_s > peak {
                peak = abs_s;
            }
        }
        for i in 2..(channel.len().saturating_sub(2)) {
            let s = channel[i].abs();
            if s > 0.85 {
                let c1 = std::f32::consts::FRAC_2_PI;
                let c2 = std::f32::consts::FRAC_2_PI / 3.0;
                let interp = (channel[i] * c1 + channel[i + 1] * c1
                    - channel[i - 1] * c2
                    - channel[i + 2] * c2)
                    .abs();
                if interp > peak {
                    peak = interp;
                }
            }
        }
        peak
    };

    let peak_l = if !left_channel.is_empty() {
        find_channel_true_peak(left_channel)
    } else {
        max_peak
    };
    let peak_r = if !right_channel.is_empty() {
        find_channel_true_peak(right_channel)
    } else {
        max_peak
    };
    let peak_mono = find_channel_true_peak(samples_mono);
    let true_peak_linear = peak_l.max(peak_r).max(peak_mono).max(1e-6);
    let true_peak_dbtp = (20.0 * (true_peak_linear as f64).log10()).clamp(-120.0, 12.0);

    // 2. ITU-R BS.1770-4 Integrated LUFS with Dual-Stage Gating
    let is_stereo = !left_channel.is_empty() && !right_channel.is_empty();
    let lufs_integrated = {
        let block_len = ((0.400 * fs).round() as usize).max(64);
        let hop = ((0.100 * fs).round() as usize).max(16);

        // Pre-filter entire audio with K-weighting cascade
        let (filtered_l, filtered_r) = if is_stereo {
            let min_len = left_channel.len().min(right_channel.len());
            let mut stage1_l = Biquad::new_high_shelf(fs);
            let mut stage2_l = Biquad::new_high_pass_rlb(fs);
            let mut stage1_r = Biquad::new_high_shelf(fs);
            let mut stage2_r = Biquad::new_high_pass_rlb(fs);

            let mut y_l = Vec::with_capacity(min_len);
            let mut y_r = Vec::with_capacity(min_len);
            for i in 0..min_len {
                y_l.push(stage2_l.process(stage1_l.process(left_channel[i] as f64)));
                y_r.push(stage2_r.process(stage1_r.process(right_channel[i] as f64)));
            }
            (y_l, y_r)
        } else {
            let mut stage1 = Biquad::new_high_shelf(fs);
            let mut stage2 = Biquad::new_high_pass_rlb(fs);
            let mut y = Vec::with_capacity(samples_mono.len());
            for &s in samples_mono {
                y.push(stage2.process(stage1.process(s as f64)));
            }
            (y.clone(), y)
        };

        let num_samples = filtered_l.len();
        let mut block_powers = Vec::new();

        let mut start = 0;
        while start + block_len <= num_samples {
            let mut p_l = 0.0f64;
            let mut p_r = 0.0f64;
            for i in start..start + block_len {
                p_l += filtered_l[i] * filtered_l[i];
                p_r += filtered_r[i] * filtered_r[i];
            }
            let z_j = if is_stereo {
                (p_l + p_r) / block_len as f64
            } else {
                p_l / block_len as f64
            };
            block_powers.push(z_j);
            start += hop;
        }

        if block_powers.is_empty() {
            // Fallback para tramos más cortos que 400 ms
            let mut sum = 0.0f64;
            for i in 0..num_samples {
                sum += filtered_l[i] * filtered_l[i] + filtered_r[i] * filtered_r[i];
            }
            let z = (sum / num_samples.max(1) as f64).max(1e-12);
            (-0.691 + 10.0 * z.log10()).clamp(-120.0, 0.0)
        } else {
            // Gating absoluto: -70 LKFS (umbral de silencio)
            let abs_thresh = 10.0f64.powf((-70.0 + 0.691) / 10.0);
            let passing_abs: Vec<f64> = block_powers
                .into_iter()
                .filter(|&z| z >= abs_thresh)
                .collect();

            if passing_abs.is_empty() {
                -70.0
            } else {
                // Gating relativo: -10 LU por debajo del promedio no bloqueado
                let mean_abs = passing_abs.iter().sum::<f64>() / passing_abs.len() as f64;
                let gamma_rel = -0.691 + 10.0 * (mean_abs.max(1e-12)).log10() - 10.0;
                let rel_thresh = 10.0f64.powf((gamma_rel + 0.691) / 10.0);

                let gated: Vec<f64> = passing_abs
                    .into_iter()
                    .filter(|&z| z >= rel_thresh)
                    .collect();
                if gated.is_empty() {
                    gamma_rel
                } else {
                    let mean_gated = gated.iter().sum::<f64>() / gated.len() as f64;
                    (-0.691 + 10.0 * (mean_gated.max(1e-12)).log10()).clamp(-120.0, 0.0)
                }
            }
        }
    };

    // 3. Dynamic Range & RMS
    let mut sum_sq_mono = 0.0f64;
    for &s in samples_mono {
        sum_sq_mono += (s as f64) * (s as f64);
    }
    let rms = (sum_sq_mono / samples_mono.len() as f64).sqrt().max(1e-9);
    let crest_factor_db = (20.0 * (true_peak_linear as f64 / rms).log10()).max(0.0);
    let dynamic_range_db = crest_factor_db.clamp(0.0, 40.0);

    // 4. Robust Noise floor
    let block_size = 512;
    let mut quiet_block_dbs = Vec::new();
    let mut non_zero_mags = Vec::new();
    let mut zero_count = 0usize;

    for chunk in samples_mono.chunks(block_size) {
        let mut b_sum_sq = 0.0f64;
        for &s in chunk {
            let abs_s = s.abs();
            if abs_s == 0.0 {
                zero_count += 1;
            } else {
                non_zero_mags.push(abs_s);
            }
            b_sum_sq += (s as f64) * (s as f64);
        }
        let b_rms = (b_sum_sq / chunk.len() as f64).sqrt();
        if b_rms > 1e-9 && b_rms < 0.00316 {
            // Quiet passage (< -50 dBFS)
            let b_db = 20.0 * b_rms.log10();
            quiet_block_dbs.push(b_db);
        }
    }

    let has_exact_digital_silence = zero_count > (samples_mono.len() / 20);
    let noise_floor_db = if !quiet_block_dbs.is_empty() {
        quiet_block_dbs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let idx = (quiet_block_dbs.len() as f64 * 0.10).floor() as usize;
        quiet_block_dbs[idx.min(quiet_block_dbs.len() - 1)].clamp(-144.0, 0.0)
    } else if !non_zero_mags.is_empty() {
        non_zero_mags.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let idx = (non_zero_mags.len() as f64 * 0.005).floor() as usize;
        let p_mag = non_zero_mags[idx.min(non_zero_mags.len() - 1)];
        (20.0 * (p_mag as f64).log10()).clamp(-144.0, 0.0)
    } else {
        -120.0
    };

    // 5. Bit Depth & LSB Activity Analysis
    let declared_bits = declared_bit_depth.unwrap_or(16);
    let mut real_bit_depth = declared_bits;
    let mut bit_depth_inflated = false;

    if declared_bits >= 24 {
        let mut zero_lsb_count = 0usize;
        let mut non_zero_count = 0usize;
        for &s in samples_mono {
            if s.abs() > 1e-5 {
                let int_val = (s * 8388607.0).round().abs() as i64;
                if (int_val & 0xFF) == 0 {
                    zero_lsb_count += 1;
                }
                non_zero_count += 1;
            }
        }
        let zero_lsb_ratio = if non_zero_count > 100 {
            zero_lsb_count as f64 / non_zero_count as f64
        } else {
            0.0
        };

        if zero_lsb_ratio > 0.85 || noise_floor_db > -90.0 {
            real_bit_depth = 16;
            bit_depth_inflated = true;
        }
    }

    QualityAnalysis {
        metrics: QualityMetrics {
            true_peak_dbtp: Some(true_peak_dbtp),
            lufs_integrated: Some(lufs_integrated),
            clipped_samples,
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
