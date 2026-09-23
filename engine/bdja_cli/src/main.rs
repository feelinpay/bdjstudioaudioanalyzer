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
    println!("  bdja_cli validate <corpus_dir|manifiesto.csv> [--sweep-drop] Valida precisión, recall y tasa de falsos positivos (FPR)");
    println!("  bdja_cli validate --lossless-dir <dir> --transcode-dir <dir> [--sweep-drop] Valida usando dos carpetas independientes");
    println!("  bdja_cli calibrate <corpus_dir|manifiesto.csv> [--sweep-drop] Calcula distribución de LLR, percentiles y umbrales óptimos");
    println!("  bdja_cli calibrate --lossless-dir <dir> --transcode-dir <dir> [--sweep-drop] Calcula umbrales óptimos usando dos carpetas");
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Provenance {
    Lossless,
    Transcode,
    LossyConfirmed,
    ProvenanceUnknown,
}

#[derive(Debug, Clone)]
struct CorpusSample {
    path: PathBuf,
    provenance: Provenance,
    is_transcode: bool,
    codec_origen: Option<String>,
    bitrate: Option<String>,
    variant_id: Option<String>,
    master_source: Option<String>,
}

struct ExcludedSample {
    path: PathBuf,
    provenance: Provenance,
    verdict: bdja_core::types::Verdict,
    score_llr: f64,
    summary: String,
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
        if e.file_type().is_file() && bdja_scan::is_audio_file(&e.path()) {
            samples.push(CorpusSample {
                path: e.path(),
                provenance: if is_transcode {
                    Provenance::Transcode
                } else {
                    Provenance::Lossless
                },
                is_transcode,
                codec_origen: None,
                bitrate: None,
                variant_id: None,
                master_source: None,
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
            let raw_path = PathBuf::from(parts[0]);
            let file_path = if raw_path.is_relative() {
                if let Some(parent) = target_path.parent() {
                    parent.join(&raw_path)
                } else {
                    raw_path
                }
            } else {
                raw_path
            };

            let label = parts[1].to_lowercase();
            let provenance = match label.as_str() {
                "lossy_confirmed" => Provenance::LossyConfirmed,
                "provenance_unknown" => Provenance::ProvenanceUnknown,
                _ if label.contains("transcode")
                    || label.contains("fake")
                    || label.contains("lossy")
                    || label.contains("upsampled")
                    || label.contains("mp3")
                    || label.contains("aac")
                    || label == "1"
                    || label == "true" =>
                {
                    Provenance::Transcode
                }
                _ => Provenance::Lossless,
            };
            let is_transcode = provenance == Provenance::Transcode;

            let codec_origen = parts
                .get(2)
                .map(|s| s.to_string())
                .filter(|s| !s.is_empty());
            let bitrate = parts
                .get(3)
                .map(|s| s.to_string())
                .filter(|s| !s.is_empty());
            let variant_id = parts
                .get(4)
                .map(|s| s.to_string())
                .filter(|s| !s.is_empty());
            let master_source = parts
                .get(5)
                .map(|s| s.to_string())
                .filter(|s| !s.is_empty());

            samples.push(CorpusSample {
                path: file_path,
                provenance,
                is_transcode,
                codec_origen,
                bitrate,
                variant_id,
                master_source,
            });
        }
    } else {
        for e in jwalk::WalkDir::new(target_path)
            .skip_hidden(true)
            .into_iter()
            .flatten()
        {
            if e.file_type().is_file() && bdja_scan::is_audio_file(&e.path()) {
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
                        provenance: Provenance::Transcode,
                        is_transcode: true,
                        codec_origen: None,
                        bitrate: None,
                        variant_id: None,
                        master_source: None,
                    });
                } else if is_lossless {
                    samples.push(CorpusSample {
                        path: p,
                        provenance: Provenance::Lossless,
                        is_transcode: false,
                        codec_origen: None,
                        bitrate: None,
                        variant_id: None,
                        master_source: None,
                    });
                }
            }
        }
    }

    Ok(samples)
}

