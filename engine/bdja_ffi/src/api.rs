use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};
use std::sync::Arc;
use parking_lot::{Mutex, RwLock};
use bdja_core::types::FileReport;
use bdja_store::ReportStore;

pub const ENGINE_REV: u32 = 1;

static INITIALIZED: RwLock<bool> = RwLock::new(false);
static DATA_DIR: RwLock<Option<String>> = RwLock::new(None);
static STORE: RwLock<Option<Arc<ReportStore>>> = RwLock::new(None);

#[derive(Debug, Clone, PartialEq)]
pub struct ScanJobStatusFfi {
    pub job_id: i64,
    pub is_active: bool,
    pub is_completed: bool,
    pub total_found: u64,
    pub analyzed_count: u64,
    pub current_path: String,
    pub new_reports: Vec<FileReportFfi>,
}

struct ScanJob {
    _job_id: i64,
    cancel_token: Arc<AtomicBool>,
    total_found: Arc<AtomicU64>,
    analyzed_count: Arc<AtomicU64>,
    current_path: Arc<RwLock<String>>,
    is_completed: Arc<AtomicBool>,
    pending_reports: Arc<Mutex<Vec<FileReportFfi>>>,
}

static NEXT_JOB_ID: AtomicI64 = AtomicI64::new(1);
static ACTIVE_JOBS: RwLock<Option<HashMap<i64, ScanJob>>> = RwLock::new(None);

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

