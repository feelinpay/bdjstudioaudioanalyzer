use bdja_core::types::CutoffKind;

pub struct SpectrumAnalysis {
    pub cutoff_kind: CutoffKind,
    pub cutoff_frequency_hz: Option<u32>,
    pub content_bandwidth_hz: u32,
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
            cutoff_kind: CutoffKind::FullSpectrum,
            cutoff_frequency_hz: None,
            content_bandwidth_hz: sample_rate / 2,
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

    // P0-2 & AUDIT v3.0: Detector de borde por ventana diferencial (E01)
    // Busca caídas abruptas (brickwall filter típico de códecs lossy) entre ventanas contiguas de ~1.200 Hz
    let step_hz = 1200.0;
    let win_bins = ((step_hz / bin_hz).round() as usize).max(8);
    // Escaneo proporcional desde el 36% de Nyquist (~8 kHz a 44.1k) hasta Nyquist - 400 Hz
    let bin_8k = ((nyquist * 0.36 / bin_hz) as usize).clamp(win_bins, n_bins - 1);
    let bin_nyquist_margin = (((nyquist - 400.0) / bin_hz) as usize).clamp(bin_8k, n_bins - 1);

    let mut detected_cliff: Option<(usize, f64, f64)> = None; // (cutoff_bin, drop, slope)

    // Barrido buscando escalón digital (brickwall)
    let step_stride = (win_bins / 4).max(1);
    for b in (bin_8k..=bin_nyquist_margin).step_by(step_stride) {
        let b_start = b.saturating_sub(win_bins);
        let b_end = (b + win_bins).min(n_bins);

        if b_start < b && b < b_end {
            let p_before: f64 = avg_power[b_start..b].iter().map(|&p| p as f64).sum::<f64>()
                / (b - b_start) as f64;
            let p_after: f64 = avg_power[b..b_end].iter().map(|&p| p as f64).sum::<f64>()
                / (b_end - b) as f64;

            let db_before = 10.0 * (p_before + 1e-12).log10();
            let db_after = 10.0 * (p_after + 1e-12).log10();
            let drop = db_before - db_after;

            // Condición de corte digital (§03 v3.0):
            // 1. Umbral relajado a (ref_level - 70.0).max(-85.0) para no perder mezclas oscuras (-12 a -15 dB/oct)
            // 2. Caída abrupta >= 20 dB en sólo 1.200 Hz (equivalente a > 50 dB/octava)
            // 3. Supresión sostenida post-corte (no es un notch aislado)
            if db_before >= (ref_level - 70.0).max(-85.0) && drop >= 20.0 {
                let p_post: f64 = avg_power[b_end..n_bins].iter().map(|&p| p as f64).sum::<f64>()
                    / (n_bins - b_end).max(1) as f64;
                let db_post = 10.0 * (p_post + 1e-12).log10();

                if db_post <= (db_before - 16.0) {
                    let f_center = b as f64 * bin_hz;
                    let delta_f = (f_center * 0.08).max(400.0);
                    let f1 = (f_center - delta_f).max(100.0);
                    let f2 = (f_center + delta_f).min(nyquist);
                    let octaves = (f2 / f1).log2().max(0.1);
                    let slope = (drop / octaves).max(45.0);

                    detected_cliff = Some((b, drop, slope));
                    break; // Tomar el primer corte artificial verificado
                }
            }
        }
    }

    let (cutoff_kind, cutoff_frequency_hz, content_bandwidth_hz, cutoff_bin, effective_bandwidth_hz, cutoff_slope_db_oct) = if let Some((b_exact, _, slope)) = detected_cliff {
        // Se detectó corte brickwall artificial (MP3 / AAC / etc.)
        let f_measured = b_exact as f64 * bin_hz;
        let hz = f_measured.round() as u32;
        (CutoffKind::BrickwallCutoff, Some(hz), hz, b_exact, hz, slope)
    } else {
        // No existe corte brickwall artificial.
        // Verificar presencia de energía en el extremo superior (88% Nyquist a Nyquist)
        let top_band_start = ((nyquist * 0.88) / bin_hz) as usize;
        let top_band_p: f64 = avg_power[top_band_start..n_bins].iter().map(|&p| p as f64).sum::<f64>()
            / (n_bins - top_band_start).max(1) as f64;
        let top_band_db = 10.0 * (top_band_p + 1e-12).log10();

        if top_band_db >= (ref_level - 50.0).max(-82.0) {
            // Espectro pleno hasta Nyquist sin restricción artificial
            (CutoffKind::FullSpectrum, None, nyquist as u32, n_bins.saturating_sub(1), nyquist as u32, 0.0)
        } else {
            // Decaimiento acústico natural sin filtro brickwall (ej. máster vintage o mezcla oscura)
            let presence_threshold = (ref_level - 45.0).max(-78.0);
            let mut natural_cutoff_bin = n_bins.saturating_sub(1);
            let mut consecutive = 0;
            for i in (bin_1k..n_bins).rev() {
                if smoothed_db[i] >= presence_threshold {
                    consecutive += 1;
                    if consecutive >= 3 {
                        natural_cutoff_bin = (i + 2).min(n_bins - 1);
                        break;
                    }
                } else {
                    consecutive = 0;
                }
            }

            let measured_hz = natural_cutoff_bin as f64 * bin_hz;
            if measured_hz >= nyquist * 0.92 {
                (CutoffKind::FullSpectrum, None, nyquist as u32, n_bins.saturating_sub(1), nyquist as u32, 0.0)
            } else {
                let f_center = measured_hz;
                let delta_f = (f_center * 0.1).max(400.0);
                let f1 = (f_center - delta_f).max(100.0);
                let f2 = (f_center + delta_f).min(nyquist);

                let bin1 = ((f1 / bin_hz) as usize).min(n_bins - 1);
                let bin2 = ((f2 / bin_hz) as usize).min(n_bins - 1);
                let delta_db = (smoothed_db[bin1] - smoothed_db[bin2]).max(0.0);
                let octaves = (f2 / f1).log2().max(0.1);
                let slope = delta_db / octaves;

                (CutoffKind::NaturalRolloff, None, measured_hz.round() as u32, natural_cutoff_bin, measured_hz.round() as u32, slope)
            }
        }
    };

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
        cutoff_kind,
        cutoff_frequency_hz,
        content_bandwidth_hz,
        effective_bandwidth_hz,
        cutoff_slope_db_oct,
        shelf_16k_drop_db,
        spectral_holes_ratio,
        upsampling_detected,
        average_spectrum_db,
    }
}