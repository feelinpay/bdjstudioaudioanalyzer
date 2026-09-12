use bdja_core::types::FileReport;
use bdja_core::types::ENGINE_REV;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

fn print_usage() {
    println!(
        "BDJ Studio Audio Analyzer — CLI Forense v{} (Rev {})",
        env!("CARGO_PKG_VERSION"),
        ENGINE_REV
    );
    println!("\nUSO:");
    println!("  bdja_cli analyze <ruta_archivo> [--json]              Analiza un archivo individual de audio");
    println!("  bdja_cli scan <ruta_directorio>                       Escanea una carpeta o biblioteca de audio");
    println!("  bdja_cli validate <corpus_dir|manifiesto.csv>         Valida precisión, recall y tasa de falsos positivos (FPR)");
    println!("  bdja_cli validate --lossless-dir <dir> --transcode-dir <dir>  Valida usando dos carpetas independientes");
    println!("  bdja_cli calibrate <corpus_dir|manifiesto.csv>        Calcula distribución de LLR, percentiles y umbrales óptimos");
    println!("  bdja_cli calibrate --lossless-dir <dir> --transcode-dir <dir> Calcula umbrales óptimos usando dos carpetas");
    println!("  bdja_cli version                                      Muestra la revisión del motor y versión");
    println!("  bdja_cli help                                         Muestra esta ayuda");
}

fn run_analyze(path_str: &str, as_json: bool) {
    let path = Path::new(path_str);
    if !path.exists() {
        eprintln!("Error: el archivo '{}' no existe.", path_str);
        std::process::exit(1);
    }

    let start = Instant::now();
    match bdja_scan::analyze_single_file(path) {
        Ok(report) => {
            let elapsed = start.elapsed();
            if as_json {
                if let Ok(json) = serde_json::to_string_pretty(&report) {
                    println!("{}", json);
                }
            } else {
                println!("============================================================");
                println!(" BDJ STUDIO AUDIO ANALYZER — REPORTE FORENSE");
                println!("============================================================");
                println!("Archivo:           {}", report.path);
                println!(
                    "Tamaño:            {:.2} MB",
                    report.file_size as f64 / (1024.0 * 1024.0)
                );
                println!(
                    "Contenedor:        {} ({})",
                    report.facts.container, report.facts.codec
                );
                println!("Frecuencia:        {} Hz", report.facts.sample_rate);
                println!(
                    "Profundidad:       {} bits",
                    report.facts.bit_depth.unwrap_or(16)
                );
                println!("Canales:           {}", report.facts.channels);
                println!(
                    "Bitrate:           {} kbps",
                    report.facts.container_bitrate_kbps.unwrap_or(0)
                );
                println!("------------------------------------------------------------");
                println!(
                    "VEREDICTO:         {} (Confianza: {:.1}%)",
                    report.verdict.display_name(),
                    report.confidence * 100.0
                );
                println!("Score LLR:         {:.2}", report.score_llr);
                println!(
                    "Ancho de banda:    {} Hz",
                    report.effective_bandwidth_hz.unwrap_or(0)
                );
                println!(
                    "Pendiente corte:   {:.1} dB/oct",
                    report.cutoff_slope_db_oct.unwrap_or(0.0)
                );
                println!("Resumen:           {}", report.verdict_summary);
                println!("------------------------------------------------------------");
                println!("EVIDENCIAS DETECTADAS:");
                for ev in &report.evidences {
                    let code_str = format!("{:?}", ev.code);
                    let strong_mark = if ev.code.is_strong() { " [FUERTE]" } else { "" };
                    println!(
                        "  * {}{}: LLR={:+.2} — {}",
                        code_str, strong_mark, ev.llr, ev.description
                    );
                }
                if !report.guards_triggered.is_empty() {
                    println!("------------------------------------------------------------");
                    println!("GUARDAS ACTIVADAS:");
                    for g in &report.guards_triggered {
                        println!("  ! {}", g);
                    }
                }
                println!("------------------------------------------------------------");
                println!("MÉTRICAS ACÚSTICAS:");
                println!(
                    "  True Peak:         {:.2} dBTP",
                    report.quality.true_peak_dbtp.unwrap_or(0.0)
                );
                println!(
                    "  LUFS Integrado:    {:.1} LUFS",
                    report.quality.lufs_integrated.unwrap_or(-14.0)
                );
                println!(
                    "  Clipping:          {} muestras consecutivas",
                    report.quality.clipped_samples
                );
                println!(
                    "  Rango Dinámico:    {:.1} dB",
                    report.quality.dynamic_range_db.unwrap_or(0.0)
                );
                println!(
                    "  Offset DC:         {:.5}",
                    report.quality.dc_offset.unwrap_or(0.0)
                );
                println!("============================================================");
                println!("Tiempo de análisis: {:?}", elapsed);
            }
        }
        Err(e) => {
            eprintln!("Error analizando archivo: {}", e);
            std::process::exit(1);
        }
    }
}

