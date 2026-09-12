use crate::pipeline::analyze_single_file;
use bdja_core::types::{FileReport, ENGINE_REV};
use bdja_store::ReportStore;
use jwalk::WalkDir;
use rayon::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

/// Extensiones analizables exhaustivamente por el motor DSP (decodificación vía Symphonia)
pub const ANALYZABLE_EXTENSIONS: &[&str] = &[
    "wav", "flac", "aif", "aiff", "mp3", "m4a", "aac", "ogg", "alac",
];

/// Extensiones de audio reconocidas en la biblioteca del DJ para mantener conteo exacto
/// de archivos, pero que no son decodificables actualmente por el backend DSP nativo.
pub const UNSUPPORTED_AUDIO_EXTENSIONS: &[&str] = &[
    "wma", "opus", "aifc", "caf", "mp4", "m4b", "oga", "mp2", "w64", "rf64", "mka", "wv", "ape",
    "tta", "dsf", "dff",
];

/// Mantenido por compatibilidad regresiva con la API pública
pub const SUPPORTED_EXTENSIONS: &[&str] = ANALYZABLE_EXTENSIONS;

/// Retorna true si el archivo puede ser analizado exhaustivamente por el DSP
pub fn is_analyzable(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|ext| ANALYZABLE_EXTENSIONS.contains(&ext.to_lowercase().as_str()))
        .unwrap_or(false)
}

/// Retorna true si el archivo es un formato de audio reconocido pero no soportado por el decodificador
pub fn is_unsupported_audio(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|ext| UNSUPPORTED_AUDIO_EXTENSIONS.contains(&ext.to_lowercase().as_str()))
        .unwrap_or(false)
}

/// Retorna true si el archivo es un archivo de audio para propósitos de inventario y escaneo
pub fn is_audio_file(path: &Path) -> bool {
    is_analyzable(path) || is_unsupported_audio(path)
}

