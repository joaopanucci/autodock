use autodock_core::lifecycle_lock::LifecycleFileLock;
use std::{
    io::{self, BufRead, BufReader, Read, Write},
    process::{Command, Stdio},
    thread,
    time::Duration,
};

// Every assertion/error path reaps helpers. A stalled startup must fail the test,
// rather than holding the CI runner indefinitely in a blocking read_line.
struct Child(std::process::Child);
impl Drop for Child {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn wait_for_marker(
    reader: impl Read + Send + 'static,
    marker: &'static str,
    timeout: Duration,
) -> io::Result<()> {
    let (sender, receiver) = std::sync::mpsc::channel();
    thread::spawn(move || {
        let result = (|| {
            for line in BufReader::new(reader).lines() {
                if line?.contains(marker) {
                    return Ok(());
                }
            }
            Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Helper terminou antes do handshake",
            ))
        })();
        let _ = sender.send(result);
    });
    receiver.recv_timeout(timeout).map_err(|error| {
        io::Error::new(
            if error == std::sync::mpsc::RecvTimeoutError::Timeout {
                io::ErrorKind::TimedOut
            } else {
                io::ErrorKind::BrokenPipe
            },
            format!("Handshake do helper indisponivel: {error}"),
        )
    })?
}

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
            if std::env::var_os("AUTODOCK_LOCK_HOLD").is_some() {
                let mut release = String::new();
                std::io::stdin().read_line(&mut release).unwrap();
            }
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
    let dir = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
    let path = dir.path().join("pending.lock");
    let executable = std::env::current_exe().unwrap();

    let mut first = Child(
        Command::new(&executable)
            .args(["--exact", "lifecycle_lock_process_helper", "--nocapture"])
            .env("AUTODOCK_LOCK_HELPER", "1")
            .env("AUTODOCK_LOCK_HOLD", "1")
            .env("AUTODOCK_LOCK_PATH", &path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    wait_for_marker(
        first.0.stdout.take().unwrap(),
        "LOCK_ACQUIRED",
        Duration::from_secs(15),
    )
    .unwrap();

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

    drop(first);

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

    let dir = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
    let path = dir.path().join("pending.lock");

    let _guard = LifecycleFileLock::try_acquire(&path).unwrap();
    let metadata = std::fs::metadata(&path).unwrap();

    assert_eq!(metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT, 0);
}

#[test]
fn lifecycle_journal_process_helper() {
    use autodock_core::container_lifecycle::*;
    let Some(mode) = std::env::var_os("AUTODOCK_JOURNAL_HELPER") else {
        return;
    };
    let directory = std::env::var_os("AUTODOCK_JOURNAL_PATH").unwrap().into();
    let service = LifecycleService::new(directory, "default".into()).unwrap();
    struct Executor {
        hold: bool,
    }
    impl LifecycleExecutor for Executor {
        fn inspect(&mut self, _: &str, _: &str) -> Result<ContainerObservation, String> {
            Ok(ContainerObservation {
                container_id: "a".repeat(64),
                state: "running".into(),
            })
        }
        fn execute(&mut self, _: &str, _: &[String]) -> LifecycleOutput {
            if self.hold {
                println!("RESERVATION_DURABLE");
                std::io::stdout().flush().unwrap();
                loop {
                    thread::park();
                }
            }
            panic!("A pending operation must never be replayed")
        }
    }
    let mut executor = Executor {
        hold: mode == "hold",
    };
    if mode == "reconcile" {
        assert_eq!(service.pending().unwrap().len(), 1);
        assert!(
            service
                .reconcile(
                    ReconciliationRequest {
                        container_id: "a".repeat(64),
                        acknowledged: true
                    },
                    &mut executor
                )
                .unwrap()
                .released
        );
        assert!(service.pending().unwrap().is_empty());
        println!("RECONCILED");
    } else {
        let error = service
            .execute(
                LifecycleRequest {
                    container_id: "A".repeat(12),
                    action: LifecycleAction::Restart,
                    confirmed: true,
                },
                &mut executor,
            )
            .unwrap_err();
        assert!(
            error.contains("ocupado") || error.contains("reconciliacao"),
            "{error}"
        );
        println!("EXECUTION_BLOCKED");
    }
}

#[test]
fn durable_reservation_survives_process_death_and_requires_reconciliation() {
    let dir = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
    let path = dir.path().join("pending");
    let executable = std::env::current_exe().unwrap();
    let command = |mode: &str| {
        let mut command = Command::new(&executable);
        command
            .args(["--exact", "lifecycle_journal_process_helper", "--nocapture"])
            .env("AUTODOCK_JOURNAL_HELPER", mode)
            .env("AUTODOCK_JOURNAL_PATH", &path);
        command
    };
    let mut child = Child(command("hold").stdout(Stdio::piped()).spawn().unwrap());
    wait_for_marker(
        child.0.stdout.take().unwrap(),
        "RESERVATION_DURABLE",
        Duration::from_secs(15),
    )
    .unwrap();
    let active = command("try").output().unwrap();
    assert!(
        active.status.success(),
        "{}",
        String::from_utf8_lossy(&active.stderr)
    );
    assert!(String::from_utf8_lossy(&active.stdout).contains("EXECUTION_BLOCKED"));
    drop(child); // abrupt interruption after reservation and before any result
    let recovered = command("try").output().unwrap();
    assert!(
        recovered.status.success(),
        "{}",
        String::from_utf8_lossy(&recovered.stderr)
    );
    assert!(String::from_utf8_lossy(&recovered.stdout).contains("EXECUTION_BLOCKED"));
    let reconciled = command("reconcile").output().unwrap();
    assert!(
        reconciled.status.success(),
        "{}",
        String::from_utf8_lossy(&reconciled.stderr)
    );
    assert!(String::from_utf8_lossy(&reconciled.stdout).contains("RECONCILED"));
}

#[test]
fn subprocess_handshake_has_deadline_and_detects_early_exit() {
    struct Stalled(std::sync::mpsc::Receiver<()>);
    impl Read for Stalled {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            let _ = self.0.recv();
            Ok(0)
        }
    }
    let (release, stalled) = std::sync::mpsc::channel();
    let error = wait_for_marker(Stalled(stalled), "READY", Duration::from_millis(20)).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    drop(release); // let the reader thread finish
    let error = wait_for_marker(
        io::Cursor::new(Vec::<u8>::new()),
        "READY",
        Duration::from_secs(1),
    )
    .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::UnexpectedEof);
}