fn run_scan(dir_str: &str) {
    let path = Path::new(dir_str);
    if !path.exists() {
        eprintln!("Error: la ruta '{}' no existe.", dir_str);
        std::process::exit(1);
    }

    println!("Iniciando escaneo masivo en: {}", dir_str);
    let cancel = Arc::new(AtomicBool::new(false));
    let roots = vec![PathBuf::from(dir_str)];
    let start = Instant::now();

    let transcode_count = Arc::new(AtomicU64::new(0));
    let lossless_count = Arc::new(AtomicU64::new(0));
    let suspicious_count = Arc::new(AtomicU64::new(0));
    let other_count = Arc::new(AtomicU64::new(0));

    let tc = transcode_count.clone();
    let lc = lossless_count.clone();
    let sc = suspicious_count.clone();
    let oc = other_count.clone();

    let on_file_analyzed = move |report: FileReport| match report.verdict {
        bdja_core::types::Verdict::ProbableTranscode => {
            tc.fetch_add(1, Ordering::Relaxed);
            println!(
                "\n  [TRANSCODE DETECTADO] {} ({} Hz, LLR={:+.1})",
                report.path,
                report.effective_bandwidth_hz.unwrap_or(0),
                report.score_llr
            );
        }
        bdja_core::types::Verdict::LosslessVerified | bdja_core::types::Verdict::LikelyLossless => {
            lc.fetch_add(1, Ordering::Relaxed);
        }
        bdja_core::types::Verdict::Suspicious => {
            sc.fetch_add(1, Ordering::Relaxed);
        }
        _ => {
            oc.fetch_add(1, Ordering::Relaxed);
        }
    };

    let on_progress = |done: u64, total: u64, current: String| {
        let name = Path::new(&current)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy();
        print!(
            "\rProgreso: {}/{} pistas | {}                  ",
            done, total, name
        );
        let _ = std::io::stdout().flush();
    };

    match bdja_scan::scan_collection(
        &roots,
        "normal",
        false,
        None,
        cancel,
        on_file_analyzed,
        on_progress,
    ) {
        Ok(processed) => {
            let elapsed = start.elapsed();
            println!("\n\n============================================================");
            println!(" RESUMEN DE ESCANEO MASIVO");
            println!("============================================================");
            println!("Total pistas procesadas:    {}", processed);
            println!(
                "Lossless verificados:       {}",
                lossless_count.load(Ordering::Relaxed)
            );
            println!(
                "Sospechosos:                {}",
                suspicious_count.load(Ordering::Relaxed)
            );
            println!(
                "Probables transcodes:       {}",
                transcode_count.load(Ordering::Relaxed)
            );
            println!(
                "Otros formatos / lossy:     {}",
                other_count.load(Ordering::Relaxed)
            );
            println!("Tiempo total transcurrido:  {:.2?}", elapsed);
            println!("============================================================");
        }
        Err(e) => {
            eprintln!("\nError durante el escaneo: {}", e);
            std::process::exit(1);
        }
    }
}

#[derive(Debug, Clone)]
struct CorpusSample {
    path: PathBuf,
    is_transcode: bool,
}

fn collect_audio_from_dir(dir: &Path, is_transcode: bool) -> Result<Vec<CorpusSample>, String> {
    if !dir.exists() {
        return Err(format!("El directorio '{}' no existe", dir.display()));
    }
    let mut samples = Vec::new();
    for e in jwalk::WalkDir::new(dir)
        .skip_hidden(true)
        .into_iter()
        .flatten()
    {
        if e.file_type().is_file() && bdja_scan::is_analyzable(&e.path()) {
            samples.push(CorpusSample {
                path: e.path(),
                is_transcode,
            });
        }
    }
    Ok(samples)
}

