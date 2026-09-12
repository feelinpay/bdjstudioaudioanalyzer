use std::collections::HashMap;
use std::path::Path;
use parking_lot::Mutex;
use rusqlite::{params, Connection};
use bdja_core::types::{Evidence, FileReport, FormatFacts, QualityMetrics, Verdict};

pub struct ReportStore {
    conn: Mutex<Connection>,
}

const SELECT_FIELDS: &str = "id, path, file_size, engine_rev, container, codec, sample_rate, bit_depth, channels, duration_ms, container_bitrate_kbps, is_lossless_declared, verdict, confidence, score_llr, effective_bandwidth_hz, cutoff_slope_db_oct, true_peak_dbtp, lufs_integrated, clipped_samples, dc_offset, dynamic_range_db, stereo_correlation, guards_json, evidences_json, verdict_summary, codec_type, spectrum_json";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DuplicateGroup {
    pub blake3_hash: String,
    pub count: usize,
    pub file_size: u64,
    pub reports: Vec<FileReport>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ScanJobRecord {
    pub job_id: i64,
    pub roots_json: String,
    pub total_found: u64,
    pub analyzed_count: u64,
    pub status: String,
    pub last_path: String,
    pub updated_at: i64,
}

fn parse_report_row(row: &rusqlite::Row) -> Result<FileReport, rusqlite::Error> {
    let id: i64 = row.get(0)?;
    let path: String = row.get(1)?;
    let file_size: u64 = row.get(2)?;
    let engine_rev: u32 = row.get(3)?;
    let container: String = row.get(4)?;
    let codec: String = row.get(5)?;
    let sample_rate: u32 = row.get(6)?;
    let bit_depth: Option<u16> = row.get(7)?;
    let channels: u16 = row.get(8)?;
    let duration_ms: u64 = row.get(9)?;
    let container_bitrate_kbps: Option<u32> = row.get(10)?;
    let is_lossless_declared: bool = row.get::<_, i64>(11)? != 0;
    let verdict_str: String = row.get(12)?;
    let confidence: f64 = row.get(13)?;
    let score_llr: f64 = row.get(14)?;
    let effective_bandwidth_hz: Option<u32> = row.get(15)?;
    let cutoff_slope_db_oct: Option<f64> = row.get(16)?;
    let true_peak_dbtp: Option<f64> = row.get(17)?;
    let lufs_integrated: Option<f64> = row.get(18)?;
    let clipped_samples: u64 = row.get(19)?;
    let dc_offset: Option<f64> = row.get(20)?;
    let dynamic_range_db: Option<f64> = row.get(21)?;
    let stereo_correlation: Option<f64> = row.get(22)?;
    let guards_json: String = row.get(23)?;
    let evidences_json: String = row.get(24)?;
    let verdict_summary: String = row.get(25)?;
    let codec_type_str: String = row.get(26).unwrap_or_else(|_| "Unknown".to_string());
    let spectrum_json: String = row.get(27).unwrap_or_else(|_| "[]".to_string());

    let verdict = match verdict_str.as_str() {
        "LosslessVerified" => Verdict::LosslessVerified,
        "LikelyLossless" => Verdict::LikelyLossless,
        "Inconclusive" => Verdict::Inconclusive,
        "Suspicious" => Verdict::Suspicious,
        "ProbableTranscode" => Verdict::ProbableTranscode,
        _ => Verdict::DeclaredLossy,
    };

    let guards_triggered: Vec<String> = serde_json::from_str(&guards_json).unwrap_or_default();
    let evidences: Vec<Evidence> = serde_json::from_str(&evidences_json).unwrap_or_default();
    let average_spectrum_db: Vec<f32> = serde_json::from_str(&spectrum_json).unwrap_or_else(|_| vec![-120.0; 256]);

    let codec_type = match codec_type_str.as_str() {
        "PcmS16Le" => bdja_core::types::Codec::PcmS16Le,
        "PcmS24Le" => bdja_core::types::Codec::PcmS24Le,
        "PcmS32Le" => bdja_core::types::Codec::PcmS32Le,
        "PcmF32Le" => bdja_core::types::Codec::PcmF32Le,
        "Flac" => bdja_core::types::Codec::Flac,
        "Alac" => bdja_core::types::Codec::Alac,
        "Mp3" => bdja_core::types::Codec::Mp3,
        "Aac" => bdja_core::types::Codec::Aac,
        "Vorbis" => bdja_core::types::Codec::Vorbis,
        "Opus" => bdja_core::types::Codec::Opus,
        _ => bdja_core::types::Codec::Unknown,
    };

    Ok(FileReport {
        file_id: id,
        path,
        file_size,
        engine_rev,
        facts: FormatFacts {
            container,
            codec,
            codec_type,
            sample_rate,
            bit_depth,
            channels,
            duration_ms,
            container_bitrate_kbps,
            is_lossless_declared,
        },
        verdict,
        confidence,
        score_llr,
        effective_bandwidth_hz,
        cutoff_slope_db_oct,
        evidences,
        quality: QualityMetrics {
            true_peak_dbtp,
            lufs_integrated,
            clipped_samples,
            dc_offset,
            dynamic_range_db,
            stereo_correlation,
        },
        guards_triggered,
        verdict_summary,
        average_spectrum_db,
    })
}

impl ReportStore {
    pub fn open_in_memory() -> Result<Self, rusqlite::Error> {
        let conn = Connection::open_in_memory()?;
        let store = Self {
            conn: Mutex::new(conn),
        };
        store.init_schema()?;
        Ok(store)
    }

    pub fn open(path: &Path) -> Result<Self, rusqlite::Error> {
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        let store = Self {
            conn: Mutex::new(conn),
        };
        store.init_schema()?;
        Ok(store)
    }

    fn init_schema(&self) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock();
        
        let version: i32 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap_or(0);

        if version < 1 {
            conn.execute_batch(
                "
                CREATE TABLE IF NOT EXISTS file_report (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    path TEXT NOT NULL UNIQUE,
                    file_size INTEGER NOT NULL,
                    engine_rev INTEGER NOT NULL,
                    mtime_utc INTEGER NOT NULL DEFAULT 0,
                    blake3_hash TEXT NOT NULL DEFAULT '',
                    container TEXT NOT NULL,
                    codec TEXT NOT NULL,
                    codec_type TEXT NOT NULL DEFAULT 'Unknown',
                    sample_rate INTEGER NOT NULL,
                    bit_depth INTEGER,
                    channels INTEGER NOT NULL,
                    duration_ms INTEGER NOT NULL,
                    container_bitrate_kbps INTEGER,
                    is_lossless_declared INTEGER NOT NULL,
                    verdict TEXT NOT NULL,
                    confidence REAL NOT NULL,
                    score_llr REAL NOT NULL,
                    effective_bandwidth_hz INTEGER,
                    cutoff_slope_db_oct REAL,
                    true_peak_dbtp REAL,
                    lufs_integrated REAL,
                    clipped_samples INTEGER NOT NULL,
                    dc_offset REAL,
                    dynamic_range_db REAL,
                    stereo_correlation REAL,
                    guards_json TEXT NOT NULL,
                    evidences_json TEXT NOT NULL,
                    verdict_summary TEXT NOT NULL,
                    spectrum_json TEXT NOT NULL DEFAULT '[]'
                );
                CREATE INDEX IF NOT EXISTS idx_verdict ON file_report(verdict);
                CREATE INDEX IF NOT EXISTS idx_cache ON file_report(path, file_size, mtime_utc, engine_rev);
                CREATE INDEX IF NOT EXISTS idx_blake3 ON file_report(blake3_hash);

                CREATE TABLE IF NOT EXISTS scan_job (
                    job_id INTEGER PRIMARY KEY,
                    roots_json TEXT NOT NULL,
                    total_found INTEGER NOT NULL DEFAULT 0,
                    analyzed_count INTEGER NOT NULL DEFAULT 0,
                    status TEXT NOT NULL,
                    last_path TEXT NOT NULL DEFAULT '',
                    updated_at INTEGER NOT NULL DEFAULT 0
                );

                PRAGMA user_version = 1;
                ",
            )?;
        }

        if version < 2 {
            // Migraciones seguras para bases preexistentes
            let _ = conn.execute("ALTER TABLE file_report ADD COLUMN codec_type TEXT NOT NULL DEFAULT 'Unknown'", []);
            let _ = conn.execute("ALTER TABLE file_report ADD COLUMN spectrum_json TEXT NOT NULL DEFAULT '[]'", []);
            let _ = conn.execute("ALTER TABLE file_report ADD COLUMN mtime_utc INTEGER NOT NULL DEFAULT 0", []);
            let _ = conn.execute("ALTER TABLE file_report ADD COLUMN blake3_hash TEXT NOT NULL DEFAULT ''", []);
            let _ = conn.execute("CREATE INDEX IF NOT EXISTS idx_blake3 ON file_report(blake3_hash)", []);
            let _ = conn.execute(
                "CREATE TABLE IF NOT EXISTS scan_job (
                    job_id INTEGER PRIMARY KEY,
                    roots_json TEXT NOT NULL,
                    total_found INTEGER NOT NULL DEFAULT 0,
                    analyzed_count INTEGER NOT NULL DEFAULT 0,
                    status TEXT NOT NULL,
                    last_path TEXT NOT NULL DEFAULT '',
                    updated_at INTEGER NOT NULL DEFAULT 0
                )",
                [],
            );
            conn.execute_batch("PRAGMA user_version = 2;")?;
        }

        Ok(())
    }

    pub fn save_report(&self, report: &FileReport) -> Result<i64, rusqlite::Error> {
        let conn = self.conn.lock();
        let guards_json = serde_json::to_string(&report.guards_triggered).unwrap_or_default();
        let evidences_json = serde_json::to_string(&report.evidences).unwrap_or_default();
        let spectrum_json = serde_json::to_string(&report.average_spectrum_db).unwrap_or_default();
        let verdict_str = format!("{:?}", report.verdict);
        let codec_type_str = format!("{:?}", report.facts.codec_type);

        let (mtime_utc, blake3_hash) = match std::fs::File::open(&report.path) {
            Ok(mut file) => {
                let mtime = file
                    .metadata()
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0);

                let mut buffer = [0u8; 65536];
                let n = std::io::Read::read(&mut file, &mut buffer).unwrap_or(0);
                let hash = blake3::hash(&buffer[..n]).to_hex().to_string();
                (mtime, hash)
            }
            Err(_) => (0, String::new()),
        };

        conn.execute(
            "
            INSERT OR REPLACE INTO file_report (
                path, file_size, engine_rev, mtime_utc, blake3_hash, container, codec, codec_type, sample_rate,
                bit_depth, channels, duration_ms, container_bitrate_kbps,
                is_lossless_declared, verdict, confidence, score_llr,
                effective_bandwidth_hz, cutoff_slope_db_oct, true_peak_dbtp,
                lufs_integrated, clipped_samples, dc_offset, dynamic_range_db,
                stereo_correlation, guards_json, evidences_json, verdict_summary, spectrum_json
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14,
                ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29
            );
            ",
            params![
                report.path,
                report.file_size,
                report.engine_rev,
                mtime_utc,
                blake3_hash,
                report.facts.container,
                report.facts.codec,
                codec_type_str,
                report.facts.sample_rate,
                report.facts.bit_depth,
                report.facts.channels,
                report.facts.duration_ms,
                report.facts.container_bitrate_kbps,
                if report.facts.is_lossless_declared { 1 } else { 0 },
                verdict_str,
                report.confidence,
                report.score_llr,
                report.effective_bandwidth_hz,
                report.cutoff_slope_db_oct,
                report.quality.true_peak_dbtp,
                report.quality.lufs_integrated,
                report.quality.clipped_samples,
                report.quality.dc_offset,
                report.quality.dynamic_range_db,
                report.quality.stereo_correlation,
                guards_json,
                evidences_json,
                report.verdict_summary,
                spectrum_json,
            ],
        )?;

        Ok(conn.last_insert_rowid())
    }

    pub fn list_reports(
        &self,
        verdict_filter: Option<&str>,
        search: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<FileReport>, rusqlite::Error> {
        let conn = self.conn.lock();
        let mut query = format!("SELECT {} FROM file_report WHERE 1=1", SELECT_FIELDS);
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(v) = verdict_filter {
            if !v.is_empty() && v != "ALL" {
                query.push_str(" AND verdict = ?");
                params.push(Box::new(v.to_string()));
            }
        }
        if let Some(s) = search {
            if !s.is_empty() {
                query.push_str(" AND path LIKE ? ESCAPE '\\'");
                let escaped = s.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_");
                params.push(Box::new(format!("%{}%", escaped)));
            }
        }

        query.push_str(" ORDER BY id DESC LIMIT ? OFFSET ?");
        params.push(Box::new(limit as i64));
        params.push(Box::new(offset as i64));

        let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn.prepare(&query)?;
        let rows = stmt.query_map(&params_refs[..], parse_report_row)?;

        let mut list = Vec::new();
        for item in rows.flatten() {
            list.push(item);
        }
        Ok(list)
    }

    pub fn count_reports(&self, verdict_filter: Option<&str>, search: Option<&str>) -> Result<u64, rusqlite::Error> {
        let conn = self.conn.lock();
        let mut query = "SELECT COUNT(*) FROM file_report WHERE 1=1".to_string();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(v) = verdict_filter {
            if !v.is_empty() && v != "ALL" {
                query.push_str(" AND verdict = ?");
                params.push(Box::new(v.to_string()));
            }
        }
        if let Some(s) = search {
            if !s.is_empty() {
                query.push_str(" AND path LIKE ? ESCAPE '\\'");
                let escaped = s.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_");
                params.push(Box::new(format!("%{}%", escaped)));
            }
        }

        let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        conn.query_row(&query, &params_refs[..], |r| r.get(0))
    }

    pub fn count_by_verdict(&self) -> Result<HashMap<String, u64>, rusqlite::Error> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare("SELECT verdict, COUNT(*) FROM file_report GROUP BY verdict")?;
        let rows = stmt.query_map([], |row| {
            let v: String = row.get(0)?;
            let count: u64 = row.get(1)?;
            Ok((v, count))
        })?;

        let mut map = HashMap::new();
        for (v, count) in rows.flatten() {
            map.insert(v, count);
        }
        Ok(map)
    }

    pub fn get_report_by_path(&self, path: &str) -> Result<Option<FileReport>, rusqlite::Error> {
        let conn = self.conn.lock();
        let query = format!("SELECT {} FROM file_report WHERE path = ?1 LIMIT 1", SELECT_FIELDS);
        let mut stmt = conn.prepare(&query)?;
        let mut rows = stmt.query_map(params![path], parse_report_row)?;

        if let Some(res) = rows.next() {
            Ok(Some(res?))
        } else {
            Ok(None)
        }
    }

    pub fn get_cached_report(
        &self,
        path: &str,
        file_size: u64,
        mtime_utc: i64,
        engine_rev: u32,
    ) -> Result<Option<FileReport>, rusqlite::Error> {
        let conn = self.conn.lock();
        let query = format!(
            "SELECT {} FROM file_report WHERE path = ?1 AND file_size = ?2 AND mtime_utc = ?3 AND engine_rev = ?4 LIMIT 1",
            SELECT_FIELDS
        );

        let mut stmt = conn.prepare(&query)?;
        let mut rows = stmt.query_map(params![path, file_size, mtime_utc, engine_rev], parse_report_row)?;

        if let Some(res) = rows.next() {
            Ok(Some(res?))
        } else {
            Ok(None)
        }
    }

    pub fn get_cached_by_hash(&self, blake3_hash: &str, engine_rev: u32) -> Result<Option<FileReport>, rusqlite::Error> {
        if blake3_hash.is_empty() {
            return Ok(None);
        }
        let conn = self.conn.lock();
        let query = format!("SELECT {} FROM file_report WHERE blake3_hash = ?1 AND engine_rev = ?2 LIMIT 1", SELECT_FIELDS);
        let mut stmt = conn.prepare(&query)?;
        let mut rows = stmt.query_map(params![blake3_hash, engine_rev], parse_report_row)?;

        if let Some(res) = rows.next() {
            Ok(Some(res?))
        } else {
            Ok(None)
        }
    }

    pub fn find_duplicates(&self, limit_groups: usize) -> Result<Vec<DuplicateGroup>, rusqlite::Error> {
        let conn = self.conn.lock();
        let dup_query = "SELECT blake3_hash, COUNT(*) as cnt, MIN(file_size) as fsize FROM file_report WHERE blake3_hash != '' GROUP BY blake3_hash HAVING cnt > 1 ORDER BY cnt DESC LIMIT ?1";
        let mut stmt = conn.prepare(dup_query)?;
        let dup_hashes: Vec<(String, usize, u64)> = stmt
            .query_map(params![limit_groups as i64], |r| {
                let h: String = r.get(0)?;
                let c: i64 = r.get(1)?;
                let s: u64 = r.get(2)?;
                Ok((h, c as usize, s))
            })?
            .flatten()
            .collect();

        let mut groups = Vec::new();
        let select_query = format!("SELECT {} FROM file_report WHERE blake3_hash = ?1 ORDER BY id ASC", SELECT_FIELDS);
        let mut select_stmt = conn.prepare(&select_query)?;

        for (hash, count, file_size) in dup_hashes {
            let reports = select_stmt
                .query_map(params![hash], parse_report_row)?
                .flatten()
                .collect();
            groups.push(DuplicateGroup {
                blake3_hash: hash,
                count,
                file_size,
                reports,
            });
        }
        Ok(groups)
    }

    pub fn save_or_update_scan_job(&self, job: &ScanJobRecord) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT OR REPLACE INTO scan_job (job_id, roots_json, total_found, analyzed_count, status, last_path, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![job.job_id, job.roots_json, job.total_found, job.analyzed_count, job.status, job.last_path, job.updated_at],
        )?;
        Ok(())
    }

    pub fn get_scan_job(&self, job_id: i64) -> Result<Option<ScanJobRecord>, rusqlite::Error> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare("SELECT job_id, roots_json, total_found, analyzed_count, status, last_path, updated_at FROM scan_job WHERE job_id = ?1")?;
        let mut rows = stmt.query(params![job_id])?;
        if let Some(r) = rows.next()? {
            Ok(Some(ScanJobRecord {
                job_id: r.get(0)?,
                roots_json: r.get(1)?,
                total_found: r.get(2)?,
                analyzed_count: r.get(3)?,
                status: r.get(4)?,
                last_path: r.get(5)?,
                updated_at: r.get(6)?,
            }))
        } else {
            Ok(None)
        }
    }
}