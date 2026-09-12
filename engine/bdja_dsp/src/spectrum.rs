pub struct SpectrumAnalysis {
    pub effective_bandwidth_hz: u32,
    pub cutoff_slope_db_oct: f64,
    pub shelf_16k_drop_db: f64,
    pub spectral_holes_ratio: f64,
    pub upsampling_detected: bool,
    pub average_spectrum_db: Vec<f32>,
}

pub fn analyze_spectrum(
    power_spectra: &[Vec<f32>],
    sample_rate: u32,
) -> SpectrumAnalysis {
    if power_spectra.is_empty() {
        return SpectrumAnalysis {
            effective_bandwidth_hz: sample_rate / 2,
            cutoff_slope_db_oct: 0.0,
            shelf_16k_drop_db: 0.0,
            spectral_holes_ratio: 0.0,
            upsampling_detected: false,
            average_spectrum_db: vec![-100.0; 256],
        };
    }

    let n_bins = power_spectra[0].len();
    let n_windows = power_spectra.len() as f32;

    // Average power spectrum
    let mut avg_power = vec![0.0f32; n_bins];
    for spec in power_spectra {
        for (i, &p) in spec.iter().enumerate() {
            if i < n_bins {
                avg_power[i] += p / n_windows;
            }
        }
    }

    let bin_hz = sample_rate as f64 / ((n_bins - 1) * 2) as f64;
    let nyquist = sample_rate as f64 / 2.0;

    // Convert to dB scale
    let db_spectrum: Vec<f64> = avg_power
        .iter()
        .map(|&p| 10.0 * (p as f64 + 1e-12).log10())
        .collect();

    // Smoothed spectrum (5-bin moving window to reduce bin-to-bin variance)
    let mut smoothed_db = vec![-120.0; n_bins];
    for i in 0..n_bins {
        let start = i.saturating_sub(2);
        let end = (i + 3).min(n_bins);
        let sum: f64 = db_spectrum[start..end].iter().sum();
        smoothed_db[i] = sum / (end - start) as f64;
    }

    // Acoustic presence reference level in the mid band (1 kHz to 6 kHz)
    let bin_1k = ((1000.0 / bin_hz) as usize).clamp(1, n_bins - 1);
    let bin_6k = ((6000.0 / bin_hz) as usize).clamp(bin_1k + 1, n_bins - 1);
    let ref_level = smoothed_db[bin_1k..bin_6k]
        .iter()
        .cloned()
        .fold(f64::NEG_INFINITY, f64::max);

    // High-frequency floor (last 5% of bins up to Nyquist)
    let high_start = ((0.95 * n_bins as f64) as usize).min(n_bins - 1);
    let hf_floor: f64 = smoothed_db[high_start..].iter().sum::<f64>()
        / (n_bins - high_start).max(1) as f64;

    // P0-2 FIX: Detección por BORDE DE CONTENIDO
    // Un corte de códec lossy produce una caída donde la energía cae abruptamente
    // hacia el piso de ruido. Buscamos el bin más alto donde haya presencia musical consistente
    // (al menos 3 bins consecutivos por encima del umbral de significancia acústica).
    let presence_threshold = (ref_level - 45.0).max(hf_floor + 10.0).max(-90.0);

    let mut cutoff_bin = n_bins.saturating_sub(1);
    let mut consecutive = 0;

    for i in (bin_1k..n_bins).rev() {
        if smoothed_db[i] >= presence_threshold {
            consecutive += 1;
            if consecutive >= 3 {
                cutoff_bin = (i + 2).min(n_bins - 1);
                break;
            }
        } else {
            consecutive = 0;
        }
    }

    let measured_hz = cutoff_bin as f64 * bin_hz;
    let effective_bandwidth_hz = if measured_hz >= nyquist * 0.95 {
        nyquist as u32
    } else {
        measured_hz.round() as u32
    };

    // 2. E02: Cutoff Slope (dB/octave around cutoff)
    let mut cutoff_slope_db_oct = 0.0;
    if cutoff_bin > 10 && cutoff_bin < n_bins - 10 && effective_bandwidth_hz < (nyquist * 0.95) as u32 {
        let f_center = cutoff_bin as f64 * bin_hz;
        let delta_f = (f_center * 0.1).max(400.0);
        let f1 = (f_center - delta_f).max(100.0);
        let f2 = (f_center + delta_f).min(nyquist);

        let bin1 = ((f1 / bin_hz) as usize).min(n_bins - 1);
        let bin2 = ((f2 / bin_hz) as usize).min(n_bins - 1);

        let db1 = smoothed_db[bin1];
        let db2 = smoothed_db[bin2];
        let delta_db = (db1 - db2).max(0.0);

        let octaves = (f2 / f1).log2().max(0.1);
        cutoff_slope_db_oct = delta_db / octaves;
    }

    // 3. E03: 16 kHz Shelf (drop between 14-16 kHz and 16-18 kHz)
    let bin_14k = ((14000.0 / bin_hz) as usize).min(n_bins - 1);
    let bin_16k = ((16000.0 / bin_hz) as usize).min(n_bins - 1);
    let bin_18k = ((18000.0 / bin_hz) as usize).min(n_bins - 1);

    let mut shelf_16k_drop_db = 0.0;
    if bin_14k < bin_16k && bin_16k < bin_18k {
        let p_14_16: f64 = avg_power[bin_14k..bin_16k].iter().map(|&p| p as f64).sum::<f64>()
            / (bin_16k - bin_14k) as f64;
        let p_16_18: f64 = avg_power[bin_16k..bin_18k].iter().map(|&p| p as f64).sum::<f64>()
            / (bin_18k - bin_16k) as f64;

        if p_14_16 > 1e-12 {
            let db_14_16 = 10.0 * (p_14_16 + 1e-12).log10();
            let db_16_18 = 10.0 * (p_16_18 + 1e-12).log10();
            shelf_16k_drop_db = (db_14_16 - db_16_18).max(0.0);
        }
    }

    // 4. E04: Spectral Holes (empty critical subbands above 10 kHz)
    let bin_10k = ((10000.0 / bin_hz) as usize).min(n_bins - 1);
    let mut empty_subbands = 0;
    let total_subbands = 32;

    if bin_10k < cutoff_bin {
        let subband_size = ((cutoff_bin - bin_10k) / total_subbands).max(1);
        let band_peak = avg_power[bin_10k..cutoff_bin]
            .iter()
            .cloned()
            .fold(0.0f32, f32::max) as f64;

        if band_peak > 1e-7 {
            for sb in 0..total_subbands {
                let start = bin_10k + sb * subband_size;
                let end = (start + subband_size).min(cutoff_bin);
                if start < end {
                    let sb_energy: f64 = avg_power[start..end].iter().map(|&p| p as f64).sum::<f64>()
                        / (end - start) as f64;
                    // Subband energy 55 dB below the active band peak indicates empty psychoacoustic hole
                    if (sb_energy / band_peak) < 3.16e-6 {
                        empty_subbands += 1;
                    }
                }
            }
        }
    }
    let spectral_holes_ratio = empty_subbands as f64 / total_subbands as f64;

    // 5. E10: Upsampling (declared > 44.1 kHz, but dead above 21.5 kHz)
    let mut upsampling_detected = false;
    let total_energy: f64 = avg_power.iter().skip(1).map(|&p| p as f64).sum();
    if sample_rate >= 48000 {
        let bin_21_5k = ((21500.0 / bin_hz) as usize).min(n_bins - 1);
        if bin_21_5k < n_bins {
            let energy_above: f64 = avg_power[bin_21_5k..].iter().map(|&p| p as f64).sum();
            if total_energy > 1e-6 && (energy_above / total_energy) < 0.00005 {
                upsampling_detected = true;
            }
        }
    }

    // Real average spectrum in dB (downsampled to 256 display points for UI visualization)
    let display_points = 256;
    let mut average_spectrum_db = Vec::with_capacity(display_points);
    let chunk_size = (n_bins / display_points).max(1);

    for chunk in avg_power.chunks(chunk_size) {
        let max_p = chunk.iter().cloned().fold(0.0f32, f32::max);
        let db = if max_p > 1e-12 {
            10.0 * max_p.log10()
        } else {
            -120.0
        };
        average_spectrum_db.push(db.clamp(-120.0, 0.0));
        if average_spectrum_db.len() >= display_points {
            break;
        }
    }
    while average_spectrum_db.len() < display_points {
        average_spectrum_db.push(-120.0);
    }

    SpectrumAnalysis {
        effective_bandwidth_hz,
        cutoff_slope_db_oct,
        shelf_16k_drop_db,
        spectral_holes_ratio,
        upsampling_detected,
        average_spectrum_db,
    }
}