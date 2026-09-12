use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ForensicEvidence {
    pub has_xing_lame_header: bool,
    pub has_lossy_encoder_signature: bool,
    pub encoder_string: Option<String>,
    pub is_extension_mismatch: bool,
    pub has_dj_metadata: bool,
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
    let is_riff_wav =
        header_buf.starts_with(b"RIFF") && header_buf.len() >= 12 && &header_buf[8..12] == b"WAVE";
    let is_flac = header_buf.starts_with(b"fLaC");
    let is_aiff = header_buf.starts_with(b"FORM")
        && header_buf.len() >= 12
        && (&header_buf[8..12] == b"AIFF" || &header_buf[8..12] == b"AIFC");
    let is_mp3_id3 = header_buf.starts_with(b"ID3");
    let is_mp3_sync =
        header_buf.len() >= 2 && header_buf[0] == 0xFF && (header_buf[1] & 0xE0) == 0xE0;
    let is_mp4 =
        header_buf.len() >= 8 && (&header_buf[4..8] == b"ftyp" || &header_buf[4..8] == b"moov");
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

    // Extension mismatch check: Declared lossless container, but actual binary is lossy bitstream
    if ((ext == "wav" && !is_riff_wav)
        || (ext == "flac" && !is_flac)
        || (ext == "aif" && !is_aiff)
        || (ext == "aiff" && !is_aiff))
        && (is_mp3_id3 || is_mp3_sync || is_mp4 || is_ogg)
    {
        evidence.is_extension_mismatch = true;
        evidence.descriptions.push(format!(
            "Discrepancia crítica de extensión: declarada .{} pero el contenedor binario real es {}",
            ext, evidence.detected_magic_type
        ));
    }

    // DJ metadata check: Standard ID3 chunks in WAV (Rekordbox / Serato / Traktor tags)
    // This is legitimate DJ software metadata, NOT transcode evidence!
    if is_riff_wav
        && (find_subsequence(&header_buf, b"id3 ") || find_subsequence(&header_buf, b"ID3 "))
    {
        evidence.has_dj_metadata = true;
        evidence.descriptions.push(
            "Chunk ID3 de metadatos detectado (legítimo de software DJ: Rekordbox/Serato)"
                .to_string(),
        );
    }

    // Scan for Xing / Info / LAME headers in header_buf
    if let Some(pos) = find_subsequence_pos(&header_buf, b"Xing")
        .or_else(|| find_subsequence_pos(&header_buf, b"Info"))
    {
        // Confirm it is not inside an ID3 text tag but in an audio frame header
        evidence.has_xing_lame_header = true;
        evidence.has_lossy_encoder_signature = true;
        evidence
            .descriptions
            .push("Cabecera Xing/Info residual de trama MP3 detectada".to_string());
        // Look for LAME version tag usually 120-160 bytes after Xing
        if header_buf.len() > pos + 128 {
            let slice = &header_buf[pos..pos + 160];
            if let Some(lame_pos) = find_subsequence_pos(slice, b"LAME") {
                let end = std::cmp::min(lame_pos + 9, slice.len());
                if let Ok(s) = std::str::from_utf8(&slice[lame_pos..end]) {
                    evidence.encoder_string = Some(s.to_string());
                    evidence
                        .descriptions
                        .push(format!("Firma de encoder MP3 residual: {}", s));
                }
            }
        }
    } else if let Some(pos) = find_subsequence_pos(&header_buf, b"LAME3.") {
        evidence.has_xing_lame_header = true;
        evidence.has_lossy_encoder_signature = true;
        let end = std::cmp::min(pos + 9, header_buf.len());
        if let Ok(s) = std::str::from_utf8(&header_buf[pos..end]) {
            evidence.encoder_string = Some(s.to_string());
            evidence
                .descriptions
                .push(format!("Firma LAME3 residual encontrada: {}", s));
        }
    } else if let Some(pos) = find_subsequence_pos(&header_buf, b"Lavf") {
        // Lavf is FFmpeg (libavformat). It is standard for DAW exports, conversion scripts and mastering.
        // It is NOT a lossy signature by itself!
        let max_len = std::cmp::min(pos + 32, header_buf.len());
        let slice = &header_buf[pos..max_len];
        let end_idx = slice
            .iter()
            .position(|&b| b == 0 || !(0x20..=0x7E).contains(&b))
            .unwrap_or(slice.len());
        if let Ok(s) = std::str::from_utf8(&slice[..end_idx]) {
            let s_clean = s.trim().to_string();
            evidence.encoder_string = Some(s_clean.clone());
            // Informational only, never accusatory
            evidence
                .descriptions
                .push(format!("Software de exportación: {}", s_clean));
        }
    }

    // Also read last 4KB for trailing tags (e.g. LAME tag at EOF in pseudo-WAV)
    if file_len > 4096 && file.seek(SeekFrom::End(-4096)).is_ok() {
        let mut tail_buf = [0u8; 4096];
        if file.read_exact(&mut tail_buf).is_ok() {
            if let Some(pos) = find_subsequence_pos(&tail_buf, b"LAME3.") {
                evidence.has_xing_lame_header = true;
                evidence.has_lossy_encoder_signature = true;
                let end = std::cmp::min(pos + 9, tail_buf.len());
                if let Ok(s) = std::str::from_utf8(&tail_buf[pos..end]) {
                    if evidence.encoder_string.is_none() {
                        evidence.encoder_string = Some(s.to_string());
                        evidence
                            .descriptions
                            .push(format!("Firma LAME residual al final del archivo: {}", s));
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
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}
