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
