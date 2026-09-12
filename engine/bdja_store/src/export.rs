use std::fs::File;
use std::io::Write;
use std::path::Path;
use bdja_core::types::FileReport;

pub fn export_to_json(reports: &[FileReport], path: &Path) -> std::io::Result<()> {
    let json = serde_json::to_string_pretty(reports)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
    let mut file = File::create(path)?;
    file.write_all(json.as_bytes())?;
    Ok(())
}

pub fn export_to_csv(reports: &[FileReport], path: &Path) -> std::io::Result<()> {
    let mut file = File::create(path)?;
    writeln!(
        file,
        "ID,Ruta,Tamano_Bytes,Contenedor,Codec,SampleRate,BitDepth,Canales,Duracion_ms,Veredicto,Confianza,LLR,AnchoBanda_Hz,Pendiente_dBOct,TruePeak_dBTP,LUFS,Clipping,Offset_DC,CorrelacionEstereo,Resumen"
    )?;

    for r in reports {
        let clean_path = r.path.replace('"', "\"\"");
        let clean_summary = r.verdict_summary.replace('"', "\"\"");
        writeln!(
            file,
            "{},\"{}\",{},\"{}\",\"{}\",{},{},{},{},\"{:?}\",{:.2},{:.2},{},{:.1},{:.1},{:.1},{},{:.5},{:.2},\"{}\"",
            r.file_id,
            clean_path,
            r.file_size,
            r.facts.container,
            r.facts.codec,
            r.facts.sample_rate,
            r.facts.bit_depth.unwrap_or(0),
            r.facts.channels,
            r.facts.duration_ms,
            r.verdict,
            r.confidence,
            r.score_llr,
            r.effective_bandwidth_hz.unwrap_or(0),
            r.cutoff_slope_db_oct.unwrap_or(0.0),
            r.quality.true_peak_dbtp.unwrap_or(0.0),
            r.quality.lufs_integrated.unwrap_or(-100.0),
            r.quality.clipped_samples,
            r.quality.dc_offset.unwrap_or(0.0),
            r.quality.stereo_correlation.unwrap_or(1.0),
            clean_summary
        )?;
    }
    Ok(())
}