use bdja_core::types::Codec;
use bdja_decode::decoder::map_symphonia_codec;
use bdja_decode::forensic::analyze_forensic_headers;
use std::io::Write;
use symphonia::core::codecs::*;

#[test]
fn test_symphonia_codec_mapping() {
    // Lossless codecs
    let pcm16 = map_symphonia_codec(CODEC_TYPE_PCM_S16LE);
    assert_eq!(pcm16, Codec::PcmS16Le);
    assert!(pcm16.is_lossless());

    let pcm24 = map_symphonia_codec(CODEC_TYPE_PCM_S24LE);
    assert_eq!(pcm24, Codec::PcmS24Le);
    assert!(pcm24.is_lossless());

    let flac = map_symphonia_codec(CODEC_TYPE_FLAC);
    assert_eq!(flac, Codec::Flac);
    assert!(flac.is_lossless());

    let alac = map_symphonia_codec(CODEC_TYPE_ALAC);
    assert_eq!(alac, Codec::Alac);
    assert!(alac.is_lossless());

    // Lossy codecs
    let mp3 = map_symphonia_codec(CODEC_TYPE_MP3);
    assert_eq!(mp3, Codec::Mp3);
    assert!(!mp3.is_lossless());

    let aac = map_symphonia_codec(CODEC_TYPE_AAC);
    assert_eq!(aac, Codec::Aac);
    assert!(!aac.is_lossless());

    let vorbis = map_symphonia_codec(CODEC_TYPE_VORBIS);
    assert_eq!(vorbis, Codec::Vorbis);
    assert!(!vorbis.is_lossless());

    let opus = map_symphonia_codec(CODEC_TYPE_OPUS);
    assert_eq!(opus, Codec::Opus);
    assert!(!opus.is_lossless());
}

#[test]
fn test_forensic_lavf_ffmpeg_is_not_lossy_signature() {
    let tmp_dir = tempfile::tempdir().unwrap();
    let wav_path = tmp_dir.path().join("test_ffmpeg.wav");

    // Create a mock WAV header with Lavf tag (FFmpeg export)
    let mut f = std::fs::File::create(&wav_path).unwrap();
    let mut header = vec![0u8; 1024];
    // RIFF .... WAVE
    header[0..4].copy_from_slice(b"RIFF");
    header[8..12].copy_from_slice(b"WAVE");
    // Embed "Lavf58.76.100" in header
    let lavf_str = b"Lavf58.76.100";
    header[100..100 + lavf_str.len()].copy_from_slice(lavf_str);
    f.write_all(&header).unwrap();
    drop(f);

    let evidence = analyze_forensic_headers(&wav_path);
    assert_eq!(evidence.detected_magic_type, "WAV (RIFF)");
    assert_eq!(evidence.encoder_string.as_deref(), Some("Lavf58.76.100"));
    // MUST NOT be marked as lossy signature!
    assert!(!evidence.has_lossy_encoder_signature);
    assert!(!evidence.has_xing_lame_header);
}

#[test]
fn test_forensic_id3_in_wav_dj_metadata_not_transcode() {
    let tmp_dir = tempfile::tempdir().unwrap();
    let wav_path = tmp_dir.path().join("test_serato.wav");

    // Create a mock WAV header with id3 chunk (Serato / Rekordbox)
    let mut f = std::fs::File::create(&wav_path).unwrap();
    let mut header = vec![0u8; 1024];
    header[0..4].copy_from_slice(b"RIFF");
    header[8..12].copy_from_slice(b"WAVE");
    // Embed standard DJ id3 chunk
    header[64..68].copy_from_slice(b"id3 ");
    f.write_all(&header).unwrap();
    drop(f);

    let evidence = analyze_forensic_headers(&wav_path);
    assert_eq!(evidence.detected_magic_type, "WAV (RIFF)");
    assert!(evidence.has_dj_metadata);
    // MUST NOT be marked as lossy signature!
    assert!(!evidence.has_lossy_encoder_signature);
    assert!(!evidence.has_xing_lame_header);
}

#[test]
fn test_forensic_detects_real_lame_transcode() {
    let tmp_dir = tempfile::tempdir().unwrap();
    let wav_path = tmp_dir.path().join("test_transcode.wav");

    // Create a mock WAV containing residual Xing header and LAME tag
    let mut f = std::fs::File::create(&wav_path).unwrap();
    let mut header = vec![0u8; 2048];
    header[0..4].copy_from_slice(b"RIFF");
    header[8..12].copy_from_slice(b"WAVE");
    // Residual Xing frame header
    header[200..204].copy_from_slice(b"Xing");
    header[320..329].copy_from_slice(b"LAME3.100");
    f.write_all(&header).unwrap();
    drop(f);

    let evidence = analyze_forensic_headers(&wav_path);
    assert!(evidence.has_lossy_encoder_signature);
    assert!(evidence.has_xing_lame_header);
    assert_eq!(evidence.encoder_string.as_deref(), Some("LAME3.100"));
}

