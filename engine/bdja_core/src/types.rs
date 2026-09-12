use serde::{Deserialize, Serialize};

/// Versión / revisión del motor de análisis y veredicto.
/// Si cambia, las cachés anteriores se invalidan automáticamente.
pub const ENGINE_REV: u32 = 1;

/// Los 6 estados de veredicto oficiales del motor (§08 de PLAN_ARQUITECTURA.md).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Verdict {
    /// Espectro completo, sin evidencias fuertes, piso de ruido coherente (LLR <= -4.0)
    LosslessVerified,
    /// Limpio, pero con algún indicador menor compatible con el material (-4.0 < LLR <= -1.5)
    LikelyLossless,
    /// Material band-limited de origen, muy corto, silencioso o contradictorio (-1.5 < LLR < +1.5)
    Inconclusive,
    /// Evidencias consistentes pero sin señal concluyente (+1.5 <= LLR < +4.0)
    Suspicious,
    /// Al menos una evidencia fuerte (E04, E05, E07, E13) más corroboración (LLR >= +4.0)
    ProbableTranscode,
    /// El archivo ya declara un códec con pérdida (MP3, AAC, Vorbis, etc.)
    DeclaredLossy,
}

impl Verdict {
    pub fn display_name(&self) -> &'static str {
        match self {
            Verdict::LosslessVerified => "Lossless verificado",
            Verdict::LikelyLossless => "Probablemente lossless",
            Verdict::Inconclusive => "Inconcluso",
            Verdict::Suspicious => "Sospechoso",
            Verdict::ProbableTranscode => "Probable transcode",
            Verdict::DeclaredLossy => "Con pérdida declarado",
        }
    }

    pub fn default_description(&self) -> &'static str {
        match self {
            Verdict::LosslessVerified => "No se encontraron evidencias de una fuente con pérdida.",
            Verdict::LikelyLossless => {
                "Sin evidencias relevantes. Algún indicador menor, compatible con el material."
            }
            Verdict::Inconclusive => {
                "No hay información suficiente para determinarlo con confianza."
            }
            Verdict::Suspicious => {
                "El archivo declara una calidad superior a la que parece contener."
            }
            Verdict::ProbableTranscode => {
                "Evidencia sólida compatible con una fuente MP3/AAC previamente comprimida."
            }
            Verdict::DeclaredLossy => {
                "Formato con pérdida declarado; contenedor y códec legítimos."
            }
        }
    }
}

/// Códigos oficiales del catálogo de evidencias (E01 a E14).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EvidenceCode {
    E01, // Ancho de banda efectivo
    E02, // Pendiente del corte
    E03, // Shelf de 16 kHz
    E04, // Huecos espectrales (Fuerte)
    E05, // Rejilla de bloques (Fuerte)
    E06, // Pre-eco
    E07, // Colapso de joint-stereo (Fuerte)
    E08, // Piso de ruido y dither
    E09, // Bit depth real
    E10, // Upsampling
    E11, // Inflado de contenedor
    E12, // Métricas de calidad
    E13, // Metadata forense (Fuerte)
    E14, // Consistencia temporal
}

impl EvidenceCode {
    pub fn is_strong(&self) -> bool {
        matches!(
            self,
            EvidenceCode::E01
                | EvidenceCode::E04
                | EvidenceCode::E05
                | EvidenceCode::E07
                | EvidenceCode::E13
        )
    }

    pub fn label(&self) -> &'static str {
        match self {
            EvidenceCode::E01 => "Ancho de banda efectivo",
            EvidenceCode::E02 => "Pendiente del corte",
            EvidenceCode::E03 => "Shelf de 16 kHz",
            EvidenceCode::E04 => "Huecos espectrales",
            EvidenceCode::E05 => "Rejilla de bloques",
            EvidenceCode::E06 => "Pre-eco",
            EvidenceCode::E07 => "Colapso de joint-stereo",
            EvidenceCode::E08 => "Piso de ruido y dither",
            EvidenceCode::E09 => "Bit depth real",
            EvidenceCode::E10 => "Upsampling",
            EvidenceCode::E11 => "Inflado de contenedor",
            EvidenceCode::E12 => "Métricas de calidad",
            EvidenceCode::E13 => "Metadata forense",
            EvidenceCode::E14 => "Consistencia temporal",
        }
    }
}

/// Evidencia individual calculada para un archivo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Evidence {
    pub code: EvidenceCode,
    pub value: Option<f64>,
    pub llr: f64,
    pub applicable: bool,
    pub description: String,
}

/// Naturaleza del corte o límite espectral detectado por el motor DSP (§02 v3.0).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CutoffKind {
    /// Corte abrupto artificial por filtro digital brickwall (típico de códecs lossy como MP3/AAC)
    BrickwallCutoff,
    /// Espectro completo real hasta Nyquist sin corte artificial
    FullSpectrum,
    /// Decaimiento acústico natural suave compatible con instrumentación o máster analógico
    NaturalRolloff,
}

