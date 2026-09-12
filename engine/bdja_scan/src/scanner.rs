use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use jwalk::WalkDir;
use rayon::prelude::*;
use bdja_core::types::{FileReport, ENGINE_REV};
use bdja_store::ReportStore;
use crate::pipeline::analyze_single_file;

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
    // 1. Fast Directory Walk & discovery
    let mut discovered: Vec<PathBuf> = Vec::new();
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
                    discovered.push(e.path());
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

    // 3. Parallel analysis with SQLite cache hit bypass
    pool.install(|| {
        discovered.par_iter().for_each(|path| {
            if cancel_token.load(Ordering::Relaxed) {
                return;
            }

            let path_str = path.to_string_lossy().to_string();
            let count = analyzed_counter.fetch_add(1, Ordering::Relaxed) + 1;
            on_progress(count, total, path_str.clone());

            // Check SQLite cache
            let cached_report = if !skip_cache {
                if let Some(ref st) = store {
                    if let Ok(Some(rep)) = st.get_report_by_path(&path_str) {
                        if let Ok(meta) = std::fs::metadata(path) {
                            if rep.file_size == meta.len() && rep.engine_rev == ENGINE_REV {
                                Some(rep)
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            };

            let report = match cached_report {
                Some(r) => r,
                None => match analyze_single_file(path) {
                    Ok(mut new_rep) => {
                        if let Some(ref st) = store {
                            if let Ok(id) = st.save_report(&new_rep) {
                                new_rep.file_id = id;
                            }
                        }
                        new_rep
                    }
                    Err(_) => return,
                },
            };

            on_file_analyzed(report);
        });
    });

    Ok(analyzed_counter.load(Ordering::Relaxed))
}

pub fn scan_directory<F, P>(
    root: &Path,
    cancel_token: Arc<AtomicBool>,
    mut on_file_analyzed: F,
    mut on_progress: P,
) -> Result<u64, String>
where
    F: FnMut(FileReport),
    P: FnMut(u64, u64, String),
{
    let mut found_files: Vec<PathBuf> = Vec::new();

    for entry in WalkDir::new(root).skip_hidden(true) {
        if cancel_token.load(Ordering::Relaxed) {
            return Ok(found_files.len() as u64);
        }
        if let Ok(entry) = entry {
            if entry.file_type().is_file() {
                let path = entry.path();
                if is_audio_file(&path) {
                    found_files.push(path);
                }
            }
        }
    }

    let total = found_files.len() as u64;
    let mut analyzed_count = 0u64;

    for path in found_files {
        if cancel_token.load(Ordering::Relaxed) {
            break;
        }

        let path_display = path.to_string_lossy().to_string();
        on_progress(analyzed_count, total, path_display);

        if let Ok(report) = analyze_single_file(&path) {
            on_file_analyzed(report);
        }

        analyzed_count += 1;
    }

    Ok(analyzed_count)
}