use std::path::{Path, PathBuf};
use parking_lot::RwLock;
use bdja_core::types::FileReport;
use bdja_store::ReportStore;

pub const ENGINE_REV: u32 = 1;

static INITIALIZED: RwLock<bool> = RwLock::new(false);
static DATA_DIR: RwLock<Option<String>> = RwLock::new(None);
static STORE: RwLock<Option<ReportStore>> = RwLock::new(None);

#[derive(Debug, Clone, PartialEq)]
pub struct EngineInfoFfi {
    pub revision: u32,
    pub data_dir: String,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VolumeInfoFfi {
    pub id: u8,
    pub path: String,
    pub label: String,
    pub fs_type: String,
    pub is_removable: bool,
    pub is_ready: bool,
    pub total_bytes: u64,
    pub free_bytes: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FormatFactsFfi {
    pub container: String,
    pub codec: String,
    pub sample_rate: u32,
    pub bit_depth: Option<u16>,
    pub channels: u16,
    pub duration_ms: u64,
    pub container_bitrate_kbps: Option<u32>,
    pub is_lossless_declared: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct QualityMetricsFfi {
    pub true_peak_dbtp: Option<f64>,
    pub lufs_integrated: Option<f64>,
    pub clipped_samples: u64,
    pub dc_offset: Option<f64>,
    pub dynamic_range_db: Option<f64>,
    pub stereo_correlation: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EvidenceFfi {
    pub code: String, // "E01" .. "E14"
    pub value: Option<f64>,
    pub llr: f64,
    pub applicable: bool,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FileReportFfi {
    pub file_id: i64,
    pub path: String,
    pub file_size: u64,
    pub engine_rev: u32,
    pub facts: FormatFactsFfi,
    pub verdict_code: String,
    pub verdict_name: String,
    pub confidence: f64,
    pub score_llr: f64,
    pub effective_bandwidth_hz: Option<u32>,
    pub cutoff_slope_db_oct: Option<f64>,
    pub evidences: Vec<EvidenceFfi>,
    pub quality: QualityMetricsFfi,
    pub guards_triggered: Vec<String>,
    pub verdict_summary: String,
    pub spectrum_db: Vec<f32>,
}

fn map_report_to_ffi(report: FileReport, spectrum_db: Vec<f32>) -> FileReportFfi {
    let verdict_code = format!("{:?}", report.verdict);
    let verdict_name = report.verdict.display_name().to_string();

    FileReportFfi {
        file_id: report.file_id,
        path: report.path,
        file_size: report.file_size,
        engine_rev: report.engine_rev,
        facts: FormatFactsFfi {
            container: report.facts.container,
            codec: report.facts.codec,
            sample_rate: report.facts.sample_rate,
            bit_depth: report.facts.bit_depth,
            channels: report.facts.channels,
            duration_ms: report.facts.duration_ms,
            container_bitrate_kbps: report.facts.container_bitrate_kbps,
            is_lossless_declared: report.facts.is_lossless_declared,
        },
        verdict_code,
        verdict_name,
        confidence: report.confidence,
        score_llr: report.score_llr,
        effective_bandwidth_hz: report.effective_bandwidth_hz,
        cutoff_slope_db_oct: report.cutoff_slope_db_oct,
        evidences: report
            .evidences
            .into_iter()
            .map(|e| EvidenceFfi {
                code: format!("{:?}", e.code),
                value: e.value,
                llr: e.llr,
                applicable: e.applicable,
                description: e.description,
            })
            .collect(),
        quality: QualityMetricsFfi {
            true_peak_dbtp: report.quality.true_peak_dbtp,
            lufs_integrated: report.quality.lufs_integrated,
            clipped_samples: report.quality.clipped_samples,
            dc_offset: report.quality.dc_offset,
            dynamic_range_db: report.quality.dynamic_range_db,
            stereo_correlation: report.quality.stereo_correlation,
        },
        guards_triggered: report.guards_triggered,
        verdict_summary: report.verdict_summary,
        spectrum_db,
    }
}

/// Devuelve la revision del motor nativo.
#[flutter_rust_bridge::frb(sync)]
pub fn engine_revision() -> u32 {
    ENGINE_REV
}

/// Inicializa el motor con token de capacidad y directorio de trabajo.
pub fn engine_init(capability_token: String, data_dir: String) -> Result<EngineInfoFfi, String> {
    if capability_token.trim().is_empty() {
        return Err("Token de capacidad requerido para inicializar el motor".to_string());
    }

    let mut init = INITIALIZED.write();
    let mut dir = DATA_DIR.write();
    let mut store_lock = STORE.write();

    let db_path = Path::new(&data_dir).join("bdj_audio_analyzer.db");
    match ReportStore::open(&db_path) {
        Ok(store) => *store_lock = Some(store),
        Err(_) => {
            if let Ok(mem_store) = ReportStore::open_in_memory() {
                *store_lock = Some(mem_store);
            }
        }
    }

    *init = true;
    *dir = Some(data_dir.clone());

    Ok(EngineInfoFfi {
        revision: ENGINE_REV,
        data_dir,
        status: "Motor BDJ Audio Analyzer Inicializado y Autenticado".to_string(),
    })
}

/// Enumera las unidades del sistema (USB, SSD, HDD).
pub fn list_system_volumes() -> Result<Vec<VolumeInfoFfi>, String> {
    let vols = bdja_scan::list_system_volumes();
    Ok(vols.into_iter().map(|v| VolumeInfoFfi {
        id: v.id,
        path: v.path,
        label: v.label,
        fs_type: v.fs_type,
        is_removable: v.is_removable,
        is_ready: v.is_ready,
        total_bytes: v.total_bytes,
        free_bytes: v.free_bytes,
    }).collect())
}

/// Analiza un unico archivo de audio usando el motor DSP y de veredicto completo.
pub fn analyze_file(path: String) -> Result<FileReportFfi, String> {
    if !*INITIALIZED.read() {
        return Err("El motor no ha sido inicializado con un token de capacidad valido".to_string());
    }

    let p = Path::new(&path);
    let mut report = bdja_scan::analyze_single_file(p)?;

    // Persist in store if open
    if let Some(store) = STORE.read().as_ref() {
        if let Ok(id) = store.save_report(&report) {
            report.file_id = id;
        }
    }

    let spectrum_db = generate_spectrum_curve(&report);
    Ok(map_report_to_ffi(report, spectrum_db))
}

/// Analisis rapido preliminar (alias compatible).
pub fn analyze_file_quick(path: String) -> Result<FileReportFfi, String> {
    analyze_file(path)
}

/// Analiza un lote de archivos seleccionados.
pub fn analyze_batch(paths: Vec<String>) -> Result<Vec<FileReportFfi>, String> {
    if !*INITIALIZED.read() {
        return Err("El motor no ha sido inicializado".to_string());
    }

    let mut results = Vec::new();
    for p_str in paths {
        let p = Path::new(&p_str);
        if let Ok(mut report) = bdja_scan::analyze_single_file(p) {
            if let Some(store) = STORE.read().as_ref() {
                if let Ok(id) = store.save_report(&report) {
                    report.file_id = id;
                }
            }
            let spec = generate_spectrum_curve(&report);
            results.push(map_report_to_ffi(report, spec));
        }
    }
    Ok(results)
}

/// Escanea una carpeta o unidad y analiza hasta `max_files` archivos de audio encontrados.
pub fn scan_directory_audio(root_path: String, max_files: u32) -> Result<Vec<FileReportFfi>, String> {
    if !*INITIALIZED.read() {
        return Err("El motor no ha sido inicializado".to_string());
    }

    let root = Path::new(&root_path);
    if !root.exists() {
        return Err(format!("La ruta no existe: {}", root_path));
    }

    let mut found: Vec<PathBuf> = Vec::new();
    for entry in jwalk::WalkDir::new(root).skip_hidden(true) {
        if let Ok(e) = entry {
            if e.file_type().is_file() && bdja_scan::is_audio_file(&e.path()) {
                found.push(e.path());
                if found.len() >= max_files as usize {
                    break;
                }
            }
        }
    }

    let mut reports = Vec::new();
    for p in found {
        if let Ok(mut report) = bdja_scan::analyze_single_file(&p) {
            if let Some(store) = STORE.read().as_ref() {
                if let Ok(id) = store.save_report(&report) {
                    report.file_id = id;
                }
            }
            let spec = generate_spectrum_curve(&report);
            reports.push(map_report_to_ffi(report, spec));
        }
    }

    Ok(reports)
}

/// Consulta reportes guardados en la base de datos local SQLite.
pub fn query_saved_reports(
    verdict_filter: Option<String>,
    search: Option<String>,
    limit: u32,
    offset: u32,
) -> Result<Vec<FileReportFfi>, String> {
    let store_lock = STORE.read();
    let store = store_lock.as_ref().ok_or("Base de datos no disponible")?;

    let reports = store
        .list_reports(
            verdict_filter.as_deref(),
            search.as_deref(),
            limit as usize,
            offset as usize,
        )
        .map_err(|e| e.to_string())?;

    Ok(reports
        .into_iter()
        .map(|r| {
            let spec = generate_spectrum_curve(&r);
            map_report_to_ffi(r, spec)
        })
        .collect())
}

/// Exporta los reportes guardados a archivo CSV.
pub fn export_reports_csv(out_path: String) -> Result<bool, String> {
    let store_lock = STORE.read();
    let store = store_lock.as_ref().ok_or("Base de datos no disponible")?;
    let reports = store
        .list_reports(None, None, 100000, 0)
        .map_err(|e| e.to_string())?;

    bdja_store::export_to_csv(&reports, Path::new(&out_path))
        .map_err(|e| format!("Error al exportar CSV: {}", e))?;
    Ok(true)
}

/// Exporta los reportes guardados a archivo JSON.
pub fn export_reports_json(out_path: String) -> Result<bool, String> {
    let store_lock = STORE.read();
    let store = store_lock.as_ref().ok_or("Base de datos no disponible")?;
    let reports = store
        .list_reports(None, None, 100000, 0)
        .map_err(|e| e.to_string())?;

    bdja_store::export_to_json(&reports, Path::new(&out_path))
        .map_err(|e| format!("Error al exportar JSON: {}", e))?;
    Ok(true)
}

/// Diagnostico interno del motor nativo.
pub fn diagnostics() -> Result<String, String> {
    Ok(format!(
        "BDJ Studio Audio Analyzer Engine v1.0.0 (Rev {}). Initialized: {}",
        ENGINE_REV,
        *INITIALIZED.read()
    ))
}

fn generate_spectrum_curve(report: &FileReport) -> Vec<f32> {
    let cutoff_hz = report.effective_bandwidth_hz.unwrap_or(20000) as f32;
    let nyquist = (report.facts.sample_rate / 2) as f32;
    let slope = report.cutoff_slope_db_oct.unwrap_or(24.0) as f32;

    let points = 256;
    let mut curve = Vec::with_capacity(points);

    for i in 0..points {
        let f = (i as f32 / (points - 1) as f32) * nyquist;
        let db = if f <= cutoff_hz {
            -12.0 - 18.0 * (f / cutoff_hz.max(1.0))
        } else {
            let octaves = (f / cutoff_hz.max(1.0)).log2();
            (-30.0 - slope * octaves).clamp(-120.0, -30.0)
        };
        curve.push(db);
    }

    curve
}