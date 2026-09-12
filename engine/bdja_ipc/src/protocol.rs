use bdja_core::types::FileReport;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkerRequest {
    AnalyzeFile { path: String, max_ms: u64 },
    Ping,
    Shutdown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkerResponse {
    Success(FileReport),
    Error(String),
    Pong,
}

/// Escribe un mensaje serializado con bincode precedido por un encabezado de longitud u32 LE
pub fn write_message<W: Write, T: Serialize>(writer: &mut W, msg: &T) -> std::io::Result<()> {
    let payload = bincode::serialize(msg)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    let len = payload.len() as u32;
    writer.write_all(&len.to_le_bytes())?;
    writer.write_all(&payload)?;
    writer.flush()?;
    Ok(())
}

/// Lee un mensaje con prefijo de longitud u32 LE y lo deserializa con bincode
pub fn read_message<R: Read, T: serde::de::DeserializeOwned>(reader: &mut R) -> std::io::Result<T> {
    let mut len_bytes = [0u8; 4];
    reader.read_exact(&mut len_bytes)?;
    let len = u32::from_le_bytes(len_bytes) as usize;

    // Límite de seguridad de 64 MB para evitar asignaciones maliciosas
    if len > 64 * 1024 * 1024 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("Mensaje IPC excede el tamaño máximo: {} bytes", len),
        ));
    }

    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf)?;

    bincode::deserialize(&buf)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_ipc_framing_roundtrip() {
        let mut buffer = Vec::new();
        let req = WorkerRequest::AnalyzeFile {
            path: "C:\\Music\\test.wav".to_string(),
            max_ms: 5000,
        };

        write_message(&mut buffer, &req).expect("Failed to write");
        assert!(buffer.len() > 4);

        let mut cursor = Cursor::new(buffer);
        let decoded: WorkerRequest = read_message(&mut cursor).expect("Failed to read");

        match decoded {
            WorkerRequest::AnalyzeFile { path, max_ms } => {
                assert_eq!(path, "C:\\Music\\test.wav");
                assert_eq!(max_ms, 5000);
            }
            _ => panic!("Wrong variant decoded"),
        }
    }
}
