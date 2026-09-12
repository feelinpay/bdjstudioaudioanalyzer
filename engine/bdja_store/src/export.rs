use std::fs::File;
use std::io::Write;
use std::path::Path;
use bdja_core::types::FileReport;

pub fn export_to_json(reports: &[FileReport], path: &Path) -> std::io::Result<()> {
    let json = serde_json::to_string_pretty(reports)
        .map_err(|e| std::io::Error::other(e.to_string()))?;
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

pub fn export_store_to_csv(store: &crate::db::ReportStore, path: &Path) -> std::io::Result<usize> {
    let file = File::create(path)?;
    let mut writer = std::io::BufWriter::new(file);
    writeln!(
        writer,
        "ID,Ruta,Tamano_Bytes,Contenedor,Codec,SampleRate,BitDepth,Canales,Duracion_ms,Veredicto,Confianza,LLR,AnchoBanda_Hz,Pendiente_dBOct,TruePeak_dBTP,LUFS,Clipping,Offset_DC,CorrelacionEstereo,Resumen"
    )?;

    let page_size = 1000;
    let mut offset = 0;
    let mut total_written = 0;

    loop {
        let batch = store
            .list_reports(None, None, page_size, offset)
            .map_err(|e| std::io::Error::other(e.to_string()))?;

        if batch.is_empty() {
            break;
        }

        for r in &batch {
            let clean_path = r.path.replace('"', "\"\"");
            let clean_summary = r.verdict_summary.replace('"', "\"\"");
            writeln!(
                writer,
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
            total_written += 1;
        }

        if batch.len() < page_size {
            break;
        }
        offset += page_size;
    }

    writer.flush()?;
    Ok(total_written)
}

pub fn export_store_to_json(store: &crate::db::ReportStore, path: &Path) -> std::io::Result<usize> {
    let file = File::create(path)?;
    let mut writer = std::io::BufWriter::new(file);
    writer.write_all(b"[\n")?;

    let page_size = 1000;
    let mut offset = 0;
    let mut total_written = 0;
    let mut first = true;

    loop {
        let batch = store
            .list_reports(None, None, page_size, offset)
            .map_err(|e| std::io::Error::other(e.to_string()))?;

        if batch.is_empty() {
            break;
        }

        for r in &batch {
            if !first {
                writer.write_all(b",\n")?;
            }
            first = false;
            let json_item = serde_json::to_string(r)
                .map_err(|e| std::io::Error::other(e.to_string()))?;
            writer.write_all(json_item.as_bytes())?;
            total_written += 1;
        }

        if batch.len() < page_size {
            break;
        }
        offset += page_size;
    }

    writer.write_all(b"\n]\n")?;
    writer.flush()?;
    Ok(total_written)
}