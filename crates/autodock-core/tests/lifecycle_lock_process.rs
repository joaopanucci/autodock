use autodock_core::lifecycle_lock::LifecycleFileLock;
use std::{
    io::{BufRead, BufReader, Write},
    process::{Command, Stdio},
    thread,
    time::Duration,
};

#[test]
fn lifecycle_lock_process_helper() {
    if std::env::var_os("AUTODOCK_LOCK_HELPER").is_none() {
        return;
    }

    let path = std::path::PathBuf::from(
        std::env::var_os("AUTODOCK_LOCK_PATH").expect("AUTODOCK_LOCK_PATH ausente"),
    );

    match LifecycleFileLock::try_acquire(&path) {
        Ok(_guard) => {
            println!("LOCK_ACQUIRED");
            std::io::stdout().flush().unwrap();
            thread::sleep(Duration::from_secs(3));
        }
        Err(error) if error.contains("ocupado") => {
            println!("LOCK_BUSY");
            std::io::stdout().flush().unwrap();
        }
        Err(error) => panic!("Falha inesperada: {error}"),
    }
}

#[test]
fn exclusive_lock_is_enforced_between_processes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("pending.lock");
    let executable = std::env::current_exe().unwrap();

    let mut first = Command::new(&executable)
        .args(["--exact", "lifecycle_lock_process_helper", "--nocapture"])
        .env("AUTODOCK_LOCK_HELPER", "1")
        .env("AUTODOCK_LOCK_PATH", &path)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    let stdout = first.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();

    loop {
        line.clear();

        if reader.read_line(&mut line).unwrap() == 0 {
            let _ = first.kill();
            let _ = first.wait();
            panic!("Primeiro processo terminou sem adquirir lock");
        }

        if line.contains("LOCK_ACQUIRED") {
            break;
        }
    }

    let second = Command::new(&executable)
        .args(["--exact", "lifecycle_lock_process_helper", "--nocapture"])
        .env("AUTODOCK_LOCK_HELPER", "1")
        .env("AUTODOCK_LOCK_PATH", &path)
        .output()
        .unwrap();

    assert!(second.status.success());
    assert!(
        String::from_utf8_lossy(&second.stdout).contains("LOCK_BUSY"),
        "Segundo processo nao respeitou lock"
    );

    let _ = first.kill();
    let _ = first.wait();

    let third = Command::new(&executable)
        .args(["--exact", "lifecycle_lock_process_helper", "--nocapture"])
        .env("AUTODOCK_LOCK_HELPER", "1")
        .env("AUTODOCK_LOCK_PATH", &path)
        .output()
        .unwrap();

    assert!(third.status.success());
    assert!(
        String::from_utf8_lossy(&third.stdout).contains("LOCK_ACQUIRED"),
        "Lock nao foi liberado"
    );

    assert!(path.is_file());
}

#[cfg(windows)]
#[test]
fn normal_lock_file_has_no_reparse_attribute() {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("pending.lock");

    let _guard = LifecycleFileLock::try_acquire(&path).unwrap();
    let metadata = std::fs::metadata(&path).unwrap();

    assert_eq!(metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT, 0);
}
