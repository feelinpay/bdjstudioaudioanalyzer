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

pub fn analyze_spectrum(power_spectra: &[Vec<f32>], sample_rate: u32) -> SpectrumAnalysis {
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
    for (i, val) in smoothed_db.iter_mut().enumerate().take(n_bins) {
        let start = i.saturating_sub(2);
        let end = (i + 3).min(n_bins);
        let sum: f64 = db_spectrum[start..end].iter().sum();
        *val = sum / (end - start) as f64;
    }

    // Acoustic presence reference level in the mid band (1 kHz to 6 kHz)
    let bin_1k = ((1000.0 / bin_hz) as usize).clamp(1, n_bins - 1);
    let bin_6k = ((6000.0 / bin_hz) as usize).clamp(bin_1k + 1, n_bins - 1);
    let ref_level = smoothed_db[bin_1k..bin_6k]
        .iter()
        .cloned()
        .fold(f64::NEG_INFINITY, f64::max);

    // P0-2 & AUDIT v3.0: Detector de borde por ventana diferencial (E01)
    // Busca caídas abruptas (brickwall filter típico de códecs lossy) entre ventanas contiguas
    let step_hz = 1200.0;
    let base_win_bins = ((step_hz / bin_hz).round() as usize).max(8);
    // Escaneo proporcional desde el 36% de Nyquist (~8 kHz a 44.1k) hasta Nyquist - 300 Hz
    let bin_8k = ((nyquist * 0.36 / bin_hz) as usize).clamp(base_win_bins, n_bins - 1);
    let bin_nyquist_margin = (((nyquist - 300.0) / bin_hz) as usize).clamp(bin_8k, n_bins - 1);

    let mut detected_cliff: Option<(usize, f64, f64)> = None; // (cutoff_bin, drop, slope)

    // Barrido buscando escalón digital (brickwall)
    let step_stride = (base_win_bins / 4).max(1);
    for b in (bin_8k..=bin_nyquist_margin).step_by(step_stride) {
        let f_b = b as f64 * bin_hz;
        let remaining_hz = nyquist - f_b;

        // Ventana previa antes del candidato (~1500 Hz terminando en b)
        let pre_win_hz = (f_b * 0.25).clamp(600.0, 1500.0);
        let pre_bins = ((pre_win_hz / bin_hz).round() as usize).max(4);
        let b_start = b.saturating_sub(pre_bins);

        // Banda posterior: adaptativa según si estamos en la zona de corte de MP3 320 / Nyquist de máster (18.5k - 23.5k)
        // o en el rango medio/estándar.
        let (post_start, post_end, min_drop) =
            if (18_500.0..=23_500.0).contains(&f_b) || remaining_hz < 3000.0 {
                let trans_margin_hz = (remaining_hz * 0.35).clamp(180.0, 450.0);
                let trans_bins = ((trans_margin_hz / bin_hz).round() as usize).max(2);
                let p_start = (b + trans_bins).min(n_bins - 1);
                let post_span_hz = remaining_hz.min(2500.0);
                let p_end = n_bins.min(p_start + ((post_span_hz / bin_hz).round() as usize).max(4));
                (p_start, p_end, 13.0)
            } else {
                let post_win_hz = step_hz.min(remaining_hz * 0.75).max(350.0);
                let win_bins = ((post_win_hz / bin_hz).round() as usize).max(4);
                let p_start = b;
                let p_end = (b + win_bins).min(n_bins);
                (p_start, p_end, 17.0)
            };

        if b_start < b && post_start < post_end.saturating_sub(2) {
            let p_before: f64 =
                avg_power[b_start..b].iter().map(|&p| p as f64).sum::<f64>() / (b - b_start) as f64;
            let p_post: f64 = avg_power[post_start..post_end]
                .iter()
                .map(|&p| p as f64)
                .sum::<f64>()
                / (post_end - post_start) as f64;

            let db_before = 10.0 * (p_before + 1e-12).log10();
            let db_post = 10.0 * (p_post + 1e-12).log10();
            let drop = db_before - db_post;

            if db_before >= (ref_level - 70.0).max(-85.0) && drop >= min_drop {
                let delta_f = (f_b * 0.08).max(350.0);
                let f1 = (f_b - delta_f).max(100.0);
                let f2 = (f_b + delta_f).min(nyquist);
                let octaves = (f2 / f1).log2().max(0.1);
                let slope = (drop / octaves).max(45.0);

                detected_cliff = Some((b, drop, slope));
                break; // Tomar el primer corte artificial verificado
            }
        }
    }

    let (
        cutoff_kind,
        cutoff_frequency_hz,
        content_bandwidth_hz,
        cutoff_bin,
        effective_bandwidth_hz,
        cutoff_slope_db_oct,
    ) = if let Some((b_exact, _, slope)) = detected_cliff {
        // Se detectó corte brickwall artificial (MP3 / AAC / etc.)
        let f_measured = b_exact as f64 * bin_hz;
        let hz = f_measured.round() as u32;
        (
            CutoffKind::BrickwallCutoff,
            Some(hz),
            hz,
            b_exact,
            hz,
            slope,
        )
    } else {
        // No existe corte brickwall artificial detectado en el barrido.
        // Ajustar tendencia espectral en octavas entre 10 kHz y 17 kHz
        let bin_10k = ((10_000.0 / bin_hz).round() as usize).clamp(bin_1k, n_bins - 1);
        let bin_17k = ((17_000.0 / bin_hz).round() as usize).clamp(bin_10k + 4, n_bins - 1);

        let mut sum_x = 0.0f64;
        let mut sum_y = 0.0f64;
        let mut sum_xx = 0.0f64;
        let mut sum_xy = 0.0f64;
        let count = (bin_17k - bin_10k) as f64;

        for (idx, &db_val) in smoothed_db.iter().enumerate().take(bin_17k).skip(bin_10k) {
            let f_i = idx as f64 * bin_hz;
            let x = (f_i / 10_000.0).log2();
            let y = db_val;
            sum_x += x;
            sum_y += y;
            sum_xx += x * x;
            sum_xy += x * y;
        }
        let denom = count * sum_xx - sum_x * sum_x;
        let (alpha, beta) = if denom.abs() > 1e-6 {
            let b = (count * sum_xy - sum_x * sum_y) / denom;
            let a = (sum_y - b * sum_x) / count;
            (a, b)
        } else {
            (ref_level - 30.0, -6.0)
        };

        // Extrapolación al centro de la banda extrema (20.5 kHz o 93% Nyquist)
        let f_top_center = nyquist * 0.93;
        let x_top = (f_top_center / 10_000.0).log2();
        let expected_top_db = alpha + beta * x_top;

        // Medir energía real en la banda extrema superior (88% Nyquist a Nyquist)
        let top_band_start = ((nyquist * 0.88) / bin_hz) as usize;
        let top_band_p: f64 = avg_power[top_band_start..n_bins]
            .iter()
            .map(|&p| p as f64)
            .sum::<f64>()
            / (n_bins - top_band_start).max(1) as f64;
        let top_band_db = 10.0 * (top_band_p + 1e-12).log10();

        // Criterio de FullSpectrum:
        // 1. Nivel absoluto por encima del umbral mínimo de energía activa
        // 2. La energía real no colapsa respecto a la extrapolación espectral (máximo 16 dB por debajo)
        let is_continuous_trend = top_band_db >= (expected_top_db - 16.0);
        let has_absolute_energy = top_band_db >= (ref_level - 50.0).max(-82.0);

        if is_continuous_trend && has_absolute_energy {
            // Espectro pleno hasta Nyquist sin restricción artificial
            (
                CutoffKind::FullSpectrum,
                None,
                nyquist as u32,
                n_bins.saturating_sub(1),
                nyquist as u32,
                0.0,
            )
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
            if is_continuous_trend && measured_hz >= nyquist * 0.92 {
                (
                    CutoffKind::FullSpectrum,
                    None,
                    nyquist as u32,
                    n_bins.saturating_sub(1),
                    nyquist as u32,
                    0.0,
                )
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

                (
                    CutoffKind::NaturalRolloff,
                    None,
                    measured_hz.round() as u32,
                    natural_cutoff_bin,
                    measured_hz.round() as u32,
                    slope,
                )
            }
        }
    };

    // 3. E03: 16 kHz Shelf (drop between 14-16 kHz and 16-18 kHz)
    let bin_14k = ((14000.0 / bin_hz) as usize).min(n_bins - 1);
    let bin_16k = ((16000.0 / bin_hz) as usize).min(n_bins - 1);
    let bin_18k = ((18000.0 / bin_hz) as usize).min(n_bins - 1);

    let mut shelf_16k_drop_db = 0.0;
    if bin_14k < bin_16k && bin_16k < bin_18k {
        let p_14_16: f64 = avg_power[bin_14k..bin_16k]
            .iter()
            .map(|&p| p as f64)
            .sum::<f64>()
            / (bin_16k - bin_14k) as f64;
        let p_16_18: f64 = avg_power[bin_16k..bin_18k]
            .iter()
            .map(|&p| p as f64)
            .sum::<f64>()
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
                    let sb_energy: f64 =
                        avg_power[start..end].iter().map(|&p| p as f64).sum::<f64>()
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