fn map_report_to_ffi(report: FileReport) -> FileReportFfi {
    let verdict_code = format!("{:?}", report.verdict);
    let verdict_name = report.verdict.display_name().to_string();
    let spectrum_db = report.average_spectrum_db;

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
    let token_clean = capability_token.trim();
    if token_clean.is_empty() {
        return Err("Token de capacidad requerido para inicializar el motor".to_string());
    }

    // P0-8: Verificación criptográfica del token de capacidad efímero (§14)
    let parts: Vec<&str> = token_clean.split(':').collect();
    if parts.len() < 2 {
        return Err("Token de capacidad con formato inválido".to_string());
    }
    let hwid = parts[0];
    let token_digest = parts[1];

    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    type HmacSha256 = Hmac<Sha256>;

    let key = b"BDJ_AUDIO_ANALYZER_CAPABILITY_SALT_2026";
    let message = format!("BDJA_CAPABILITY:{}:{}", hwid, ENGINE_REV);

    let mut mac = HmacSha256::new_from_slice(key)
        .map_err(|e| format!("Error HMAC: {}", e))?;
    mac.update(message.as_bytes());
    let expected_bytes = mac.finalize().into_bytes();
    let expected_hex = expected_bytes.iter().map(|b| format!("{:02x}", b)).collect::<String>();

    // Comparación en tiempo constante (sin cortocircuito) para prevenir timing attacks
    let digest_clean = token_digest.trim().to_lowercase();
    let expected_clean = expected_hex.to_lowercase();
    if digest_clean.len() != expected_clean.len() {
        return Err("Token de capacidad inválido o alterado".to_string());
    }
    let mut diff: u8 = 0;
    for (a, b) in digest_clean.bytes().zip(expected_clean.bytes()) {
        diff |= a ^ b;
    }
    if diff != 0 {
        return Err("Token de capacidad inválido o alterado".to_string());
    }

    let mut init = INITIALIZED.write();
    let mut dir = DATA_DIR.write();
    let mut store_lock = STORE.write();

    let db_path = Path::new(&data_dir).join("bdj_audio_analyzer.db");
    match ReportStore::open(&db_path) {
        Ok(store) => *store_lock = Some(Arc::new(store)),
        Err(_) => {
            if let Ok(mem_store) = ReportStore::open_in_memory() {
                *store_lock = Some(Arc::new(mem_store));
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

    Ok(map_report_to_ffi(report))
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
            results.push(map_report_to_ffi(report));
        }
    }
    Ok(results)
}

/// Inicia un trabajo de escaneo masivo asincrono en segundo plano (soporta 100.000+ pistas).
pub fn start_scan_job(
    roots: Vec<String>,
    throttle_mode: String,
    skip_cache: bool,
) -> Result<i64, String> {
    if !*INITIALIZED.read() {
        return Err("El motor no ha sido inicializado".to_string());
    }

    let job_id = NEXT_JOB_ID.fetch_add(1, Ordering::SeqCst);
    let cancel_token = Arc::new(AtomicBool::new(false));
    let total_found = Arc::new(AtomicU64::new(0));
    let analyzed_count = Arc::new(AtomicU64::new(0));
    let current_path = Arc::new(RwLock::new("Iniciando escaneo...".to_string()));
    let is_completed = Arc::new(AtomicBool::new(false));
    let pending_reports = Arc::new(Mutex::new(Vec::new()));

    let job = ScanJob {
        _job_id: job_id,
        cancel_token: cancel_token.clone(),
        total_found: total_found.clone(),
        analyzed_count: analyzed_count.clone(),
        current_path: current_path.clone(),
        is_completed: is_completed.clone(),
        pending_reports: pending_reports.clone(),
    };

    {
        let mut jobs_lock = ACTIVE_JOBS.write();
        let map = jobs_lock.get_or_insert_with(HashMap::new);
        map.insert(job_id, job);
    }

    let store_arc = STORE.read().clone();
    let root_paths: Vec<PathBuf> = roots.into_iter().map(PathBuf::from).collect();

    std::thread::spawn(move || {
        let cancel_clone = cancel_token.clone();
        let total_clone = total_found.clone();
        let count_clone = analyzed_count.clone();
        let path_clone = current_path.clone();
        let pending_clone = pending_reports.clone();

        let _ = bdja_scan::scan_collection(
            &root_paths,
            &throttle_mode,
            skip_cache,
            store_arc,
            cancel_clone,
            move |report| {
                let report_ffi = map_report_to_ffi(report);
                let mut lock = pending_clone.lock();
                // N-6: Acotar cola en memoria a 1.000 reportes (los datos completos ya están en SQLite)
                if lock.len() < 1000 {
                    lock.push(report_ffi);
                }
            },
            move |count, total, path_display| {
                total_clone.store(total, Ordering::Relaxed);
                count_clone.store(count, Ordering::Relaxed);
                *path_clone.write() = path_display;
            },
        );

        is_completed.store(true, Ordering::SeqCst);
    });

    Ok(job_id)
}

/// Consulta el progreso y recoge nuevos reportes generados desde la ultima consulta.
pub fn poll_scan_job(job_id: i64) -> Result<ScanJobStatusFfi, String> {
    let (is_completed, is_active, total_found, analyzed_count, current_path, new_reports) = {
        let jobs_lock = ACTIVE_JOBS.read();
        let map = jobs_lock.as_ref().ok_or("No hay trabajos activos")?;
        let job = map.get(&job_id).ok_or_else(|| format!("Trabajo {} no encontrado", job_id))?;

        let is_completed = job.is_completed.load(Ordering::SeqCst);
        let is_active = !is_completed;
        let total_found = job.total_found.load(Ordering::Relaxed);
        let analyzed_count = job.analyzed_count.load(Ordering::Relaxed);
        let current_path = job.current_path.read().clone();
        let new_reports = {
            let mut lock = job.pending_reports.lock();
            std::mem::take(&mut *lock)
        };
        (is_completed, is_active, total_found, analyzed_count, current_path, new_reports)
    };

    if is_completed {
        // N-7: Purgar trabajos completados antiguos en ACTIVE_JOBS
        let mut jobs_write = ACTIVE_JOBS.write();
        if let Some(map) = jobs_write.as_mut() {
            if map.len() > 10 {
                map.retain(|id, j| *id == job_id || !j.is_completed.load(Ordering::Relaxed));
            }
        }
    }

    Ok(ScanJobStatusFfi {
        job_id,
        is_active,
        is_completed,
        total_found,
        analyzed_count,
        current_path,
        new_reports,
    })
}

/// Cancela un trabajo de escaneo especifico en tiempo real.
pub fn cancel_scan_job(job_id: i64) -> bool {
    let jobs_lock = ACTIVE_JOBS.read();
    if let Some(map) = jobs_lock.as_ref() {
        if let Some(job) = map.get(&job_id) {
            job.cancel_token.store(true, Ordering::SeqCst);
            return true;
        }
    }
    false
}

/// Cancela todos los trabajos de escaneo activos inmediatamente.
pub fn cancel_all_scans() -> bool {
    let jobs_lock = ACTIVE_JOBS.read();
    if let Some(map) = jobs_lock.as_ref() {
        for job in map.values() {
            job.cancel_token.store(true, Ordering::SeqCst);
        }
        return true;
    }
    false
}

/// Escanea una carpeta o unidad y analiza hasta `max_files` archivos de audio encontrados (0 para ilimitado).
pub fn scan_directory_audio(root_path: String, max_files: u32) -> Result<Vec<FileReportFfi>, String> {
    if !*INITIALIZED.read() {
        return Err("El motor no ha sido inicializado".to_string());
    }

    let root = Path::new(&root_path);
    if !root.exists() {
        return Err(format!("La ruta no existe: {}", root_path));
    }

    let cancel_token = Arc::new(AtomicBool::new(false));
    let store_arc = STORE.read().clone();
    let roots = vec![root.to_path_buf()];

    let reports_acc = Arc::new(Mutex::new(Vec::new()));
    let reports_clone = reports_acc.clone();
    let cancel_clone = cancel_token.clone();

    let _ = bdja_scan::scan_collection(
        &roots,
        "turbo",
        false,
        store_arc,
        cancel_token,
        move |rep| {
            let mut list = reports_clone.lock();
            if max_files == 0 || list.len() < max_files as usize {
                list.push(map_report_to_ffi(rep));
            }
            if max_files > 0 && list.len() >= max_files as usize {
                cancel_clone.store(true, Ordering::Relaxed);
            }
        },
        |_, _, _| {},
    );

    let res = std::mem::take(&mut *reports_acc.lock());
    Ok(res)
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
        .map(map_report_to_ffi)
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