fn load_corpus_samples(target_str: &str) -> Result<Vec<CorpusSample>, String> {
    let target_path = Path::new(target_str);
    if !target_path.exists() {
        return Err(format!("La ruta '{}' no existe", target_str));
    }

    let mut samples = Vec::new();

    if target_path.is_file() {
        let content = std::fs::read_to_string(target_path)
            .map_err(|e| format!("Error leyendo manifiesto CSV '{}': {}", target_str, e))?;

        for (line_idx, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            // Skip header
            if line_idx == 0
                && (trimmed.to_lowercase().starts_with("path")
                    || trimmed.to_lowercase().starts_with("archivo"))
            {
                continue;
            }
            let sep = if trimmed.contains(';') { ';' } else { ',' };
            let parts: Vec<&str> = trimmed
                .split(sep)
                .map(|s| s.trim().trim_matches('"'))
                .collect();
            if parts.len() < 2 {
                continue;
            }
            let file_path = PathBuf::from(parts[0]);
            let label = parts[1].to_lowercase();
            let is_transcode = label.contains("transcode")
                || label.contains("fake")
                || label.contains("lossy")
                || label.contains("upsampled")
                || label.contains("mp3")
                || label.contains("aac")
                || label == "1"
                || label == "true";
            let is_lossless = label.contains("lossless")
                || label.contains("genuine")
                || label.contains("authentic")
                || label.contains("master")
                || label.contains("original")
                || label == "0"
                || label == "false";

            if is_transcode {
                samples.push(CorpusSample {
                    path: file_path,
                    is_transcode: true,
                });
            } else if is_lossless {
                samples.push(CorpusSample {
                    path: file_path,
                    is_transcode: false,
                });
            }
        }
    } else {
        for e in jwalk::WalkDir::new(target_path)
            .skip_hidden(true)
            .into_iter()
            .flatten()
        {
            if e.file_type().is_file() && bdja_scan::is_analyzable(&e.path()) {
                let p = e.path();
                let path_lower = p.to_string_lossy().to_lowercase();
                let is_transcode = path_lower.contains("transcode")
                    || path_lower.contains("fake")
                    || path_lower.contains("upsampled")
                    || path_lower.contains("mp3_")
                    || path_lower.contains("aac_");
                let is_lossless = path_lower.contains("lossless")
                    || path_lower.contains("genuine")
                    || path_lower.contains("authentic")
                    || path_lower.contains("master");

                if is_transcode {
                    samples.push(CorpusSample {
                        path: p,
                        is_transcode: true,
                    });
                } else if is_lossless {
                    samples.push(CorpusSample {
                        path: p,
                        is_transcode: false,
                    });
                }
            }
        }
    }

    Ok(samples)
}

