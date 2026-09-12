use bdja_core::types::ENGINE_REV;
use bdja_core::types::FileReport;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

fn print_usage() {
    println!("BDJ Studio Audio Analyzer — CLI Forense v{} (Rev {})", env!("CARGO_PKG_VERSION"), ENGINE_REV);
    println!("\nUSO:");
    println!("  bdja_cli analyze <ruta_archivo> [--json]   Analiza un archivo individual de audio");
    println!("  bdja_cli scan <ruta_directorio>            Escanea una carpeta o biblioteca de audio");
    println!("  bdja_cli validate <directorio_corpus>      Valida precisión, recall y tasa de falsos positivos (FPR)");
    println!("  bdja_cli calibrate <directorio_corpus>     Calcula distribución de LLR y umbrales óptimos");
    println!("  bdja_cli version                           Muestra la revisión del motor y versión");
    println!("  bdja_cli help                              Muestra esta ayuda");
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
                println!("Tamaño:            {:.2} MB", report.file_size as f64 / (1024.0 * 1024.0));
                println!("Contenedor:        {} ({})", report.facts.container, report.facts.codec);
                println!("Frecuencia:        {} Hz", report.facts.sample_rate);
                println!("Profundidad:       {} bits", report.facts.bit_depth.unwrap_or(16));
                println!("Canales:           {}", report.facts.channels);
                println!("Bitrate:           {} kbps", report.facts.container_bitrate_kbps.unwrap_or(0));
                println!("------------------------------------------------------------");
                println!("VEREDICTO:         {} (Confianza: {:.1}%)", report.verdict.display_name(), report.confidence * 100.0);
                println!("Score LLR:         {:.2}", report.score_llr);
                println!("Ancho de banda:    {} Hz", report.effective_bandwidth_hz.unwrap_or(0));
                println!("Pendiente corte:   {:.1} dB/oct", report.cutoff_slope_db_oct.unwrap_or(0.0));
                println!("Resumen:           {}", report.verdict_summary);
                println!("------------------------------------------------------------");
                println!("EVIDENCIAS DETECTADAS:");
                for ev in &report.evidences {
                    let code_str = format!("{:?}", ev.code);
                    let strong_mark = if ev.code.is_strong() { " [FUERTE]" } else { "" };
                    println!("  * {}{}: LLR={:+.2} — {}", code_str, strong_mark, ev.llr, ev.description);
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
                println!("  True Peak:         {:.2} dBTP", report.quality.true_peak_dbtp.unwrap_or(0.0));
                println!("  LUFS Integrado:    {:.1} LUFS", report.quality.lufs_integrated.unwrap_or(-14.0));
                println!("  Clipping:          {} muestras consecutivas", report.quality.clipped_samples);
                println!("  Rango Dinámico:    {:.1} dB", report.quality.dynamic_range_db.unwrap_or(0.0));
                println!("  Offset DC:         {:.5}", report.quality.dc_offset.unwrap_or(0.0));
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

    let on_file_analyzed = move |report: FileReport| {
        match report.verdict {
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
        }
    };

    let on_progress = |done: u64, total: u64, current: String| {
        let name = Path::new(&current)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy();
        print!("\rProgreso: {}/{} pistas | {}                  ", done, total, name);
        let _ = std::io::stdout().flush();
    };

    match bdja_scan::scan_collection(&roots, "turbo", false, None, cancel, on_file_analyzed, on_progress) {
        Ok(processed) => {
            let elapsed = start.elapsed();
            println!("\n\n============================================================");
            println!(" RESUMEN DE ESCANEO MASIVO");
            println!("============================================================");
            println!("Total pistas procesadas:    {}", processed);
            println!("Lossless verificados:       {}", lossless_count.load(Ordering::Relaxed));
            println!("Sospechosos:                {}", suspicious_count.load(Ordering::Relaxed));
            println!("Probables transcodes:       {}", transcode_count.load(Ordering::Relaxed));
            println!("Otros formatos / lossy:     {}", other_count.load(Ordering::Relaxed));
            println!("Tiempo total transcurrido:  {:.2?}", elapsed);
            println!("============================================================");
        }
        Err(e) => {
            eprintln!("\nError durante el escaneo: {}", e);
            std::process::exit(1);
        }
    }
}

fn run_validate(dir_str: &str) {
    let path = Path::new(dir_str);
    if !path.exists() {
        eprintln!("Error: el directorio del corpus '{}' no existe.", dir_str);
        std::process::exit(1);
    }

    println!("============================================================");
    println!(" BDJ STUDIO AUDIO ANALYZER — VALIDACIÓN FORENSE DE CORPUS");
    println!("============================================================");
    println!("Directorio de corpus: {}", dir_str);

    let mut tp = 0u64;
    let mut fn_count = 0u64;
    let mut tn = 0u64;
    let mut fp = 0u64;
    let mut inconclusive = 0u64;

    let mut all_files = Vec::new();
    for entry in jwalk::WalkDir::new(path).skip_hidden(true) {
        if let Ok(e) = entry {
            if e.file_type().is_file() && bdja_scan::is_audio_file(&e.path()) {
                all_files.push(e.path());
            }
        }
    }

    println!("Pistas encontradas para validación: {}", all_files.len());
    if all_files.is_empty() {
        println!("No se encontraron archivos de audio soportados.");
        return;
    }

    let start = Instant::now();
    for p in &all_files {
        let path_lower = p.to_string_lossy().to_lowercase();
        let is_expected_transcode = path_lower.contains("transcode")
            || path_lower.contains("fake")
            || path_lower.contains("upsampled")
            || path_lower.contains("mp3_")
            || path_lower.contains("aac_");

        let is_expected_lossless = path_lower.contains("lossless")
            || path_lower.contains("genuine")
            || path_lower.contains("authentic")
            || path_lower.contains("master");

        if let Ok(report) = bdja_scan::analyze_single_file(p) {
            let is_convicted = matches!(
                report.verdict,
                bdja_core::types::Verdict::ProbableTranscode | bdja_core::types::Verdict::Suspicious
            );
            let is_cleared = matches!(
                report.verdict,
                bdja_core::types::Verdict::LosslessVerified | bdja_core::types::Verdict::LikelyLossless
            );

            if is_expected_transcode {
                if is_convicted {
                    tp += 1;
                } else if is_cleared {
                    fn_count += 1;
                    println!("  [FALSO NEGATIVO] {} (LLR={:.2})", p.display(), report.score_llr);
                } else {
                    inconclusive += 1;
                }
            } else if is_expected_lossless {
                if is_cleared {
                    tn += 1;
                } else if is_convicted {
                    fp += 1;
                    println!("  [FALSO POSITIVO] {} (LLR={:.2})", p.display(), report.score_llr);
                } else {
                    inconclusive += 1;
                }
            }
        }
    }

    let elapsed = start.elapsed();
    let total_classified = tp + fn_count + tn + fp;
    let recall = if tp + fn_count > 0 { (tp as f64) / ((tp + fn_count) as f64) * 100.0 } else { 0.0 };
    let fpr = if fp + tn > 0 { (fp as f64) / ((fp + tn) as f64) * 100.0 } else { 0.0 };
    let precision = if tp + fp > 0 { (tp as f64) / ((tp + fp) as f64) * 100.0 } else { 0.0 };

    println!("\n------------------------------------------------------------");
    println!(" RESULTADOS DE VALIDACIÓN (Matriz de Confusión)");
    println!("------------------------------------------------------------");
    println!("  Total clasificados:          {}", total_classified);
    println!("  Verdaderos Positivos (TP):   {}", tp);
    println!("  Falsos Negativos (FN):       {}", fn_count);
    println!("  Verdaderos Negativos (TN):   {}", tn);
    println!("  Falsos Positivos (FP):       {}", fp);
    println!("  Casos Inconclusos:           {}", inconclusive);
    println!("------------------------------------------------------------");
    println!("  Sensibilidad / Recall:       {:.2}%", recall);
    println!("  Tasa Falsos Positivos (FPR): {:.2}%", fpr);
    println!("  Precisión (PPV):             {:.2}%", precision);
    println!("  Tiempo de ejecución:         {:.2?}", elapsed);
    println!("============================================================");
}

fn run_calibrate(dir_str: &str) {
    let path = Path::new(dir_str);
    if !path.exists() {
        eprintln!("Error: el directorio del corpus '{}' no existe.", dir_str);
        std::process::exit(1);
    }

    println!("============================================================");
    println!(" BDJ STUDIO AUDIO ANALYZER — CALIBRACIÓN LLR DE CORPUS");
    println!("============================================================");

    let mut lossless_scores = Vec::new();
    let mut transcode_scores = Vec::new();

    for entry in jwalk::WalkDir::new(path).skip_hidden(true) {
        if let Ok(e) = entry {
            if e.file_type().is_file() && bdja_scan::is_audio_file(&e.path()) {
                let p = e.path();
                let path_lower = p.to_string_lossy().to_lowercase();
                let is_transcode = path_lower.contains("transcode") || path_lower.contains("fake") || path_lower.contains("mp3");
                let is_lossless = path_lower.contains("lossless") || path_lower.contains("genuine") || path_lower.contains("master");

                if let Ok(rep) = bdja_scan::analyze_single_file(&p) {
                    if is_transcode {
                        transcode_scores.push(rep.score_llr);
                    } else if is_lossless {
                        lossless_scores.push(rep.score_llr);
                    }
                }
            }
        }
    }

    println!("Muestras analizadas: {} lossless, {} transcodes", lossless_scores.len(), transcode_scores.len());

    let avg = |v: &[f64]| if v.is_empty() { 0.0 } else { v.iter().sum::<f64>() / v.len() as f64 };
    let avg_lossless = avg(&lossless_scores);
    let avg_transcode = avg(&transcode_scores);

    println!("  Media LLR Lossless legítimos:   {:+.2}", avg_lossless);
    println!("  Media LLR Transcodes:          {:+.2}", avg_transcode);
    println!("  Margen de separación (Delta):   {:.2} LLR", avg_transcode - avg_lossless);
    println!("------------------------------------------------------------");
    println!("Calibración completada con éxito.");
    println!("============================================================");
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
                eprintln!("Uso: bdja_cli validate <directorio_corpus>");
                return;
            }
            run_validate(&args[2]);
        }
        "calibrate" => {
            if args.len() < 3 {
                eprintln!("Uso: bdja_cli calibrate <directorio_corpus>");
                return;
            }
            run_calibrate(&args[2]);
        }
        "version" | "-v" | "--version" => {
            println!("bdja_cli v{} (Engine Rev {})", env!("CARGO_PKG_VERSION"), ENGINE_REV);
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
