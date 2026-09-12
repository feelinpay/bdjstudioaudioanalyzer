use bdja_ipc::protocol::{read_message, write_message, WorkerRequest, WorkerResponse};
use std::io::{stdin, stdout, BufReader, BufWriter};
use std::path::Path;

fn main() {
    let stdin = stdin();
    let stdout = stdout();
    let mut reader = BufReader::new(stdin.lock());
    let mut writer = BufWriter::new(stdout.lock());

    loop {
        match read_message::<_, WorkerRequest>(&mut reader) {
            Ok(WorkerRequest::AnalyzeFile { path, .. }) => {
                let p = Path::new(&path);
                let response = match bdja_scan::analyze_single_file(p) {
                    Ok(report) => WorkerResponse::Success(Box::new(report)),
                    Err(err) => WorkerResponse::Error(err),
                };
                if write_message(&mut writer, &response).is_err() {
                    break;
                }
            }
            Ok(WorkerRequest::Ping) => {
                if write_message(&mut writer, &WorkerResponse::Pong).is_err() {
                    break;
                }
            }
            Ok(WorkerRequest::Shutdown) => {
                break;
            }
            Err(_) => {
                // Pipe cerrado por el proceso supervisor o EOF -> salida limpia
                break;
            }
        }
    }
}
