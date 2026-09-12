use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use jwalk::WalkDir;
use bdja_core::types::FileReport;
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

    // 1. Directory Walk
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

    // 2. Analyze found audio files
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