fn parse_corpus_args(args: &[String]) -> Result<(String, Vec<CorpusSample>), String> {
    let mut lossless_dir: Option<&str> = None;
    let mut transcode_dir: Option<&str> = None;
    let mut manifest_path: Option<&str> = None;
    let mut positional: Option<&str> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--lossless-dir" => {
                if i + 1 < args.len() {
                    lossless_dir = Some(&args[i + 1]);
                    i += 2;
                } else {
                    return Err("Falta la ruta para --lossless-dir".to_string());
                }
            }
            "--transcode-dir" => {
                if i + 1 < args.len() {
                    transcode_dir = Some(&args[i + 1]);
                    i += 2;
                } else {
                    return Err("Falta la ruta para --transcode-dir".to_string());
                }
            }
            "--manifest" => {
                if i + 1 < args.len() {
                    manifest_path = Some(&args[i + 1]);
                    i += 2;
                } else {
                    return Err("Falta la ruta para --manifest".to_string());
                }
            }
            other => {
                if !other.starts_with("--") && positional.is_none() {
                    positional = Some(other);
                }
                i += 1;
            }
        }
    }

    if lossless_dir.is_some() || transcode_dir.is_some() {
        if lossless_dir.is_none() || transcode_dir.is_none() {
            return Err(
                "Debe especificar ambos: --lossless-dir <dir> y --transcode-dir <dir>".to_string(),
            );
        }
        let l_dir = lossless_dir.unwrap();
        let t_dir = transcode_dir.unwrap();
        let mut samples = collect_audio_from_dir(Path::new(l_dir), false)?;
        let n_l = samples.len();
        let mut trans = collect_audio_from_dir(Path::new(t_dir), true)?;
        let n_t = trans.len();
        samples.append(&mut trans);
        let desc = format!(
            "Carpetas independientes (Lossless: '{}' [{} pistas], Transcode: '{}' [{} pistas])",
            l_dir, n_l, t_dir, n_t
        );
        return Ok((desc, samples));
    }

    if let Some(target) = manifest_path.or(positional) {
        let samples = load_corpus_samples(target)?;
        let desc = format!("Corpus: '{}'", target);
        return Ok((desc, samples));
    }

    Err("Debe especificar un corpus: '--lossless-dir <dir> --transcode-dir <dir>', '--manifest <csv>' o una ruta posicional.".to_string())
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    if sorted.len() == 1 {
        return sorted[0];
    }
    let idx = (p / 100.0) * (sorted.len() - 1) as f64;
    let low = idx.floor() as usize;
    let high = idx.ceil() as usize;
    let frac = idx - low as f64;
    if low == high {
        sorted[low]
    } else {
        sorted[low] * (1.0 - frac) + sorted[high] * frac
    }
}

fn std_dev(scores: &[f64], mean: f64) -> f64 {
    if scores.len() < 2 {
        return 0.0;
    }
    let variance =
        scores.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / (scores.len() - 1) as f64;
    variance.sqrt()
}

fn run_validate(target_desc: &str, samples: Vec<CorpusSample>) {
    println!("============================================================");
    println!(" BDJ STUDIO AUDIO ANALYZER — VALIDACIÓN FORENSE DE CORPUS");
    println!("============================================================");
    println!("Objetivo: {}", target_desc);
    println!("Muestras encontradas para validación: {}", samples.len());

    if samples.is_empty() {
        println!("No se encontraron pistas etiquetadas en el corpus.");
        return;
    }

    let mut tp = 0u64;
    let mut fn_count = 0u64;
    let mut tn = 0u64;
    let mut fp = 0u64;
    let mut inconclusive = 0u64;

    let start = Instant::now();
    for s in &samples {
        match bdja_scan::analyze_single_file(&s.path) {
            Ok(report) => {
                let is_convicted = matches!(
                    report.verdict,
                    bdja_core::types::Verdict::ProbableTranscode
                        | bdja_core::types::Verdict::Suspicious
                );
                let is_cleared = matches!(
                    report.verdict,
                    bdja_core::types::Verdict::LosslessVerified
                        | bdja_core::types::Verdict::LikelyLossless
                );

                if s.is_transcode {
                    if is_convicted {
                        tp += 1;
                    } else if is_cleared {
                        fn_count += 1;
                        println!(
                            "  [FALSO NEGATIVO] {} (LLR={:.2})",
                            s.path.display(),
                            report.score_llr
                        );
                    } else {
                        inconclusive += 1;
                    }
                } else if is_cleared {
                    tn += 1;
                } else if is_convicted {
                    fp += 1;
                    println!(
                        "  [FALSO POSITIVO] {} (LLR={:.2})",
                        s.path.display(),
                        report.score_llr
                    );
                } else {
                    inconclusive += 1;
                }
            }
            Err(e) => {
                println!("  [ERROR DECODE] {}: {}", s.path.display(), e);
                inconclusive += 1;
            }
        }
    }

    let elapsed = start.elapsed();
    let total_classified = tp + fn_count + tn + fp;
    let recall = if tp + fn_count > 0 {
        (tp as f64) / ((tp + fn_count) as f64) * 100.0
    } else {
        0.0
    };
    let fpr = if fp + tn > 0 {
        (fp as f64) / ((fp + tn) as f64) * 100.0
    } else {
        0.0
    };
    let precision = if tp + fp > 0 {
        (tp as f64) / ((tp + fp) as f64) * 100.0
    } else {
        0.0
    };
    let accuracy = if total_classified > 0 {
        ((tp + tn) as f64) / (total_classified as f64) * 100.0
    } else {
        0.0
    };

    println!("\n------------------------------------------------------------");
    println!(" RESULTADOS DE VALIDACIÓN (Matriz de Confusión)");
    println!("------------------------------------------------------------");
    println!("  Total evaluados:             {}", samples.len());
    println!("  Total clasificados:          {}", total_classified);
    println!("  Verdaderos Positivos (TP):   {}", tp);
    println!("  Falsos Negativos (FN):       {}", fn_count);
    println!("  Verdaderos Negativos (TN):   {}", tn);
    println!("  Falsos Positivos (FP):       {}", fp);
    println!("  Casos Inconclusos:           {}", inconclusive);
    println!("------------------------------------------------------------");
    println!("  Sensibilidad / Recall:       {:.2}%", recall);
    println!(
        "  Tasa Falsos Positivos (FPR): {:.2}% (Objetivo <= 1.0%)",
        fpr
    );
    println!("  Precisión (PPV):             {:.2}%", precision);
    println!("  Exactitud Global:            {:.2}%", accuracy);
    println!("  Tiempo transcurrido:         {:.2?}", elapsed);
    println!("------------------------------------------------------------");

    if fpr <= 1.0 && recall >= 95.0 {
        println!("  [GATE STATUS: PASS] El motor cumple las tolerancias forenses requeridas.");
    } else {
        println!("  [GATE STATUS: FAIL] El motor no alcanza los criterios de producción (FPR <= 1.0%, Recall >= 95.0%).");
    }
    println!("============================================================");
}

