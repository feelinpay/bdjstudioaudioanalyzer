use crate::protocol::{read_message, write_message, WorkerRequest, WorkerResponse};
use bdja_core::types::FileReport;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

pub struct WorkerProcess {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl WorkerProcess {
    pub fn spawn(worker_bin: &Path) -> std::io::Result<Self> {
        let mut child = Command::new(worker_bin)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::BrokenPipe, "No se pudo conectar a stdin del worker"))?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::BrokenPipe, "No se pudo conectar a stdout del worker"))?;

        Ok(Self {
            child,
            stdin,
            stdout: BufReader::new(stdout),
        })
    }

    pub fn send_request(&mut self, req: &WorkerRequest) -> std::io::Result<()> {
        write_message(&mut self.stdin, req)
    }

    pub fn recv_response(&mut self) -> std::io::Result<WorkerResponse> {
        read_message(&mut self.stdout)
    }

    pub fn request(&mut self, req: &WorkerRequest) -> std::io::Result<WorkerResponse> {
        self.send_request(req)?;
        self.recv_response()
    }

    pub fn is_alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    pub fn shutdown(&mut self) {
        let _ = write_message(&mut self.stdin, &WorkerRequest::Shutdown);
        let _ = self.child.wait();
    }

    pub fn kill(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for WorkerProcess {
    fn drop(&mut self) {
        if self.is_alive() {
            self.shutdown();
        }
    }
}

/// Supervisor de procesos para aislar análisis en workers desechables
pub struct WorkerSupervisor {
    worker_bin_path: PathBuf,
    active_worker: Option<WorkerProcess>,
}

impl WorkerSupervisor {
    pub fn new(worker_bin_path: PathBuf) -> Self {
        Self {
            worker_bin_path,
            active_worker: None,
        }
    }

    /// Intenta localizar automáticamente el binario bdja_worker.exe
    pub fn find_worker_binary() -> Option<PathBuf> {
        let exe_name = if cfg!(windows) { "bdja_worker.exe" } else { "bdja_worker" };

        // 1. En el mismo directorio del ejecutable actual
        if let Ok(current_exe) = std::env::current_exe() {
            if let Some(parent) = current_exe.parent() {
                let candidate = parent.join(exe_name);
                if candidate.exists() {
                    return Some(candidate);
                }
            }
        }

        // 2. En target/release o target/debug del workspace
        let candidates = [
            PathBuf::from("target/release").join(exe_name),
            PathBuf::from("target/debug").join(exe_name),
            PathBuf::from("../target/release").join(exe_name),
            PathBuf::from("../target/debug").join(exe_name),
        ];

        for cand in &candidates {
            if cand.exists() {
                return Some(cand.clone());
            }
        }

        None
    }

    fn ensure_worker(&mut self) -> Result<&mut WorkerProcess, String> {
        if let Some(ref mut w) = self.active_worker {
            if w.is_alive() {
                return Ok(self.active_worker.as_mut().unwrap());
            }
        }

        // Si no está vivo o no existe, lanzar nuevo
        let w = WorkerProcess::spawn(&self.worker_bin_path)
            .map_err(|e| format!("Error iniciando worker {}: {}", self.worker_bin_path.display(), e))?;
        self.active_worker = Some(w);
        Ok(self.active_worker.as_mut().unwrap())
    }

    pub fn ping(&mut self) -> Result<bool, String> {
        let worker = self.ensure_worker()?;
        match worker.request(&WorkerRequest::Ping) {
            Ok(WorkerResponse::Pong) => Ok(true),
            Ok(other) => Err(format!("Respuesta inesperada al ping: {:?}", other)),
            Err(e) => Err(format!("Error enviando ping: {}", e)),
        }
    }

    /// Analiza un archivo a través del worker aislado. Si el worker cae, lo reinicia
    pub fn analyze_file(&mut self, path: &str, max_ms: u64) -> Result<FileReport, String> {
        let worker = self.ensure_worker()?;

        let req = WorkerRequest::AnalyzeFile {
            path: path.to_string(),
            max_ms,
        };

        match worker.request(&req) {
            Ok(WorkerResponse::Success(report)) => Ok(*report),
            Ok(WorkerResponse::Error(err)) => Err(err),
            Ok(WorkerResponse::Pong) => Err("Respuesta inesperada del worker: Pong".to_string()),
            Err(e) => {
                // Fallo de comunicación o crash del proceso hijo
                if let Some(mut old_worker) = self.active_worker.take() {
                    old_worker.kill();
                }
                Err(format!("Worker falló durante el análisis de {}: {}", path, e))
            }
        }
    }
}
