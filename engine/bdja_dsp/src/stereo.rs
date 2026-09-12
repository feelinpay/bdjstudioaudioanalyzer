pub struct StereoAnalysis {
    pub global_correlation: f64,
    pub joint_stereo_collapse_detected: bool,
    pub joint_stereo_crossover_hz: Option<u32>,
    pub high_band_correlation: f64,
}

pub fn analyze_stereo(
    left_samples: &[f32],
    right_samples: &[f32],
    _sample_rate: u32,
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

    let mut dot: f64 = 0.0;
    let mut sum_l2: f64 = 0.0;
    let mut sum_r2: f64 = 0.0;

    for i in 0..n {
        let l = left_samples[i] as f64;
        let r = right_samples[i] as f64;
        dot += l * r;
        sum_l2 += l * l;
        sum_r2 += r * r;
    }

    let denom = (sum_l2 * sum_r2).sqrt();
    let global_correlation = if denom > 1e-9 {
        (dot / denom).clamp(-1.0, 1.0)
    } else {
        1.0
    };

    // Subband correlation via simple filtering:
    // Difference signal for high band: L[i] - L[i-1], R[i] - R[i-1]
    let mut hf_dot: f64 = 0.0;
    let mut hf_l2: f64 = 0.0;
    let mut hf_r2: f64 = 0.0;

    for i in 1..n {
        let hl = (left_samples[i] - left_samples[i - 1]) as f64;
        let hr = (right_samples[i] - right_samples[i - 1]) as f64;
        hf_dot += hl * hr;
        hf_l2 += hl * hl;
        hf_r2 += hr * hr;
    }

    let hf_denom = (hf_l2 * hf_r2).sqrt();
    let high_band_correlation = if hf_denom > 1e-9 {
        (hf_dot / hf_denom).clamp(-1.0, 1.0)
    } else {
        1.0
    };

    // If global correlation is modest (< 0.85) indicating stereo content,
    // but high frequency correlation is virtually 1.0 (> 0.985),
    // this signifies Intensity Stereo collapse!
    let joint_stereo_collapse_detected = global_correlation < 0.82 && high_band_correlation > 0.985;
    let joint_stereo_crossover_hz = if joint_stereo_collapse_detected {
        Some(14000)
    } else {
        None
    };

    StereoAnalysis {
        global_correlation,
        joint_stereo_collapse_detected,
        joint_stereo_crossover_hz,
        high_band_correlation,
    }
}