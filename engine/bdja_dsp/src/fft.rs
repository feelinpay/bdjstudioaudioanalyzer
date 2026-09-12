use std::sync::Arc;
use realfft::{RealFftPlanner, RealToComplex};

pub struct FftProcessor {
    fft_8192: Arc<dyn RealToComplex<f32>>,
    fft_1024: Arc<dyn RealToComplex<f32>>,
    hann_8192: Vec<f32>,
    hann_1024: Vec<f32>,
}

impl FftProcessor {
    pub fn new() -> Self {
        let mut planner = RealFftPlanner::<f32>::new();
        let fft_8192 = planner.plan_fft_forward(8192);
        let fft_1024 = planner.plan_fft_forward(1024);

        let hann_8192: Vec<f32> = (0..8192)
            .map(|i| 0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / 8192.0).cos()))
            .collect();

        let hann_1024: Vec<f32> = (0..1024)
            .map(|i| 0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / 1024.0).cos()))
            .collect();

        Self {
            fft_8192,
            fft_1024,
            hann_8192,
            hann_1024,
        }
    }

    /// Computes power spectrum (length 4097) of 8192 sample window with Hann windowing
    pub fn power_spectrum_8192(&self, window: &[f32]) -> Vec<f32> {
        let mut input = vec![0.0f32; 8192];
        for (i, (&sample, &hann)) in window.iter().zip(self.hann_8192.iter()).enumerate() {
            if i < 8192 {
                input[i] = sample * hann;
            }
        }

        let mut output = self.fft_8192.make_output_vec();
        if self.fft_8192.process(&mut input, &mut output).is_ok() {
            output.iter().map(|c| c.norm_sqr()).collect()
        } else {
            vec![0.0f32; 4097]
        }
    }

    /// Computes power spectrum (length 513) of 1024 sample window with Hann windowing
    pub fn power_spectrum_1024(&self, window: &[f32]) -> Vec<f32> {
        let mut input = vec![0.0f32; 1024];
        for (i, (&sample, &hann)) in window.iter().zip(self.hann_1024.iter()).enumerate() {
            if i < 1024 {
                input[i] = sample * hann;
            }
        }

        let mut output = self.fft_1024.make_output_vec();
        if self.fft_1024.process(&mut input, &mut output).is_ok() {
            output.iter().map(|c| c.norm_sqr()).collect()
        } else {
            vec![0.0f32; 513]
        }
    }
}

pub fn get_fft_processor() -> FftProcessor {
    FftProcessor::new()
}