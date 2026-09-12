use std::fs::File;
use std::path::Path;

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

use bdja_core::types::FormatFacts;
use crate::error::{DecodeError, Result};
use crate::forensic::{analyze_forensic_headers, ForensicEvidence};

pub const WINDOW_SIZE_FINE: usize = 8192;
pub const WINDOW_SIZE_FAST: usize = 1024;
pub const TARGET_WINDOWS: usize = 192;
pub const TARGET_SEGMENTS: usize = 12;

#[derive(Debug, Clone, Default)]
pub struct BitDepthStats {
    pub declared_bits: Option<u16>,
    pub estimated_real_bits: u16,
    pub zero_lsb_ratio: f64,
}

#[derive(Debug, Clone)]
pub struct DecodedAudio {
    pub facts: FormatFacts,
    pub forensic: ForensicEvidence,
    pub sample_rate: u32,
    pub channels: u16,
    pub windows_8192: Vec<Vec<f32>>,
    pub windows_1024: Vec<Vec<f32>>,
    pub left_channel_samples: Vec<f32>,
    pub right_channel_samples: Vec<f32>,
    pub bit_depth_stats: BitDepthStats,
    pub max_peak: f32,
    pub clipped_samples: u64,
    pub dc_offset: f64,
}

pub fn decode_audio_file(path: &Path) -> Result<DecodedAudio> {
    let metadata = std::fs::metadata(path)?;
    let file_size = metadata.len();
    if file_size == 0 {
        return Err(DecodeError::ZeroLength);
    }
    if file_size > 2 * 1024 * 1024 * 1024 {
        return Err(DecodeError::FileTooLarge(file_size));
    }

    let forensic = analyze_forensic_headers(path);

    let file = File::open(path)?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let meta_opts = MetadataOptions::default();
    let fmt_opts = FormatOptions::default();

    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &fmt_opts, &meta_opts)
        .map_err(|e| DecodeError::UnrecognizedFormat(e.to_string()))?;

    let mut format = probed.format;

    // Find first audio track
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or(DecodeError::NoAudioTrack)?;

    let track_id = track.id;
    let codec_params = track.codec_params.clone();
    let sample_rate = codec_params.sample_rate.unwrap_or(44100);
    let channels = codec_params.channels.map(|c| c.count() as u16).unwrap_or(2);
    let bit_depth = codec_params.bits_per_sample.map(|b| b as u16);

    let n_frames = codec_params.n_frames.unwrap_or(0);
    let duration_ms = if sample_rate > 0 && n_frames > 0 {
        (n_frames as f64 / sample_rate as f64 * 1000.0) as u64
    } else {
        0
    };

    if duration_ms > 3 * 3600 * 1000 {
        return Err(DecodeError::DurationExceeded(duration_ms));
    }

    // Format & Codec identification
    let codec_name = format!("{:?}", codec_params.codec);
    let is_lossless_declared = match codec_name.as_str() {
        s if s.contains("Pcm") || s.contains("Flac") || s.contains("Alac") => true,
        _ => false,
    };

    let container_bitrate_kbps = if duration_ms > 0 {
        Some(((file_size * 8) / duration_ms) as u32)
    } else {
        None
    };

    let facts = FormatFacts {
        container: forensic.detected_magic_type.clone(),
        codec: codec_name,
        sample_rate,
        bit_depth,
        channels,
        duration_ms,
        container_bitrate_kbps,
        is_lossless_declared,
    };

    let dec_opts = DecoderOptions::default();
    let mut decoder = symphonia::default::get_codecs()
        .make(&codec_params, &dec_opts)
        .map_err(|e| DecodeError::DecoderInit(e.to_string()))?;

    // Collect decoded samples across track
    let mut left_samples = Vec::with_capacity(WINDOW_SIZE_FINE * 24);
    let mut right_samples = Vec::with_capacity(WINDOW_SIZE_FINE * 24);
    let mut all_mono = Vec::with_capacity(WINDOW_SIZE_FINE * 32);

    let mut max_peak: f32 = 0.0;
    let mut clipped_samples: u64 = 0;
    let mut sum_samples: f64 = 0.0;
    let mut total_sample_count: u64 = 0;

    let mut sample_buf: Option<SampleBuffer<f32>> = None;

    // Decode packets up to budget limit
    let max_packets_budget = 4000;
    let mut packets_read = 0;

    while packets_read < max_packets_budget {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(SymphoniaError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(SymphoniaError::ResetRequired) => continue,
            Err(_) => break,
        };

        if packet.track_id() != track_id {
            continue;
        }

        packets_read += 1;

        match decoder.decode(&packet) {
            Ok(audio_buf) => {
                let spec = *audio_buf.spec();
                let capacity = audio_buf.capacity();

                let sbuf = sample_buf.get_or_insert_with(|| SampleBuffer::new(capacity as u64, spec));
                sbuf.copy_interleaved_ref(audio_buf);

                let samples = sbuf.samples();
                let n_ch = spec.channels.count();

                for frame in samples.chunks(n_ch) {
                    let (l, r) = if n_ch >= 2 {
                        (frame[0], frame[1])
                    } else if n_ch == 1 {
                        (frame[0], frame[0])
                    } else {
                        (0.0, 0.0)
                    };

                    let mono = (l + r) * 0.5;

                    let abs_mono = mono.abs();
                    if abs_mono > max_peak {
                        max_peak = abs_mono;
                    }
                    if abs_mono >= 0.9999 {
                        clipped_samples += 1;
                    }
                    sum_samples += mono as f64;
                    total_sample_count += 1;

                    // Collect stratified pool
                    if all_mono.len() < WINDOW_SIZE_FINE * 32 {
                        all_mono.push(mono);
                        if left_samples.len() < WINDOW_SIZE_FINE * 16 {
                            left_samples.push(l);
                            right_samples.push(r);
                        }
                    }
                }
            }
            Err(SymphoniaError::DecodeError(_)) => continue,
            Err(_) => break,
        }
    }

    let dc_offset = if total_sample_count > 0 {
        sum_samples / total_sample_count as f64
    } else {
        0.0
    };

    // Slice stratified windows of 8192 and 1024
    let mut windows_8192 = Vec::new();
    let mut windows_1024 = Vec::new();

    if all_mono.len() >= WINDOW_SIZE_FINE {
        let step = (all_mono.len() - WINDOW_SIZE_FINE) / (TARGET_SEGMENTS.max(1));
        for i in 0..TARGET_SEGMENTS {
            let start = i * step;
            if start + WINDOW_SIZE_FINE <= all_mono.len() {
                let w8 = all_mono[start..start + WINDOW_SIZE_FINE].to_vec();
                // Check that window is not completely silent
                let energy: f32 = w8.iter().map(|s| s * s).sum();
                if energy > 0.0001 {
                    windows_8192.push(w8);
                }
            }
        }
    } else if !all_mono.is_empty() {
        // Zero pad if shorter than 8192
        let mut padded = all_mono.clone();
        padded.resize(WINDOW_SIZE_FINE, 0.0);
        windows_8192.push(padded);
    }

    if all_mono.len() >= WINDOW_SIZE_FAST {
        let step_fast = (all_mono.len() - WINDOW_SIZE_FAST) / 32;
        for i in 0..32 {
            let start = i * step_fast;
            if start + WINDOW_SIZE_FAST <= all_mono.len() {
                windows_1024.push(all_mono[start..start + WINDOW_SIZE_FAST].to_vec());
            }
        }
    }

    let bit_depth_stats = BitDepthStats {
        declared_bits: bit_depth,
        estimated_real_bits: bit_depth.unwrap_or(16),
        zero_lsb_ratio: 0.0,
    };

    Ok(DecodedAudio {
        facts,
        forensic,
        sample_rate,
        channels,
        windows_8192,
        windows_1024,
        left_channel_samples: left_samples,
        right_channel_samples: right_samples,
        bit_depth_stats,
        max_peak,
        clipped_samples,
        dc_offset,
    })
}