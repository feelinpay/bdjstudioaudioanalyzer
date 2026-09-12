use std::fs::File;
use std::path::Path;

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{
    DecoderOptions, CODEC_TYPE_AAC, CODEC_TYPE_ALAC, CODEC_TYPE_FLAC, CODEC_TYPE_MP3,
    CODEC_TYPE_NULL, CODEC_TYPE_OPUS, CODEC_TYPE_PCM_F32BE, CODEC_TYPE_PCM_F32LE,
    CODEC_TYPE_PCM_S16BE, CODEC_TYPE_PCM_S16LE, CODEC_TYPE_PCM_S24BE, CODEC_TYPE_PCM_S24LE,
    CODEC_TYPE_PCM_S32BE, CODEC_TYPE_PCM_S32LE, CODEC_TYPE_PCM_U8, CODEC_TYPE_VORBIS,
    CodecType,
};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::{FormatOptions, SeekMode, SeekTo};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

use bdja_core::types::{Codec, FormatFacts};
use crate::error::{DecodeError, Result};
use crate::forensic::{analyze_forensic_headers, ForensicEvidence};

pub const WINDOW_SIZE_FINE: usize = 8192;
pub const WINDOW_SIZE_FAST: usize = 1024;
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

pub fn map_symphonia_codec(ct: CodecType) -> Codec {
    match ct {
        CODEC_TYPE_PCM_S16LE => Codec::PcmS16Le,
        CODEC_TYPE_PCM_S24LE => Codec::PcmS24Le,
        CODEC_TYPE_PCM_S32LE => Codec::PcmS32Le,
        CODEC_TYPE_PCM_F32LE => Codec::PcmF32Le,
        CODEC_TYPE_PCM_S16BE => Codec::PcmS16Be,
        CODEC_TYPE_PCM_S24BE => Codec::PcmS24Be,
        CODEC_TYPE_PCM_S32BE => Codec::PcmS32Be,
        CODEC_TYPE_PCM_F32BE => Codec::PcmF32Be,
        CODEC_TYPE_PCM_U8 => Codec::PcmU8,
        CODEC_TYPE_FLAC => Codec::Flac,
        CODEC_TYPE_ALAC => Codec::Alac,
        CODEC_TYPE_MP3 => Codec::Mp3,
        CODEC_TYPE_AAC => Codec::Aac,
        CODEC_TYPE_VORBIS => Codec::Vorbis,
        CODEC_TYPE_OPUS => Codec::Opus,
        _ => {
            let debug_name = format!("{:?}", ct);
            if debug_name.contains("Pcm") {
                Codec::PcmOther
            } else if debug_name.contains("Flac") {
                Codec::Flac
            } else if debug_name.contains("Alac") {
                Codec::Alac
            } else if debug_name.contains("Mp3") {
                Codec::Mp3
            } else if debug_name.contains("Aac") {
                Codec::Aac
            } else if debug_name.contains("Vorbis") {
                Codec::Vorbis
            } else if debug_name.contains("Opus") {
                Codec::Opus
            } else {
                Codec::Unknown
            }
        }
    }
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

    // P0-1 FIX: Canonical Codec identification using Symphonia constants
    let codec_type = map_symphonia_codec(codec_params.codec);
    let is_lossless_declared = codec_type.is_lossless();
    let codec_name = codec_type.display_name().to_string();

    let container_bitrate_kbps = (file_size * 8).checked_div(duration_ms).map(|b| b as u32);

    let facts = FormatFacts {
        container: forensic.detected_magic_type.clone(),
        codec: codec_name,
        codec_type,
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

    let mut left_samples = Vec::with_capacity(WINDOW_SIZE_FINE * 24);
    let mut right_samples = Vec::with_capacity(WINDOW_SIZE_FINE * 24);
    let mut windows_8192 = Vec::new();
    let mut windows_1024 = Vec::new();

    let mut max_peak: f32 = 0.0;
    let mut clipped_samples: u64 = 0;
    let mut sum_samples: f64 = 0.0;
    let mut total_sample_count: u64 = 0;

    let mut sample_buf: Option<SampleBuffer<f32>> = None;

    // P0-3 FIX: Stratified sampling across 12 temporal segments (5% to 95%)
    let mut seek_successful = false;

    if n_frames > 0 && duration_ms > 10000 {
        for seg in 0..TARGET_SEGMENTS {
            let frac = 0.05 + 0.90 * (seg as f64 / (TARGET_SEGMENTS - 1).max(1) as f64);
            let target_frame = (frac * n_frames as f64) as u64;

            if format.seek(SeekMode::Coarse, SeekTo::TimeStamp { ts: target_frame, track_id }).is_ok() {
                decoder.reset();
                seek_successful = true;

                // Decode audio samples for this segment (accumulate at least 16384 samples)
                let mut segment_mono = Vec::with_capacity(WINDOW_SIZE_FINE * 2);
                let mut segment_packets = 0;

                while segment_packets < 60 && segment_mono.len() < WINDOW_SIZE_FINE * 2 {
                    let packet = match format.next_packet() {
                        Ok(p) => p,
                        Err(_) => break,
                    };
                    if packet.track_id() != track_id {
                        continue;
                    }
                    segment_packets += 1;

                    if let Ok(audio_buf) = decoder.decode(&packet) {
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
                            let abs_l = l.abs();
                            let abs_r = r.abs();
                            let peak_sample = abs_l.max(abs_r);
                            if peak_sample > max_peak {
                                max_peak = peak_sample;
                            }
                            if abs_l >= 0.9999 || abs_r >= 0.9999 {
                                clipped_samples += 1;
                            }
                            sum_samples += mono as f64;
                            total_sample_count += 1;

                            segment_mono.push(mono);
                            if left_samples.len() < WINDOW_SIZE_FINE * 16 {
                                left_samples.push(l);
                                right_samples.push(r);
                            }
                        }
                    }
                }

                if segment_mono.len() >= WINDOW_SIZE_FINE {
                    let win = segment_mono[..WINDOW_SIZE_FINE].to_vec();
                    let energy: f32 = win.iter().map(|s| s * s).sum();
                    if energy > 0.0001 {
                        windows_8192.push(win);
                    }
                }
                if segment_mono.len() >= WINDOW_SIZE_FAST * 2 {
                    windows_1024.push(segment_mono[..WINDOW_SIZE_FAST].to_vec());
                    windows_1024.push(segment_mono[WINDOW_SIZE_FAST..WINDOW_SIZE_FAST * 2].to_vec());
                }
            }
        }
    }

    // Fallback: If seek was not supported or yielded fewer than 4 windows, decode sequentially
    if !seek_successful || windows_8192.len() < 4 {
        decoder.reset();
        let _ = format.seek(SeekMode::Coarse, SeekTo::TimeStamp { ts: 0, track_id });

        let mut all_mono = Vec::with_capacity(WINDOW_SIZE_FINE * 32);
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

        // Slice windows from all_mono if windows_8192 is empty
        if windows_8192.is_empty() {
            if all_mono.len() >= WINDOW_SIZE_FINE {
                let step = (all_mono.len() - WINDOW_SIZE_FINE) / TARGET_SEGMENTS.max(1);
                for i in 0..TARGET_SEGMENTS {
                    let start = i * step;
                    if start + WINDOW_SIZE_FINE <= all_mono.len() {
                        let w8 = all_mono[start..start + WINDOW_SIZE_FINE].to_vec();
                        let energy: f32 = w8.iter().map(|s| s * s).sum();
                        if energy > 0.0001 {
                            windows_8192.push(w8);
                        }
                    }
                }
            } else if !all_mono.is_empty() {
                let mut padded = all_mono.clone();
                padded.resize(WINDOW_SIZE_FINE, 0.0);
                windows_8192.push(padded);
            }
        }

        if windows_1024.is_empty() && all_mono.len() >= WINDOW_SIZE_FAST {
            let step_fast = (all_mono.len() - WINDOW_SIZE_FAST) / 32;
            for i in 0..32 {
                let start = i * step_fast;
                if start + WINDOW_SIZE_FAST <= all_mono.len() {
                    windows_1024.push(all_mono[start..start + WINDOW_SIZE_FAST].to_vec());
                }
            }
        }
    }

    let dc_offset = if total_sample_count > 0 {
        sum_samples / total_sample_count as f64
    } else {
        0.0
    };

    // Calculate real LSB zero ratio and estimated bit depth
    let declared_b = bit_depth.unwrap_or(16);
    let mut zero_lsb_count = 0u64;
    let mut evaluated_samples = 0u64;
    for &s in &left_samples {
        if s.abs() > 1e-5 {
            if declared_b >= 24 {
                let int_val = (s * 8388607.0).round().abs() as i64;
                if (int_val & 0xFF) == 0 {
                    zero_lsb_count += 1;
                }
            } else if declared_b == 16 {
                let int_val = (s * 32767.0).round().abs() as i64;
                if (int_val & 0xFF) == 0 {
                    zero_lsb_count += 1;
                }
            }
            evaluated_samples += 1;
        }
    }
    let zero_lsb_ratio = if evaluated_samples > 100 {
        zero_lsb_count as f64 / evaluated_samples as f64
    } else {
        0.0
    };
    let estimated_real_bits = if declared_b >= 24 && zero_lsb_ratio > 0.85 {
        16
    } else if declared_b == 16 && zero_lsb_ratio > 0.85 {
        8
    } else {
        declared_b
    };

    let bit_depth_stats = BitDepthStats {
        declared_bits: bit_depth,
        estimated_real_bits,
        zero_lsb_ratio,
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