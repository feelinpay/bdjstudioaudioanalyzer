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
