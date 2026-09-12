use crate::pipeline::analyze_single_file;
use bdja_core::types::{FileReport, ENGINE_REV};
use bdja_store::ReportStore;
use jwalk::WalkDir;
use rayon::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

pub const SUPPORTED_EXTENSIONS: &[&str] = &[
    "wav", "flac", "aif", "aiff", "mp3", "m4a", "aac", "ogg", "alac", "wma",
];

pub fn is_audio_file(path: &Path) -> bool {
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        let ext_lower = ext.to_lowercase();
        SUPPORTED_EXTENSIONS.contains(&ext_lower.as_str())
    } else {
        false
    }
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
                        _ => return, // Continúa de forma segura si un archivo está dañado
                    }
                }
            };

            on_file_analyzed(report);
        });
    });

    Ok(analyzed_counter.load(Ordering::Relaxed))
}
