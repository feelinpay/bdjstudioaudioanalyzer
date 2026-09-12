use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ForensicEvidence {
    pub has_xing_lame_header: bool,
    pub encoder_string: Option<String>,
    pub has_anomalous_id3_in_wav: bool,
    pub is_extension_mismatch: bool,
    pub declared_extension: String,
    pub detected_magic_type: String,
    pub descriptions: Vec<String>,
}

pub fn analyze_forensic_headers(path: &Path) -> ForensicEvidence {
    let mut evidence = ForensicEvidence::default();
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    evidence.declared_extension = ext.clone();

    let mut file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return evidence,
    };

    let file_len = file.metadata().map(|m| m.len()).unwrap_or(0);
    if file_len < 32 {
        return evidence;
    }

    // Read first 64KB header
    let header_size = std::cmp::min(file_len as usize, 65536);
    let mut header_buf = vec![0u8; header_size];
    if file.read_exact(&mut header_buf).is_err() {
        return evidence;
    }

    // Check magic bytes
    let is_riff_wav = header_buf.starts_with(b"RIFF") && header_buf.len() >= 12 && &header_buf[8..12] == b"WAVE";
    let is_flac = header_buf.starts_with(b"fLaC");
    let is_aiff = header_buf.starts_with(b"FORM") && header_buf.len() >= 12 && (&header_buf[8..12] == b"AIFF" || &header_buf[8..12] == b"AIFC");
    let is_mp3_id3 = header_buf.starts_with(b"ID3");
    let is_mp3_sync = header_buf.len() >= 2 && header_buf[0] == 0xFF && (header_buf[1] & 0xE0) == 0xE0;
    let is_mp4 = header_buf.len() >= 8 && (&header_buf[4..8] == b"ftyp" || &header_buf[4..8] == b"moov");
    let is_ogg = header_buf.starts_with(b"OggS");

    evidence.detected_magic_type = if is_riff_wav {
        "WAV (RIFF)".to_string()
    } else if is_flac {
        "FLAC".to_string()
    } else if is_aiff {
        "AIFF".to_string()
    } else if is_mp3_id3 || is_mp3_sync {
        "MP3".to_string()
    } else if is_mp4 {
        "MP4 / AAC".to_string()
    } else if is_ogg {
        "OGG".to_string()
    } else {
        "UNKNOWN".to_string()
    };

    // Check extension mismatch
    if (ext == "wav" && !is_riff_wav) || (ext == "flac" && !is_flac) || (ext == "aif" && !is_aiff) || (ext == "aiff" && !is_aiff) {
        if is_mp3_id3 || is_mp3_sync || is_mp4 || is_ogg {
            evidence.is_extension_mismatch = true;
            evidence.descriptions.push(format!(
                "Extension declarada .{} pero la cabecera real es {}",
                ext, evidence.detected_magic_type
            ));
        }
    }

    // Check anomalous ID3 inside WAV
    if is_riff_wav {
        if find_subsequence(&header_buf, b"id3 ") || find_subsequence(&header_buf, b"ID3 ") {
            evidence.has_anomalous_id3_in_wav = true;
            evidence.descriptions.push("Chunk ID3 anomalo encontrado dentro del contenedor RIFF/WAV".to_string());
        }
    }

    // Scan for Xing / Info / LAME headers in header_buf
    if let Some(pos) = find_subsequence_pos(&header_buf, b"Xing").or_else(|| find_subsequence_pos(&header_buf, b"Info")) {
        evidence.has_xing_lame_header = true;
        evidence.descriptions.push("Cabecera Xing/Info residual detectada".to_string());
        // Look for LAME version tag usually 120 bytes after Xing
        if header_buf.len() > pos + 128 {
            let slice = &header_buf[pos..pos + 160];
            if let Some(lame_pos) = find_subsequence_pos(slice, b"LAME") {
                let end = std::cmp::min(lame_pos + 9, slice.len());
                if let Ok(s) = std::str::from_utf8(&slice[lame_pos..end]) {
                    evidence.encoder_string = Some(s.to_string());
                    evidence.descriptions.push(format!("Firma de encoder detectada: {}", s));
                }
            }
        }
    } else if let Some(pos) = find_subsequence_pos(&header_buf, b"LAME3.") {
        evidence.has_xing_lame_header = true;
        let end = std::cmp::min(pos + 9, header_buf.len());
        if let Ok(s) = std::str::from_utf8(&header_buf[pos..end]) {
            evidence.encoder_string = Some(s.to_string());
            evidence.descriptions.push(format!("Firma LAME residual encontrada: {}", s));
        }
    } else if let Some(pos) = find_subsequence_pos(&header_buf, b"Lavf") {
        let end = std::cmp::min(pos + 10, header_buf.len());
        if let Ok(s) = std::str::from_utf8(&header_buf[pos..end]) {
            evidence.encoder_string = Some(s.to_string());
            evidence.descriptions.push(format!("Firma Lavf (FFmpeg) encontrada: {}", s));
        }
    }

    // Also read last 4KB for trailing tags (e.g. ID3v1 or APE tag or LAME tag at EOF)
    if file_len > 4096 {
        if file.seek(SeekFrom::End(-4096)).is_ok() {
            let mut tail_buf = [0u8; 4096];
            if file.read_exact(&mut tail_buf).is_ok() {
                if let Some(pos) = find_subsequence_pos(&tail_buf, b"LAME3.") {
                    evidence.has_xing_lame_header = true;
                    let end = std::cmp::min(pos + 9, tail_buf.len());
                    if let Ok(s) = std::str::from_utf8(&tail_buf[pos..end]) {
                        if evidence.encoder_string.is_none() {
                            evidence.encoder_string = Some(s.to_string());
                            evidence.descriptions.push(format!("Firma LAME residual al final del archivo: {}", s));
                        }
                    }
                }
            }
        }
    }

    evidence
}

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> bool {
    find_subsequence_pos(haystack, needle).is_some()
}

fn find_subsequence_pos(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|window| window == needle)
}