fn run_calibrate(target_desc: &str, samples: Vec<CorpusSample>) {
    println!("============================================================");
    println!(" BDJ STUDIO AUDIO ANALYZER — CALIBRACIÓN FORENSE DE CORPUS");
    println!("============================================================");
    println!("Objetivo: {}", target_desc);
    println!("Muestras cargadas: {}", samples.len());

    let mut lossless_scores: Vec<f64> = Vec::new();
    let mut transcode_scores: Vec<f64> = Vec::new();

    let start = Instant::now();
    for s in &samples {
        if let Ok(report) = bdja_scan::analyze_single_file(&s.path) {
            if s.is_transcode {
                transcode_scores.push(report.score_llr);
            } else {
                lossless_scores.push(report.score_llr);
            }
        }
    }

    if lossless_scores.is_empty() || transcode_scores.is_empty() {
        eprintln!(
            "Error: el corpus debe contener al menos 1 pista lossless y 1 pista transcode. Encontradas: {} lossless, {} transcodes",
            lossless_scores.len(),
            transcode_scores.len()
        );
        return;
    }

    lossless_scores.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    transcode_scores.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let n_lossless = lossless_scores.len();
    let n_transcode = transcode_scores.len();

    if n_lossless < 50 || n_transcode < 50 {
        println!("------------------------------------------------------------");
        println!(
            "  [AVISO ESTADÍSTICO] Muestras por clase bajas (Lossless: {}, Transcode: {}).\n  Se recomienda un mínimo de 50 muestras por clase para que el umbral óptimo sea estadísticamente representativo.",
            n_lossless, n_transcode
        );
        println!("------------------------------------------------------------");
    }

    let mean_lossless = lossless_scores.iter().sum::<f64>() / n_lossless as f64;
    let mean_transcode = transcode_scores.iter().sum::<f64>() / n_transcode as f64;

    let sd_lossless = std_dev(&lossless_scores, mean_lossless);
    let sd_transcode = std_dev(&transcode_scores, mean_transcode);

    let p_lossless = [
        lossless_scores[0],
        percentile(&lossless_scores, 5.0),
        percentile(&lossless_scores, 25.0),
        percentile(&lossless_scores, 50.0),
        percentile(&lossless_scores, 75.0),
        percentile(&lossless_scores, 95.0),
        lossless_scores[n_lossless - 1],
    ];

    let p_transcode = [
        transcode_scores[0],
        percentile(&transcode_scores, 5.0),
        percentile(&transcode_scores, 25.0),
        percentile(&transcode_scores, 50.0),
        percentile(&transcode_scores, 75.0),
        percentile(&transcode_scores, 95.0),
        transcode_scores[n_transcode - 1],
    ];

    println!("\nDISTRIBUCIÓN DE LOG-LIKELIHOOD RATIOS (LLR):");
    println!("-----------------------------------------------------------------------------");
    println!(
        "{:<20} | {:<22} | {:<22}",
        "Métrica / Percentil", "Lossless Legítimo", "Transcode Condenado"
    );
    println!("-----------------------------------------------------------------------------");
    println!(
        "{:<20} | {:<22} | {:<22}",
        "Muestras (N)", n_lossless, n_transcode
    );
    println!(
        "{:<20} | {:<+22.2} | {:<+22.2}",
        "Mínimo (p0)", p_lossless[0], p_transcode[0]
    );
    println!(
        "{:<20} | {:<+22.2} | {:<+22.2}",
        "Percentil 5 (p5)", p_lossless[1], p_transcode[1]
    );
    println!(
        "{:<20} | {:<+22.2} | {:<+22.2}",
        "Percentil 25 (p25)", p_lossless[2], p_transcode[2]
    );
    println!(
        "{:<20} | {:<+22.2} | {:<+22.2}",
        "Mediana (p50)", p_lossless[3], p_transcode[3]
    );
    println!(
        "{:<20} | {:<+22.2} | {:<+22.2}",
        "Percentil 75 (p75)", p_lossless[4], p_transcode[4]
    );
    println!(
        "{:<20} | {:<+22.2} | {:<+22.2}",
        "Percentil 95 (p95)", p_lossless[5], p_transcode[5]
    );
    println!(
        "{:<20} | {:<+22.2} | {:<+22.2}",
        "Máximo (p100)", p_lossless[6], p_transcode[6]
    );
    println!("-----------------------------------------------------------------------------");
    println!(
        "{:<20} | {:<+22.2} | {:<+22.2}",
        "Media aritmética", mean_lossless, mean_transcode
    );
    println!(
        "{:<20} | {:<22.2} | {:<22.2}",
        "Desviación estándar", sd_lossless, sd_transcode
    );
    let delta_p50 = p_transcode[3] - p_lossless[3];
    println!("-----------------------------------------------------------------------------");
    println!(
        "Separación Delta (Mediana Transcode - Mediana Lossless): {:+.2} LLR",
        delta_p50
    );

    // Búsqueda de umbral óptimo que garantice FPR <= 1.0% maximizando Recall
    let min_grid = (p_lossless[0] - 2.0).floor() as i64;
    let max_grid = (p_transcode[6] + 2.0).ceil() as i64;
    let steps = ((max_grid - min_grid) as f64 / 0.05).round() as usize;

    let mut best_threshold = 2.0;
    let mut best_recall = 0.0;
    let mut best_fpr = 0.0;
    let mut best_precision = 0.0;
    let mut found_valid = false;

    for i in 0..=steps {
        let t = min_grid as f64 + (i as f64) * 0.05;
        let fp = lossless_scores.iter().filter(|&&s| s >= t).count();
        let tn = n_lossless - fp;
        let tp = transcode_scores.iter().filter(|&&s| s >= t).count();
        let fn_count = n_transcode - tp;

        let fpr = (fp as f64) / ((fp + tn) as f64) * 100.0;
        let recall = (tp as f64) / ((tp + fn_count) as f64) * 100.0;
        let precision = if tp + fp > 0 {
            (tp as f64) / ((tp + fp) as f64) * 100.0
        } else {
            0.0
        };

        if fpr <= 1.0 && (!found_valid || recall > best_recall) {
            best_threshold = t;
            best_recall = recall;
            best_fpr = fpr;
            best_precision = precision;
            found_valid = true;
        }
    }

    if !found_valid {
        // Fallback: minimizar errores totales si ningún punto da FPR <= 1.0%
        let mut min_errors = usize::MAX;
        for i in 0..=steps {
            let t = min_grid as f64 + (i as f64) * 0.05;
            let fp = lossless_scores.iter().filter(|&&s| s >= t).count();
            let tn = n_lossless - fp;
            let tp = transcode_scores.iter().filter(|&&s| s >= t).count();
            let fn_count = n_transcode - tp;
            let errs = fp + fn_count;
            if errs < min_errors {
                min_errors = errs;
                best_threshold = t;
                best_fpr = (fp as f64) / ((fp + tn) as f64) * 100.0;
                best_recall = (tp as f64) / ((tp + fn_count) as f64) * 100.0;
                best_precision = if tp + fp > 0 {
                    (tp as f64) / ((tp + fp) as f64) * 100.0
                } else {
                    0.0
                };
            }
        }
    }

    println!("\nOPTIMIZACIÓN FORENSE DE UMBRAL:");
    println!("  Umbral óptimo (T*):          {:+.2} LLR", best_threshold);
    println!("  Sensibilidad / Recall:       {:.2}%", best_recall);
    println!(
        "  Tasa Falsos Positivos (FPR): {:.2}% (Objetivo <= 1.0%)",
        best_fpr
    );
    println!("  Precisión (PPV):             {:.2}%", best_precision);

    println!("\nUMBRALES RECOMENDADOS PARA PRODUCCIÓN:");
    let verified_threshold = (p_lossless[4]).min(-0.5);
    let likely_lossless = (best_threshold - 1.5).max(verified_threshold);
    let suspicious = best_threshold - 1.5;
    println!(
        "  Lossless Verificado:         LLR <= {:+.2}",
        verified_threshold
    );
    println!(
        "  Likely Lossless:             LLR <  {:+.2}",
        likely_lossless
    );
    println!("  Sospechoso (Suspicious):     LLR >= {:+.2}", suspicious);
    println!(
        "  Probable Transcode:          LLR >= {:+.2}",
        best_threshold
    );

    println!("\nEVALUACIÓN DE GATE FORENSE:");
    let is_pass = best_fpr <= 1.0 && best_recall >= 95.0 && delta_p50 >= 3.0;
    if is_pass {
        println!("  [GATE PASS] Calibración aprobada: separación forense nítida y estadísticamente válida.");
    } else {
        println!("  [GATE FAIL] El corpus no cumple los criterios requeridos (FPR <= 1.0%, Recall >= 95.0%, Delta >= 3.0 LLR).");
    }

    println!("Tiempo de análisis: {:.2?}", start.elapsed());
    println!("============================================================");

    if !is_pass && std::env::var("BDJA_STRICT_GATE").is_ok() {
        std::process::exit(1);
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        print_usage();
        return;
    }

    match args[1].as_str() {
        "analyze" => {
            if args.len() < 3 {
                eprintln!("Uso: bdja_cli analyze <ruta_archivo> [--json]");
                return;
            }
            let as_json = args.iter().any(|a| a == "--json");
            run_analyze(&args[2], as_json);
        }
        "scan" => {
            if args.len() < 3 {
                eprintln!("Uso: bdja_cli scan <ruta_directorio>");
                return;
            }
            run_scan(&args[2]);
        }
        "validate" => {
            if args.len() < 3 {
                eprintln!("Uso: bdja_cli validate <corpus_dir|manifiesto.csv>");
                eprintln!("     bdja_cli validate --manifest <manifiesto.csv>");
                eprintln!("     bdja_cli validate --lossless-dir <dir> --transcode-dir <dir>");
                return;
            }
            match parse_corpus_args(&args[2..]) {
                Ok((desc, samples)) => run_validate(&desc, samples),
                Err(e) => {
                    eprintln!("Error en parámetros de validación: {}", e);
                    std::process::exit(1);
                }
            }
        }
        "calibrate" => {
            if args.len() < 3 {
                eprintln!("Uso: bdja_cli calibrate <corpus_dir|manifiesto.csv>");
                eprintln!("     bdja_cli calibrate --manifest <manifiesto.csv>");
                eprintln!("     bdja_cli calibrate --lossless-dir <dir> --transcode-dir <dir>");
                return;
            }
            match parse_corpus_args(&args[2..]) {
                Ok((desc, samples)) => run_calibrate(&desc, samples),
                Err(e) => {
                    eprintln!("Error en parámetros de calibración: {}", e);
                    std::process::exit(1);
                }
            }
        }
        "version" | "-v" | "--version" => {
            println!(
                "bdja_cli v{} (Engine Rev {})",
                env!("CARGO_PKG_VERSION"),
                ENGINE_REV
            );
        }
        "help" | "-h" | "--help" => {
            print_usage();
        }
        other => {
            eprintln!("Comando desconocido: '{}'", other);
            print_usage();
        }
    }
}
