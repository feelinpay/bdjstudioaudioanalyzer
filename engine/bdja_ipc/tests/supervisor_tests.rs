use bdja_ipc::WorkerSupervisor;

#[test]
fn test_supervisor_worker_ping() {
    // If worker binary exists in target/debug or target/release, test ping
    if let Some(worker_path) = WorkerSupervisor::find_worker_binary() {
        let mut supervisor = WorkerSupervisor::new(worker_path);
        let ping_result = supervisor.ping();
        assert!(ping_result.is_ok(), "Worker ping should succeed: {:?}", ping_result);
        assert_eq!(ping_result.unwrap(), true);
    } else {
        println!("Skipping worker spawn test (binary not yet in target directory)");
    }
}