/// Códecs soportados por el motor de análisis y reproducción.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Codec {
    // Lossless PCM
    PcmS16Le,
    PcmS24Le,
    PcmS32Le,
    PcmF32Le,
    PcmS16Be,
    PcmS24Be,
    PcmS32Be,
    PcmF32Be,
    PcmU8,
    PcmOther,
    // Lossless comprimido
    Flac,
    Alac,
    WavPack,
    // Lossy
    Mp3,
    Aac,
    Vorbis,
    Opus,
    Wma,
    // Desconocido
    Unknown,
}

impl Codec {
    pub fn is_lossless(&self) -> bool {
        matches!(
            self,
            Codec::PcmS16Le
                | Codec::PcmS24Le
                | Codec::PcmS32Le
                | Codec::PcmF32Le
                | Codec::PcmS16Be
                | Codec::PcmS24Be
                | Codec::PcmS32Be
                | Codec::PcmF32Be
                | Codec::PcmU8
                | Codec::PcmOther
                | Codec::Flac
                | Codec::Alac
                | Codec::WavPack
        )
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Codec::PcmS16Le => "PCM 16-bit LE",
            Codec::PcmS24Le => "PCM 24-bit LE",
            Codec::PcmS32Le => "PCM 32-bit LE",
            Codec::PcmF32Le => "PCM 32-bit Float LE",
            Codec::PcmS16Be => "PCM 16-bit BE",
            Codec::PcmS24Be => "PCM 24-bit BE",
            Codec::PcmS32Be => "PCM 32-bit BE",
            Codec::PcmF32Be => "PCM 32-bit Float BE",
            Codec::PcmU8 => "PCM 8-bit",
            Codec::PcmOther => "PCM",
            Codec::Flac => "FLAC",
            Codec::Alac => "ALAC (Apple Lossless)",
            Codec::WavPack => "WavPack",
            Codec::Mp3 => "MP3 (MPEG Audio Layer III)",
            Codec::Aac => "AAC (Advanced Audio Coding)",
            Codec::Vorbis => "Ogg Vorbis",
            Codec::Opus => "Opus",
            Codec::Wma => "WMA",
            Codec::Unknown => "Códec desconocido",
        }
    }
}

/// Hechos técnicos reales del contenedor y códec (por parseo profundo, nunca extensión).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FormatFacts {
    pub container: String,
    pub codec: String,
    pub codec_type: Codec,
    pub sample_rate: u32,
    pub bit_depth: Option<u16>,
    pub channels: u16,
    pub duration_ms: u64,
    pub container_bitrate_kbps: Option<u32>,
    pub is_lossless_declared: bool,
}

/// Métricas de calidad acústica (informativas, no de procedencia).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QualityMetrics {
    pub true_peak_dbtp: Option<f64>,
    pub lufs_integrated: Option<f64>,
    pub clipped_samples: u64,
    pub dc_offset: Option<f64>,
    pub dynamic_range_db: Option<f64>,
    pub stereo_correlation: Option<f64>,
}

/// Reporte completo de análisis de un archivo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileReport {
    pub file_id: i64,
    pub path: String,
    pub file_size: u64,
    pub engine_rev: u32,
    pub facts: FormatFacts,
    pub verdict: Verdict,
    pub confidence: f64,
    pub score_llr: f64,
    pub effective_bandwidth_hz: Option<u32>,
    pub cutoff_slope_db_oct: Option<f64>,
    pub evidences: Vec<Evidence>,
    pub quality: QualityMetrics,
    pub guards_triggered: Vec<String>,
    pub verdict_summary: String,
    pub average_spectrum_db: Vec<f32>,
}

/// Información de una unidad de almacenamiento del sistema.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VolumeInfo {
    pub id: u8,
    pub path: String,
    pub label: String,
    pub fs_type: String,
    pub is_removable: bool,
    pub is_ready: bool,
    pub total_bytes: u64,
    pub free_bytes: u64,
}

/// Opciones para escaneo masivo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScanOptions {
    pub roots: Vec<String>,
    pub throttle_mode: String, // "normal", "silent", "turbo"
    pub skip_cache: bool,
}

/// Evento de progreso del escaneo reportado hacia Dart a 10 Hz.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ScanEvent {
    JobStarted {
        job_id: i64,
        total_roots: usize,
    },
    FileProgress {
        job_id: i64,
        analyzed_count: u64,
        total_found: u64,
        current_path: String,
    },
    FileDone {
        job_id: i64,
        report: Box<FileReport>,
    },
    VolumeUnavailable {
        job_id: i64,
        root: String,
    },
    JobDone {
        job_id: i64,
        total_analyzed: u64,
        total_errors: u64,
    },
    JobAborted {
        job_id: i64,
        reason: String,
    },
}

/// Información de inicialización del motor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EngineInfo {
    pub revision: u32,
    pub data_dir: String,
    pub status: String,
}
