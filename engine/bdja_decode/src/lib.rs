pub mod decoder;
pub mod error;
pub mod forensic;

pub use decoder::{decode_audio_file, BitDepthStats, DecodedAudio, WINDOW_SIZE_FAST, WINDOW_SIZE_FINE};
pub use error::{DecodeError, Result};
pub use forensic::{analyze_forensic_headers, ForensicEvidence};