fn parse_corpus_args(args: &[String]) -> Result<(String, Vec<CorpusSample>, bool), String> {
    let mut lossless_dir: Option<&str> = None;
    let mut transcode_dir: Option<&str> = None;
    let mut manifest_path: Option<&str> = None;
    let mut positional: Option<&str> = None;
    let mut sweep_drop = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--sweep-drop" => {
                sweep_drop = true;
                i += 1;
            }
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
        return Ok((desc, samples, sweep_drop));
    }

    if let Some(target) = manifest_path.or(positional) {
        let samples = load_corpus_samples(target)?;
        let desc = format!("Corpus: '{}'", target);
        return Ok((desc, samples, sweep_drop));
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

fn run_validate(target_desc: &str, samples: Vec<CorpusSample>, sweep_drop: bool) {
    println!("============================================================");
    println!(" BDJ STUDIO AUDIO ANALYZER — VALIDACIÓN FORENSE DE CORPUS");
    println!("============================================================");
    println!("Objetivo: {}", target_desc);
    println!("Muestras encontradas para validación: {}", samples.len());

    if samples.is_empty() {
        println!("No se encontraron pistas etiquetadas en el corpus.");
        return;
    }

    let mut tp_strict = 0u64;
    let mut fn_strict = 0u64;
    let mut tn_strict = 0u64;
    let mut fp_strict = 0u64;

    let mut tp_perm = 0u64;
    let mut fn_perm = 0u64;
    let mut tn_perm = 0u64;
    let mut fp_perm = 0u64;

    let mut inconclusive = 0u64;
    let mut excluded_samples: Vec<ExcludedSample> = Vec::new();

    #[derive(Default)]
    struct VariantStats {
        total: u64,
        tp_strict: u64,
        tp_perm: u64,
        inconclusive: u64,
    }
    let mut variant_stats: std::collections::BTreeMap<String, VariantStats> =
        std::collections::BTreeMap::new();

    #[derive(Default)]
    struct MasterStats {
        lossless_ok: bool,
        lossless_evaluated: bool,
        lossless_verdict: Option<bdja_core::types::Verdict>,
        lossless_llr: f64,
        transcodes_total: u64,
        transcodes_strict_tp: u64,
        transcodes_perm_tp: u64,
        missed_variants_strict: Vec<String>,
        missed_variants_perm: Vec<String>,
    }
    let mut master_stats: std::collections::BTreeMap<String, MasterStats> =
        std::collections::BTreeMap::new();

    let start = Instant::now();
    for s in &samples {
        match bdja_scan::analyze_single_file(&s.path) {
            Ok(report) => {
                if s.provenance == Provenance::LossyConfirmed
                    || s.provenance == Provenance::ProvenanceUnknown
                {
                    excluded_samples.push(ExcludedSample {
                        path: s.path.clone(),
                        provenance: s.provenance,
                        verdict: report.verdict,
                        score_llr: report.score_llr,
                        summary: report.verdict_summary,
                    });
                    continue;
                }

                let is_strict_convicted =
                    report.verdict == bdja_core::types::Verdict::ProbableTranscode;
                let is_perm_convicted = matches!(
                    report.verdict,
                    bdja_core::types::Verdict::ProbableTranscode
                        | bdja_core::types::Verdict::Suspicious
                );
                let is_cleared = matches!(
                    report.verdict,
                    bdja_core::types::Verdict::LosslessVerified
                        | bdja_core::types::Verdict::LikelyLossless
                );
                let is_inc = report.verdict == bdja_core::types::Verdict::Inconclusive;

                if s.is_transcode {
                    if is_strict_convicted {
                        tp_strict += 1;
                    } else {
                        fn_strict += 1;
                    }

                    if is_perm_convicted {
                        tp_perm += 1;
                    } else {
                        fn_perm += 1;
                        if is_cleared {
                            println!(
                                "  [FALSO NEGATIVO] {} (LLR={:.2}, Veredicto: {:?})",
                                s.path.display(),
                                report.score_llr,
                                report.verdict
                            );
                        }
                    }

                    if is_inc {
                        inconclusive += 1;
                    }

                    let var_key = match (&s.codec_origen, &s.bitrate) {
                        (Some(c), Some(b)) => format!("{} {}", c, b),
                        (Some(c), None) => c.clone(),
                        (None, Some(b)) => b.clone(),
                        (None, None) => s.variant_id.clone().unwrap_or_default(),
                    };
                    if !var_key.is_empty() {
                        let entry = variant_stats.entry(var_key).or_default();
                        entry.total += 1;
                        if is_strict_convicted {
                            entry.tp_strict += 1;
                        }
                        if is_perm_convicted {
                            entry.tp_perm += 1;
                        }
                        if is_inc {
                            entry.inconclusive += 1;
                        }
                    }

                    if let Some(master_name) = &s.master_source {
                        let m_entry = master_stats.entry(master_name.clone()).or_default();
                        m_entry.transcodes_total += 1;
                        let var_label = match (&s.codec_origen, &s.bitrate) {
                            (Some(c), Some(b)) => format!("{} {}", c, b),
                            _ => s
                                .variant_id
                                .clone()
                                .unwrap_or_else(|| "transcode".to_string()),
                        };
                        if is_strict_convicted {
                            m_entry.transcodes_strict_tp += 1;
                        } else {
                            m_entry.missed_variants_strict.push(var_label.clone());
                        }
                        if is_perm_convicted {
                            m_entry.transcodes_perm_tp += 1;
                        } else {
                            m_entry.missed_variants_perm.push(var_label);
                        }
                    }
                } else {
                    if is_strict_convicted {
                        fp_strict += 1;
                        println!(
                            "  [FALSO POSITIVO ESTRICTO] {} (LLR={:.2})",
                            s.path.display(),
                            report.score_llr
                        );
                    } else {
                        tn_strict += 1;
                    }

                    if is_perm_convicted {
                        fp_perm += 1;
                        println!(
                            "  [FALSO POSITIVO PERMISIVO] {} (LLR={:.2}, Veredicto: {:?})",
                            s.path.display(),
                            report.score_llr,
                            report.verdict
                        );
                    } else {
                        tn_perm += 1;
                    }

                    if is_inc {
                        inconclusive += 1;
                    }

                    if let Some(master_name) = &s.master_source {
                        let m_entry = master_stats.entry(master_name.clone()).or_default();
                        m_entry.lossless_evaluated = true;
                        m_entry.lossless_verdict = Some(report.verdict);
                        m_entry.lossless_llr = report.score_llr;
                        m_entry.lossless_ok = !is_strict_convicted && !is_perm_convicted;
                    }
                }
            }
            Err(e) => {
                println!("  [ERROR DECODE] {}: {}", s.path.display(), e);
                inconclusive += 1;
            }
        }
    }

    let elapsed = start.elapsed();

    // Métricas Frontera Estricta (ProbableTranscode, LLR >= +4.0)
    let recall_strict = if tp_strict + fn_strict > 0 {
        (tp_strict as f64) / ((tp_strict + fn_strict) as f64) * 100.0
    } else {
        0.0
    };
    let fpr_strict = if fp_strict + tn_strict > 0 {
        (fp_strict as f64) / ((fp_strict + tn_strict) as f64) * 100.0
    } else {
        0.0
    };
    let precision_strict = if tp_strict + fp_strict > 0 {
        (tp_strict as f64) / ((tp_strict + fp_strict) as f64) * 100.0
    } else {
        0.0
    };

    // Métricas Frontera Permisiva (Suspicious o superior, LLR >= +1.5)
    let total_classified_perm = tp_perm + fn_perm + tn_perm + fp_perm;
    let recall_perm = if tp_perm + fn_perm > 0 {
        (tp_perm as f64) / ((tp_perm + fn_perm) as f64) * 100.0
    } else {
        0.0
    };
    let fpr_perm = if fp_perm + tn_perm > 0 {
        (fp_perm as f64) / ((fp_perm + tn_perm) as f64) * 100.0
    } else {
        0.0
    };
    let precision_perm = if tp_perm + fp_perm > 0 {
        (tp_perm as f64) / ((tp_perm + fp_perm) as f64) * 100.0
    } else {
        0.0
    };
    let accuracy_perm = if total_classified_perm > 0 {
        ((tp_perm + tn_perm) as f64) / (total_classified_perm as f64) * 100.0
    } else {
        0.0
    };

    println!("\n------------------------------------------------------------");
    println!(" RESULTADOS DE VALIDACIÓN (B-9: Desglose por Frontera)");
    println!("------------------------------------------------------------");
    println!(
        "  Total evaluados:             {}",
        samples.len() - excluded_samples.len()
    );
    if !excluded_samples.is_empty() {
        println!(
            "  Total excluidos auditados:   {}",
            excluded_samples.len()
        );
    }
    println!("  Casos Inconclusos:           {}", inconclusive);
    println!("\n  [1] FRONTERA ESTRICTA (ProbableTranscode, LLR >= +4.0):");
    println!("    Verdaderos Positivos (TP):   {}", tp_strict);
    println!("    Falsos Negativos (FN):       {}", fn_strict);
    println!("    Verdaderos Negativos (TN):   {}", tn_strict);
    println!("    Falsos Positivos (FP):       {}", fp_strict);
    let n_neg_strict = fp_strict + tn_strict;
    let bound_95_strict = if n_neg_strict > 0 {
        (3.0 / n_neg_strict as f64) * 100.0
    } else {
        100.0
    };
    if fp_strict == 0 && n_neg_strict < 300 {
        println!(
            "    Tasa Falsos Positivos (FPR): {:.2}% [Cota sup. 95%: <= {:.2}% con N={}] (Objetivo <= 1.0% requiere N >= 300)",
            fpr_strict, bound_95_strict, n_neg_strict
        );
    } else {
        println!(
            "    Tasa Falsos Positivos (FPR): {:.2}% (Objetivo <= 1.0%)",
            fpr_strict
        );
    }
    println!("    Precisión (PPV):             {:.2}%", precision_strict);
    println!("\n  [2] FRONTERA PERMISIVA (Suspicious o superior, LLR >= +1.5):");
    println!("    Verdaderos Positivos (TP):   {}", tp_perm);
    println!("    Falsos Negativos (FN):       {}", fn_perm);
    println!("    Verdaderos Negativos (TN):   {}", tn_perm);
    println!("    Falsos Positivos (FP):       {}", fp_perm);
    println!("    Sensibilidad / Recall:       {:.2}%", recall_perm);
    let n_neg_perm = fp_perm + tn_perm;
    let bound_95_perm = if n_neg_perm > 0 {
        (3.0 / n_neg_perm as f64) * 100.0
    } else {
        100.0
    };
    if fp_perm == 0 && n_neg_perm < 300 {
        println!(
            "    Tasa Falsos Positivos (FPR): {:.2}% [Cota sup. 95%: <= {:.2}% con N={}] (Objetivo <= 1.0% requiere N >= 300)",
            fpr_perm, bound_95_perm, n_neg_perm
        );
    } else {
        println!(
            "    Tasa Falsos Positivos (FPR): {:.2}% (Objetivo <= 1.0%)",
            fpr_perm
        );
    }
    println!("    Precisión (PPV):             {:.2}%", precision_perm);
    println!("    Exactitud Global:            {:.2}%", accuracy_perm);

    if !variant_stats.is_empty() {
        println!("\n  [3] DESGLOSE DE RECALL POR VARIANTE / BITRATE:");
        println!(
            "    {:<18} {:>8}   {:>15}   {:>15}   {:>12}",
            "Variante", "Muestras", "Recall Estricto", "Recall Permisivo", "Inconclusos"
        );
        println!(
            "    {:-<18} {:-<8}   {:-<15}   {:-<15}   {:-<12}",
            "", "", "", "", ""
        );
        for (var_name, stats) in &variant_stats {
            let r_strict = if stats.total > 0 {
                (stats.tp_strict as f64) / (stats.total as f64) * 100.0
            } else {
                0.0
            };
            let r_perm = if stats.total > 0 {
                (stats.tp_perm as f64) / (stats.total as f64) * 100.0
            } else {
                0.0
            };
            println!(
                "    {:<18} {:>8}   {:>14.1}%   {:>14.1}%   {:>12}",
                var_name, stats.total, r_strict, r_perm, stats.inconclusive
            );
        }
    }

    if !master_stats.is_empty() {
        let failing_masters: Vec<(&String, &MasterStats)> = master_stats
            .iter()
            .filter(|(_, st)| {
                (st.lossless_evaluated && !st.lossless_ok)
                    || st.transcodes_strict_tp < st.transcodes_total
                    || st.transcodes_perm_tp < st.transcodes_total
            })
            .collect();

        println!("\n  [4] DESGLOSE POR MÁSTER (MÁSTERS CON CASOS DIFÍCILES O FALLOS):");
        if failing_masters.is_empty() {
            println!(
                "    Todos los másters evaluados ({}) obtuvieron 100% de detección en sus variantes sin falsos positivos.",
                master_stats.len()
            );
        } else {
            println!(
                "    {:<35} {:>14}   {:>14}   {:<35}",
                "Máster Origen", "Cazados (Est)", "Cazados (Perm)", "Detalle de Fallos"
            );
            println!("    {:-<35} {:-<14}   {:-<14}   {:-<35}", "", "", "", "");
            for (m_name, st) in failing_masters {
                let ratio_strict = format!("{}/{}", st.transcodes_strict_tp, st.transcodes_total);
                let ratio_perm = format!("{}/{}", st.transcodes_perm_tp, st.transcodes_total);
                let mut failure_notes = Vec::new();
                if st.lossless_evaluated && !st.lossless_ok {
                    failure_notes.push(format!("FP en máster (LLR={:.2})", st.lossless_llr));
                }
                if !st.missed_variants_perm.is_empty() {
                    failure_notes
                        .push(format!("Miss perm: {}", st.missed_variants_perm.join(", ")));
                } else if st.transcodes_strict_tp < st.transcodes_total {
                    failure_notes.push("Solo falló en estricto".to_string());
                }
                println!(
                    "    {:<35} {:>14}   {:>14}   {:<35}",
                    m_name,
                    ratio_strict,
                    ratio_perm,
                    failure_notes.join(" | ")
                );
            }
        }
    }

    if !excluded_samples.is_empty() {
        println!("\n  [5] ARCHIVOS EXCLUIDOS POR PROCEDENCIA AUDITADA (NO ENTRAN A FPR/RECALL):");
        println!(
            "    {:<35} {:<20} {:>7}   {:<22}   {:<35}",
            "Archivo", "Procedencia", "LLR", "Veredicto", "Diagnóstico del Motor"
        );
        println!(
            "    {:-<35} {:-<20} {:-<7}   {:-<22}   {:-<35}",
            "", "", "", "", ""
        );
        for ex in &excluded_samples {
            let filename = ex.path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            let prov_str = match ex.provenance {
                Provenance::LossyConfirmed => "lossy_confirmed",
                Provenance::ProvenanceUnknown => "provenance_unknown",
                _ => "otro",
            };
            println!(
                "    {:<35} {:<20} {:>+7.2}   {:<22}   {:<35}",
                filename,
                prov_str,
                ex.score_llr,
                format!("{:?}", ex.verdict),
                ex.summary
            );
        }
    }

    println!("------------------------------------------------------------");
    println!("  Tiempo transcurrido:         {:.2?}", elapsed);
    println!("------------------------------------------------------------");

    if fpr_strict <= 1.0 && recall_strict >= 95.0 && n_neg_strict >= 300 {
        println!("  [GATE STATUS: PASS] El motor cumple las tolerancias en frontera estricta (Recall >= 95.0%, FPR <= 1.0% con N >= 300).");
    } else {
        let n_note = if n_neg_strict < 300 {
            format!(" [N_neg={} insuficiente para certificar FPR <= 1.0%]", n_neg_strict)
        } else {
            String::new()
        };
        println!(
            "  [GATE STATUS: FAIL] El motor no alcanza el objetivo de producción en frontera estricta (Recall estricto: {:.2}% vs >= 95.0%, FPR: {:.2}% vs <= 1.0%){}.",
            recall_strict, fpr_strict, n_note
        );
    }
    println!("============================================================");

    if sweep_drop {
        run_sweep_drop_analysis(&samples);
    }
}

fn run_calibrate(target_desc: &str, samples: Vec<CorpusSample>, sweep_drop: bool) {
    println!("============================================================");
    println!(" BDJ STUDIO AUDIO ANALYZER — CALIBRACIÓN FORENSE DE CORPUS");
    println!("============================================================");
    println!("Objetivo: {}", target_desc);
    println!("Muestras cargadas: {}", samples.len());

    let mut lossless_scores: Vec<f64> = Vec::new();
    let mut transcode_scores: Vec<f64> = Vec::new();
    let mut excluded_count = 0usize;

    let start = Instant::now();
    for s in &samples {
        if s.provenance == Provenance::LossyConfirmed
            || s.provenance == Provenance::ProvenanceUnknown
        {
            excluded_count += 1;
            continue;
        }

        if let Ok(report) = bdja_scan::analyze_single_file(&s.path) {
            if s.is_transcode {
                transcode_scores.push(report.score_llr);
            } else {
                lossless_scores.push(report.score_llr);
            }
        }
    }

    if excluded_count > 0 {
        println!(
            "  [Procedencia] {} archivos excluidos de la calibración por auditoría de procedencia (lossy_confirmed / provenance_unknown).",
            excluded_count
        );
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
        "{:<20} | {:<+22.2} | {:<+22.2}",
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
    let bound_95_cal = if n_lossless > 0 {
        (3.0 / n_lossless as f64) * 100.0
    } else {
        100.0
    };
    if best_fpr == 0.0 && n_lossless < 300 {
        println!(
            "  Tasa Falsos Positivos (FPR): {:.2}% [Cota sup. 95%: <= {:.2}% con N={}] (Objetivo <= 1.0% requiere N >= 300)",
            best_fpr, bound_95_cal, n_lossless
        );
    } else {
        println!(
            "  Tasa Falsos Positivos (FPR): {:.2}% (Objetivo <= 1.0%)",
            best_fpr
        );
    }
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

    if sweep_drop {
        run_sweep_drop_analysis(&samples);
    }

    if !is_pass && std::env::var("BDJA_STRICT_GATE").is_ok() {
        std::process::exit(1);
    }
}

fn run_sweep_drop_analysis(samples: &[CorpusSample]) {
    println!("\n------------------------------------------------------------");
    println!(" [HISTOGRAMA Y BARRIDO DE CAÍDAS ESPECTRALES (14–21.5 kHz)]");
    println!("------------------------------------------------------------");

    let mut inconclusos_drops: Vec<(String, f64, f64, u32)> = Vec::new();
    let mut detected_drops: Vec<(String, f64, f64, u32)> = Vec::new();
    let mut lossless_drops: Vec<(String, f64, f64, u32)> = Vec::new();

    println!("  Midiendo caídas espectrales y pendientes...");
    for s in samples {
        if s.provenance != Provenance::Transcode && s.provenance != Provenance::Lossless {
            continue;
        }

        let rep = match bdja_scan::analyze_single_file(&s.path) {
            Ok(r) => r,
            Err(_) => continue,
        };

        let points = match bdja_scan::inspect_file_spectral_drops(&s.path) {
            Ok(pts) => pts,
            Err(_) => continue,
        };

        let filename = s
            .path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        let band_points: Vec<&bdja_dsp::SpectralDropPoint> = points
            .iter()
            .filter(|p| p.freq_hz >= 14_000.0 && p.freq_hz <= 21_500.0)
            .collect();

        let max_pt = band_points.iter().cloned().max_by(|a, b| {
            a.drop_db
                .partial_cmp(&b.drop_db)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let (drop, slope, freq) = if let Some(pt) = max_pt {
            (pt.drop_db, pt.slope_db_oct, pt.freq_hz.round() as u32)
        } else {
            (0.0, 0.0, 0)
        };

        if s.provenance == Provenance::Transcode {
            if rep.verdict == bdja_core::types::Verdict::Inconclusive
                || rep.verdict == bdja_core::types::Verdict::LikelyLossless
                || rep.verdict == bdja_core::types::Verdict::LosslessVerified
            {
                inconclusos_drops.push((filename, drop, slope, freq));
            } else {
                detected_drops.push((filename, drop, slope, freq));
            }
        } else if s.provenance == Provenance::Lossless {
            lossless_drops.push((filename, drop, slope, freq));
        }
    }

    let bins: &[(f64, f64, &str)] = &[
        (0.0, 6.0, "< 6.0 dB"),
        (6.0, 7.0, "6.0 - 7.0 dB"),
        (7.0, 8.0, "7.0 - 8.0 dB"),
        (8.0, 9.0, "8.0 - 9.0 dB"),
        (9.0, 10.0, "9.0 - 10.0 dB"),
        (10.0, 11.0, "10.0 - 11.0 dB"),
        (11.0, 12.0, "11.0 - 12.0 dB"),
        (12.0, 13.0, "12.0 - 13.0 dB"),
        (13.0, 14.0, "13.0 - 14.0 dB"),
        (14.0, 15.0, "14.0 - 15.0 dB"),
        (15.0, 17.0, "15.0 - 17.0 dB"),
        (17.0, 100.0, ">= 17.0 dB"),
    ];

    println!("\n  [HISTOGRAMA DE CAÍDAS MÁXIMAS EN BANDA 14–21.5 kHz]:");
    println!(
        "    {:<16} {:>24} {:>20} {:>20}",
        "Rango de Caída",
        format!("Inconclusos ({})", inconclusos_drops.len()),
        format!("Detectados ({})", detected_drops.len()),
        format!("Lossless ({})", lossless_drops.len())
    );
    println!(
        "    {:-<16} {:-<24} {:-<20} {:-<20}",
        "", "", "", ""
    );

    for &(low, high, label) in bins {
        let inc_c = inconclusos_drops
            .iter()
            .filter(|(_, d, _, _)| *d >= low && *d < high)
            .count();
        let det_c = detected_drops
            .iter()
            .filter(|(_, d, _, _)| *d >= low && *d < high)
            .count();
        let los_c = lossless_drops
            .iter()
            .filter(|(_, d, _, _)| *d >= low && *d < high)
            .count();
        println!(
            "    {:<16} {:>24} {:>20} {:>20}",
            label, inc_c, det_c, los_c
        );
    }

    println!("\n  [BARRIDO DE UMBRAL min_drop (SIMULACIÓN DE CAPTURA EN 14–21.5 kHz)]:");
    println!(
        "    {:<12} {:>22} {:>20} {:>18}",
        "min_drop", "Inconclusos Cazables", "Falsos Positivos", "Margen"
    );
    println!("    {:-<12} {:-<22} {:-<20} {:-<18}", "", "", "", "");

    let thresholds = [8.0, 9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0, 17.0];
    for &th in &thresholds {
        let inc_caught = inconclusos_drops
            .iter()
            .filter(|(_, d, _, _)| *d >= th)
            .count();
        let los_fp = lossless_drops
            .iter()
            .filter(|(_, d, _, _)| *d >= th)
            .count();
        let verdict = if los_fp == 0 {
            "LIMPIO (0 FP)".to_string()
        } else {
            format!("RIESGO ({} FP)", los_fp)
        };
        println!(
            "    {:<12.1} {:>22} {:>20} {:>18}",
            th, inc_caught, los_fp, verdict
        );
    }

    lossless_drops.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    println!("\n  [TOP CAÍDAS ESPECTRALES EN LOSSLESS CONFIRMADOS (PISO DE MÁSTER)]:");
    for (name, drop, slope, freq) in lossless_drops.iter().take(5) {
        println!(
            "    * {:<38} Caída: {:>5.1} dB a {:>5} Hz (Pendiente: {:>5.1} dB/oct)",
            name, drop, freq, slope
        );
    }

    inconclusos_drops.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    println!("\n  [TOP CAÍDAS EN TRANSCODES INCONCLUSOS (POTENCIALES CAZADOS)]:");
    for (name, drop, slope, freq) in inconclusos_drops.iter().take(8) {
        println!(
            "    * {:<38} Caída: {:>5.1} dB a {:>5} Hz (Pendiente: {:>5.1} dB/oct)",
            name, drop, freq, slope
        );
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
                eprintln!("Uso: bdja_cli validate <corpus_dir|manifiesto.csv> [--sweep-drop]");
                eprintln!("     bdja_cli validate --manifest <manifiesto.csv> [--sweep-drop]");
                eprintln!("     bdja_cli validate --lossless-dir <dir> --transcode-dir <dir> [--sweep-drop]");
                return;
            }
            match parse_corpus_args(&args[2..]) {
                Ok((desc, samples, sweep_drop)) => run_validate(&desc, samples, sweep_drop),
                Err(e) => {
                    eprintln!("Error en parámetros de validación: {}", e);
                    std::process::exit(1);
                }
            }
        }
        "calibrate" => {
            if args.len() < 3 {
                eprintln!("Uso: bdja_cli calibrate <corpus_dir|manifiesto.csv> [--sweep-drop]");
                eprintln!("     bdja_cli calibrate --manifest <manifiesto.csv> [--sweep-drop]");
                eprintln!("     bdja_cli calibrate --lossless-dir <dir> --transcode-dir <dir> [--sweep-drop]");
                return;
            }
            match parse_corpus_args(&args[2..]) {
                Ok((desc, samples, sweep_drop)) => run_calibrate(&desc, samples, sweep_drop),
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