pub fn scan_collection<F, P>(
    roots: &[PathBuf],
    throttle_mode: &str,
    skip_cache: bool,
    store: Option<Arc<ReportStore>>,
    cancel_token: Arc<AtomicBool>,
    on_file_analyzed: F,
    on_progress: P,
) -> Result<u64, String>
where
    F: Fn(FileReport) + Send + Sync,
    P: Fn(u64, u64, String) + Send + Sync,
{
    // 1. Fast Directory Walk & discovery with real-time feedback
    let mut discovered: Vec<PathBuf> = Vec::new();
    let mut discovery_report_ticker = 0usize;

    for root in roots {
        if cancel_token.load(Ordering::Relaxed) {
            return Ok(0);
        }
        if !root.exists() {
            continue;
        }
        if root.is_file() {
            if is_audio_file(root) {
                discovered.push(root.clone());
            }
            continue;
        }

        for entry in WalkDir::new(root).skip_hidden(true) {
            if cancel_token.load(Ordering::Relaxed) {
                break;
            }
            if let Ok(e) = entry {
                if e.file_type().is_file() && is_audio_file(&e.path()) {
                    let p = e.path();
                    discovered.push(p.clone());
                    discovery_report_ticker += 1;
                    if discovery_report_ticker >= 50 {
                        discovery_report_ticker = 0;
                        on_progress(
                            0,
                            discovered.len() as u64,
                            format!("Descubriendo: {}", p.to_string_lossy()),
                        );
                    }
                }
            }
        }
    }

    let total = discovered.len() as u64;
    if total == 0 {
        return Ok(0);
    }

    // 2. Concurrency pool configuration based on throttle mode
    let threads = match throttle_mode.to_lowercase().as_str() {
        "turbo" => num_cpus::get().max(1),
        "silent" => 1,
        _ => (num_cpus::get() / 2).clamp(2, 8),
    };

    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .map_err(|e| format!("Error al crear thread pool: {}", e))?;

    let analyzed_counter = Arc::new(AtomicU64::new(0));

    // 3. Parallel analysis with SQLite cache hit and hash deduplication bypass
    pool.install(|| {
        discovered.par_iter().for_each(|path| {
            if cancel_token.load(Ordering::Relaxed) {
                return;
            }

            let path_str = path.to_string_lossy().to_string();
            let count = analyzed_counter.fetch_add(1, Ordering::Relaxed) + 1;
            on_progress(count, total, path_str.clone());

            let meta_opt = std::fs::metadata(path).ok();
            let file_size = meta_opt.as_ref().map(|m| m.len()).unwrap_or(0);
            let mtime_utc = meta_opt
                .as_ref()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);

            // Check SQLite cache with path, size, mtime, and engine_rev
            let cached_report = if !skip_cache {
                if let Some(ref st) = store {
                    match st.get_cached_report(&path_str, file_size, mtime_utc, ENGINE_REV) {
                        Ok(Some(rep)) => Some(rep),
                        _ => {
                            // Deduplication by quick hash: check first 64 KB BLAKE3
                            if let Ok(mut file) = std::fs::File::open(path) {
                                let mut buffer = [0u8; 65536];
                                let n = std::io::Read::read(&mut file, &mut buffer).unwrap_or(0);
                                if n > 0 {
                                    let hash = blake3::hash(&buffer[..n]).to_hex().to_string();
                                    match st.get_cached_by_hash(&hash, ENGINE_REV) {
                                        Ok(Some(mut match_rep)) => {
                                            // Reutilizar resultado para copia en distinta ruta
                                            match_rep.path = path_str.clone();
                                            match_rep.file_size = file_size;
                                            let _ = st.save_report(&match_rep);
                                            Some(match_rep)
                                        }
                                        _ => None,
                                    }
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        }
                    }
                } else {
                    None
                }
            } else {
                None
            };

            // N-1: Safe execution with panic catching for corrupted/hostile files
            let report = match cached_report {
                Some(r) => r,
                None => {
                    if is_unsupported_audio(path) {
                        let ext = path
                            .extension()
                            .and_then(|s| s.to_str())
                            .unwrap_or("UNKNOWN")
                            .to_lowercase();
                        let ext_upper = ext.to_uppercase();
                        let mut unsupported_rep = bdja_core::types::FileReport {
                            file_id: 0,
                            path: path_str.clone(),
                            file_size,
                            engine_rev: ENGINE_REV,
                            facts: bdja_core::types::FormatFacts {
                                container: ext_upper.clone(),
                                codec: format!("No soportado ({})", ext),
                                codec_type: bdja_core::types::Codec::Unknown,
                                sample_rate: 0,
                                bit_depth: None,
                                channels: 0,
                                duration_ms: 0,
                                container_bitrate_kbps: None,
                                is_lossless_declared: false,
                            },
                            verdict: bdja_core::types::Verdict::Inconclusive,
                            confidence: 0.0,
                            score_llr: 0.0,
                            effective_bandwidth_hz: None,
                            cutoff_slope_db_oct: None,
                            evidences: Vec::new(),
                            quality: bdja_core::types::QualityMetrics {
                                true_peak_dbtp: None,
                                lufs_integrated: None,
                                clipped_samples: 0,
                                dc_offset: None,
                                dynamic_range_db: None,
                                stereo_correlation: None,
                            },
                            guards_triggered: vec![format!(
                                "Formato de audio no soportado actualmente por el motor ({})",
                                ext
                            )],
                            verdict_summary: format!(
                                "Formato de audio ({}) reconocido en la biblioteca pero no soportado por el decodificador",
                                ext_upper
                            ),
                            average_spectrum_db: Vec::new(),
                        };
                        if let Some(ref st) = store {
                            if let Ok(id) = st.save_report(&unsupported_rep) {
                                unsupported_rep.file_id = id;
                            }
                        }
                        unsupported_rep
                    } else {
                        let path_buf = path.to_path_buf();
                        let analyze_res =
                            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                analyze_single_file(&path_buf)
                            }));

                    match analyze_res {
                        Ok(Ok(mut new_rep)) => {
                            if let Some(ref st) = store {
                                if let Ok(id) = st.save_report(&new_rep) {
                                    new_rep.file_id = id;
                                }
                            }
                            new_rep
                        }
                        Ok(Err(err_msg)) => {
                            let mut err_rep = bdja_core::types::FileReport {
                                file_id: 0,
                                path: path_str.clone(),
                                file_size,
                                engine_rev: ENGINE_REV,
                                facts: bdja_core::types::FormatFacts {
                                    container: path
                                        .extension()
                                        .and_then(|s| s.to_str())
                                        .unwrap_or("UNKNOWN")
                                        .to_uppercase(),
                                    codec: "Error/Corrupted".to_string(),
                                    codec_type: bdja_core::types::Codec::Unknown,
                                    sample_rate: 0,
                                    bit_depth: None,
                                    channels: 0,
                                    duration_ms: 0,
                                    container_bitrate_kbps: None,
                                    is_lossless_declared: false,
                                },
                                verdict: bdja_core::types::Verdict::Inconclusive,
                                confidence: 0.0,
                                score_llr: 0.0,
                                effective_bandwidth_hz: None,
                                cutoff_slope_db_oct: None,
                                evidences: Vec::new(),
                                quality: bdja_core::types::QualityMetrics {
                                    true_peak_dbtp: None,
                                    lufs_integrated: None,
                                    clipped_samples: 0,
                                    dc_offset: None,
                                    dynamic_range_db: None,
                                    stereo_correlation: None,
                                },
                                guards_triggered: vec![
                                    "Fallo al decodificar audio (archivo ilegible o corrupto)".to_string(),
                                ],
                                verdict_summary: format!("Error analizando archivo: {}", err_msg),
                                average_spectrum_db: Vec::new(),
                            };
                            if let Some(ref st) = store {
                                if let Ok(id) = st.save_report(&err_rep) {
                                    err_rep.file_id = id;
                                }
                            }
                            err_rep
                        }
                        Err(_) => {
                            let mut err_rep = bdja_core::types::FileReport {
                                file_id: 0,
                                path: path_str.clone(),
                                file_size,
                                engine_rev: ENGINE_REV,
                                facts: bdja_core::types::FormatFacts {
                                    container: path
                                        .extension()
                                        .and_then(|s| s.to_str())
                                        .unwrap_or("UNKNOWN")
                                        .to_uppercase(),
                                    codec: "Panic/Corrupted".to_string(),
                                    codec_type: bdja_core::types::Codec::Unknown,
                                    sample_rate: 0,
                                    bit_depth: None,
                                    channels: 0,
                                    duration_ms: 0,
                                    container_bitrate_kbps: None,
                                    is_lossless_declared: false,
                                },
                                verdict: bdja_core::types::Verdict::Inconclusive,
                                confidence: 0.0,
                                score_llr: 0.0,
                                effective_bandwidth_hz: None,
                                cutoff_slope_db_oct: None,
                                evidences: Vec::new(),
                                quality: bdja_core::types::QualityMetrics {
                                    true_peak_dbtp: None,
                                    lufs_integrated: None,
                                    clipped_samples: 0,
                                    dc_offset: None,
                                    dynamic_range_db: None,
                                    stereo_correlation: None,
                                },
                                guards_triggered: vec![
                                    "Pánico interceptado por seguridad".to_string()
                                ],
                                verdict_summary:
                                    "Pánico no controlado durante la decodificación (archivo severamente malformado)"
                                        .to_string(),
                                average_spectrum_db: Vec::new(),
                            };
                            if let Some(ref st) = store {
                                if let Ok(id) = st.save_report(&err_rep) {
                                    err_rep.file_id = id;
                                }
                            }
                            err_rep
                        }
                    }
                }
            }
        };

            on_file_analyzed(report);
        });
    });

    Ok(analyzed_counter.load(Ordering::Relaxed))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;

    #[test]
    fn test_audio_extension_classification() {
        // Analizables
        assert!(is_analyzable(Path::new("track.wav")));
        assert!(is_analyzable(Path::new("track.FLAC")));
        assert!(is_analyzable(Path::new("track.mp3")));
        assert!(is_analyzable(Path::new("track.m4a")));
        assert!(is_analyzable(Path::new("track.aac")));
        assert!(is_analyzable(Path::new("track.ogg")));
        assert!(is_analyzable(Path::new("track.aif")));
        assert!(is_analyzable(Path::new("track.aiff")));
        assert!(is_analyzable(Path::new("track.alac")));

        assert!(!is_unsupported_audio(Path::new("track.wav")));
        assert!(is_audio_file(Path::new("track.wav")));

        // Reconocidos pero no soportados directamente por el backend DSP
        assert!(is_unsupported_audio(Path::new("track.wma")));
        assert!(is_unsupported_audio(Path::new("track.OPUS")));
        assert!(is_unsupported_audio(Path::new("track.mka")));
        assert!(is_unsupported_audio(Path::new("track.wv")));
        assert!(is_unsupported_audio(Path::new("track.ape")));
        assert!(is_unsupported_audio(Path::new("track.dsf")));

        assert!(!is_analyzable(Path::new("track.wma")));
        assert!(is_audio_file(Path::new("track.wma")));

        // No son audio
        assert!(!is_audio_file(Path::new("track.txt")));
        assert!(!is_audio_file(Path::new("track.pdf")));
        assert!(!is_audio_file(Path::new("track.cue")));
        assert!(!is_audio_file(Path::new("track.jpg")));
    }

    #[test]
    fn test_scan_unsupported_audio_generates_inconclusive_report_without_crash() {
        let temp_dir = std::env::temp_dir().join(format!("bdja_test_scan_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let dummy_opus = temp_dir.join("test_track.opus");
        let _ = std::fs::write(&dummy_opus, b"OggS_fake_opus_stream_data");

        let reports = Arc::new(std::sync::Mutex::new(Vec::new()));
        let reports_clone = Arc::clone(&reports);

        let cancel = Arc::new(AtomicBool::new(false));
        let count = scan_collection(
            &[dummy_opus.clone()],
            "silent",
            true,
            None,
            cancel,
            move |rep| {
                reports_clone.lock().unwrap().push(rep);
            },
            |_, _, _| {},
        );

        let _ = std::fs::remove_file(&dummy_opus);
        let _ = std::fs::remove_dir(&temp_dir);

        assert_eq!(count.unwrap(), 1);
        let locked = reports.lock().unwrap();
        assert_eq!(locked.len(), 1);
        let rep = &locked[0];
        assert_eq!(rep.verdict, bdja_core::types::Verdict::Inconclusive);
        assert_eq!(rep.facts.container, "OPUS");
        assert!(rep.facts.codec.contains("No soportado"));
        assert!(rep.guards_triggered.iter().any(|g| g.contains("opus")));
    }
}