#[test]
fn test_unsupported_format_handling() {
    use bdja_decode::decoder::decode_audio_file;
    use bdja_decode::error::DecodeError;

    let tmp_dir = tempfile::tempdir().unwrap();
    let wv_path = tmp_dir.path().join("test.wv");

    // Write dummy header resembling a non-symphonia format (WavPack magic 'wvpk')
    let mut f = std::fs::File::create(&wv_path).unwrap();
    let mut data = vec![0u8; 512];
    data[0..4].copy_from_slice(b"wvpk");
    f.write_all(&data).unwrap();
    drop(f);

    let res = decode_audio_file(&wv_path);
    assert!(res.is_err());
    match res.err().unwrap() {
        DecodeError::Unsupported(_) | DecodeError::UnrecognizedFormat(_) => {
            // Correctly classified as unsupported/unrecognized format, NOT Corrupted or Io
        }
        other => panic!(
            "Expected Unsupported or UnrecognizedFormat, got {:?}",
            other
        ),
    }
}

#[test]
fn test_corrupted_wav_is_not_reported_as_unsupported() {
    use bdja_decode::decoder::decode_audio_file;
    use bdja_decode::error::DecodeError;

    let tmp_dir = tempfile::tempdir().unwrap();
    let broken_wav_path = tmp_dir.path().join("broken.wav");

    // Crear archivo .wav con datos truncados y corruptos
    let mut f = std::fs::File::create(&broken_wav_path).unwrap();
    let mut data = vec![0u8; 64];
    data[0..4].copy_from_slice(b"RIFF");
    data[4..8].copy_from_slice(&999999u32.to_le_bytes());
    data[8..12].copy_from_slice(b"WAVE");
    f.write_all(&data).unwrap();
    drop(f);

    let res = decode_audio_file(&broken_wav_path);
    assert!(res.is_err());
    let err = res.err().unwrap();
    match err {
        DecodeError::Unsupported(_) => {
            panic!("B-15 Regresión: Un archivo .wav dañado NUNCA debe reportarse como DecodeError::Unsupported");
        }
        DecodeError::CorruptedHeader(_) | DecodeError::Io(_) => {
            // Correcto: clasificado como cabecera corrupta o fallo de lectura por truncamiento
        }
        other => {
            panic!("Esperaba CorruptedHeader o Io, obtenido: {:?}", other);
        }
    }
}

#[test]
fn test_wav_with_mp3_tag_0x0055_is_unsupported_codec_not_corrupted() {
    use bdja_decode::decoder::decode_audio_file;
    use bdja_decode::error::DecodeError;

    let tmp_dir = tempfile::tempdir().unwrap();
    let wav_mp3_path = tmp_dir.path().join("wav_mp3_0x0055.wav");

    let mut f = std::fs::File::create(&wav_mp3_path).unwrap();
    // Build valid RIFF WAVE header with fmt chunk declaring 0x0055 (MPEGLAYER3)
    let mut data = Vec::new();
    data.extend_from_slice(b"RIFF");
    data.extend_from_slice(&(44u32).to_le_bytes()); // total len - 8
    data.extend_from_slice(b"WAVE");
    data.extend_from_slice(b"fmt ");
    data.extend_from_slice(&(16u32).to_le_bytes()); // fmt chunk size
    data.extend_from_slice(&(0x0055u16).to_le_bytes()); // WAVE_FORMAT_MPEGLAYER3
    data.extend_from_slice(&(2u16).to_le_bytes()); // 2 channels
    data.extend_from_slice(&(44100u32).to_le_bytes()); // 44100 Hz
    data.extend_from_slice(&(16000u32).to_le_bytes()); // avg bytes/sec
    data.extend_from_slice(&(1u16).to_le_bytes()); // block align
    data.extend_from_slice(&(0u16).to_le_bytes()); // bits per sample
    data.extend_from_slice(b"data");
    data.extend_from_slice(&(0u32).to_le_bytes()); // data size 0
    f.write_all(&data).unwrap();
    drop(f);

    let res = decode_audio_file(&wav_mp3_path);
    assert!(res.is_err());
    match res.err().unwrap() {
        DecodeError::Unsupported(msg) => {
            // B-16: Symphonia reconoce el contenedor WAV, pero rechaza la etiqueta 0x0055
            assert!(
                msg.contains("wav") || msg.contains("wave format"),
                "El mensaje debe indicar códec/formato interno no soportado, obtenido: {}",
                msg
            );
        }
        DecodeError::CorruptedHeader(_) => {
            panic!("B-16 Regresión: Un WAV válido con formato 0x0055 NO debe reportarse como CorruptedHeader");
        }
        other => {
            panic!("Esperaba DecodeError::Unsupported, obtenido: {:?}", other);
        }
    }
}
