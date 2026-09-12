use std::f64::consts::PI;

pub struct StereoAnalysis {
    pub global_correlation: f64,
    pub joint_stereo_collapse_detected: bool,
    pub joint_stereo_crossover_hz: Option<u32>,
    pub high_band_correlation: f64,
}

/// Computes Pearson correlation coefficient between two signals
fn pearson_correlation(a: &[f64], b: &[f64]) -> f64 {
    let n = a.len().min(b.len());
    if n == 0 {
        return 1.0;
    }

    let mut dot: f64 = 0.0;
    let mut sum_a2: f64 = 0.0;
    let mut sum_b2: f64 = 0.0;

    for i in 0..n {
        let x = a[i];
        let y = b[i];
        dot += x * y;
        sum_a2 += x * x;
        sum_b2 += y * y;
    }

    let denom = (sum_a2 * sum_b2).sqrt();
    if denom > 1e-9 {
        (dot / denom).clamp(-1.0, 1.0)
    } else {
        1.0
    }
}

/// Applies a 2nd-order Butterworth high-pass filter
fn biquad_highpass(samples: &[f32], cutoff_hz: f64, sample_rate: f64) -> Vec<f64> {
    let nyquist = sample_rate * 0.5;
    let safe_cutoff = cutoff_hz.clamp(500.0, nyquist * 0.95);
    let omega = 2.0 * PI * (safe_cutoff / sample_rate);
    let cos_omega = omega.cos();
    let sin_omega = omega.sin();
    let alpha = sin_omega / (2.0 * std::f64::consts::FRAC_1_SQRT_2);

    let b0 = (1.0 + cos_omega) * 0.5;
    let b1 = -(1.0 + cos_omega);
    let b2 = (1.0 + cos_omega) * 0.5;
    let a0 = 1.0 + alpha;
    let a1 = -2.0 * cos_omega;
    let a2 = 1.0 - alpha;

    let b0 = b0 / a0;
    let b1 = b1 / a0;
    let b2 = b2 / a0;
    let a1 = a1 / a0;
    let a2 = a2 / a0;

    let mut out = Vec::with_capacity(samples.len());
    let mut x1 = 0.0;
    let mut x2 = 0.0;
    let mut y1 = 0.0;
    let mut y2 = 0.0;

    for &s in samples {
        let x = s as f64;
        let y = b0 * x + b1 * x1 + b2 * x2 - a1 * y1 - a2 * y2;
        x2 = x1;
        x1 = x;
        y2 = y1;
        y1 = y;
        out.push(y);
    }

    out
}

pub fn analyze_stereo(
    left_samples: &[f32],
    right_samples: &[f32],
    sample_rate: u32,
) -> StereoAnalysis {
    let n = left_samples.len().min(right_samples.len());
    if n < 1024 {
        return StereoAnalysis {
            global_correlation: 1.0,
            joint_stereo_collapse_detected: false,
            joint_stereo_crossover_hz: None,
            high_band_correlation: 1.0,
        };
    }

    let left_f64: Vec<f64> = left_samples[..n].iter().map(|&s| s as f64).collect();
    let right_f64: Vec<f64> = right_samples[..n].iter().map(|&s| s as f64).collect();

    let global_correlation = pearson_correlation(&left_f64, &right_f64);

    let sr = (sample_rate as f64).max(22050.0);
    let nyquist = sr * 0.5;

    // Check candidate crossover frequencies: 10 kHz, 12 kHz, 14 kHz, 16 kHz
    let candidate_cutoffs = [10000.0, 12000.0, 14000.0, 16000.0];
    let mut detected_crossover = None;
    let mut high_band_correlation = 1.0;

    for &cutoff in &candidate_cutoffs {
        if cutoff < nyquist * 0.85 {
            let left_hf = biquad_highpass(&left_samples[..n], cutoff, sr);
            let right_hf = biquad_highpass(&right_samples[..n], cutoff, sr);
            let hf_corr = pearson_correlation(&left_hf, &right_hf);

            if cutoff == 12000.0 || (cutoff == 10000.0 && detected_crossover.is_none()) {
                high_band_correlation = hf_corr;
            }

            // If global/broadband indicates noticeable stereo divergence (< 0.85),
            // but above cutoff channels become practically identical (> 0.985):
            if global_correlation < 0.85 && hf_corr > 0.985 && detected_crossover.is_none() {
                detected_crossover = Some(cutoff as u32);
                high_band_correlation = hf_corr;
            }
        }
    }

    let joint_stereo_collapse_detected = detected_crossover.is_some();

    StereoAnalysis {
        global_correlation,
        joint_stereo_collapse_detected,
        joint_stereo_crossover_hz: detected_crossover,
        high_band_correlation,
    }
}