//! Sensitive output capture. No files, logging, or diagnostic-report integration.
//! Uses the same command-group Jobs/process groups as doctor, with bounded drains.
use crate::doctor::ToolStatus;
use command_group::CommandGroup;
use std::{
    io::Read,
    process::{Command, Stdio},
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

pub(crate) const STREAM_LIMIT: usize = 128 * 1024;

#[derive(Default)]
struct Capture {
    bytes: Vec<u8>,
    lines: usize,
    truncated: bool,
    done: bool,
    failed: bool,
}

fn drain(
    mut pipe: impl Read + Send + 'static,
    lines: usize,
) -> (Arc<Mutex<Capture>>, thread::JoinHandle<()>) {
    let shared = Arc::new(Mutex::new(Capture::default()));
    let reader = Arc::clone(&shared);
    let handle = thread::spawn(move || {
        let mut buffer = [0u8; 8192];
        loop {
            match pipe.read(&mut buffer) {
                Ok(0) => {
                    reader.lock().unwrap().done = true;
                    break;
                }
                Ok(count) => {
                    let mut capture = reader.lock().unwrap();
                    for &byte in &buffer[..count] {
                        if capture.bytes.len() >= STREAM_LIMIT || capture.lines >= lines {
                            capture.truncated = true;
                        } else {
                            capture.bytes.push(byte);
                            if byte == b'\n' {
                                capture.lines += 1;
                            }
                        }
                    }
                    // Continue draining, but never accumulate beyond the cap.
                }
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(_) => {
                    let mut capture = reader.lock().unwrap();
                    capture.failed = true;
                    capture.done = true;
                    break;
                }
            }
        }
    });
    (shared, handle)
}

pub(crate) struct Output {
    pub status: ToolStatus,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub truncated: bool,
    pub detail: String,
    pub duration_ms: u128,
}

impl Output {
    pub fn missing() -> Self {
        Self {
            status: ToolStatus::Missing,
            exit_code: None,
            stdout: String::new(),
            stderr: String::new(),
            truncated: false,
            detail: tr!(
                "Docker não resolvido no PATH; a instalação pode existir.",
                "Docker was not resolved in PATH; the installation may exist."
            ),
            duration_ms: 0,
        }
    }
}

pub(crate) fn run(command: &mut Command, timeout: Duration, lines: usize) -> Output {
    let started = Instant::now();
    let mut result = Output::missing();
    result.status = ToolStatus::ExecutionError;
    result.detail.clear();
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let mut child = match command.group_spawn() {
        Ok(child) => child,
        Err(_) => {
            result.detail = tr!(
                "Não foi possível iniciar a consulta de logs.",
                "Could not start the logs query."
            );
            result.duration_ms = started.elapsed().as_millis();
            return result;
        }
    };
    // Take handles before calling group wait methods, as required by command-group.
    let stdout_pipe = child.inner().stdout.take().expect("piped stdout");
    let stderr_pipe = child.inner().stderr.take().expect("piped stderr");
    let (stdout, out_thread) = drain(stdout_pipe, lines);
    let (stderr, err_thread) = drain(stderr_pipe, lines);
    let mut exit = None;
    let cancel = loop {
        if exit.is_none() {
            match child.try_wait() {
                Ok(status) => exit = status,
                Err(_) => {
                    result.detail = tr!(
                        "Falha ao consultar estado do processo.",
                        "Failed to query the process state."
                    );
                    break true;
                }
            }
        }
        let out = stdout.lock().unwrap();
        let err = stderr.lock().unwrap();
        if out.failed || err.failed {
            result.detail = tr!(
                "Falha ao capturar a saída do processo.",
                "Failed to capture the process output."
            );
            break true;
        }
        if out.truncated || err.truncated {
            result.truncated = true;
            result.detail = tr!("Saída truncada: limite de linhas ou 128 KiB por fluxo atingido. Apenas o prefixo capturado é exibido.", "Output truncated: line limit or 128 KiB per stream reached. Only the captured prefix is shown.");
            break true;
        }
        if let Some(status) = exit {
            if out.done && err.done {
                result.status = if status.success() {
                    ToolStatus::Available
                } else {
                    ToolStatus::ExecutionError
                };
                result.exit_code = status.code();
                break false;
            }
        }
        if started.elapsed() >= timeout {
            result.status = ToolStatus::Timeout;
            result.detail = tr!(
                "Consulta excedeu o prazo de execução/captura; saída pode ser parcial.",
                "The query exceeded the execution/capture deadline; output may be partial."
            );
            break true;
        }
        drop(out);
        drop(err);
        thread::sleep(Duration::from_millis(10));
    };
    if cancel {
        let kill_failed = child.kill().is_err();
        let cleanup = Instant::now();
        let mut process_exited = false;
        let mut readers_finished = false;
        let mut cleanup_error = false;

        while cleanup.elapsed() < Duration::from_secs(2) {
            match child.try_wait() {
                Ok(Some(status)) => {
                    process_exited = true;
                    result.exit_code = status.code();
                }
                Ok(None) => {}
                Err(_) => {
                    cleanup_error = true;
                    break;
                }
            }

            readers_finished = out_thread.is_finished() && err_thread.is_finished();

            if process_exited && readers_finished {
                break;
            }

            thread::sleep(Duration::from_millis(10));
        }

        if kill_failed || cleanup_error || !process_exited || !readers_finished {
            result.detail.push_str(&tr!(
                " Atenção: não foi possível confirmar a limpeza completa do processo e de seus leitores.",
                " Warning: could not confirm full cleanup of the process and its readers."
            ));
        }

        if result.exit_code.is_none() {
            result.exit_code = exit.and_then(|status| status.code());
        }
    }
    // Never join a blocked reader after the deadline. Snapshot bounded buffers only.
    if out_thread.is_finished() {
        let _ = out_thread.join();
    }
    if err_thread.is_finished() {
        let _ = err_thread.join();
    }
    for (capture, target) in [(&stdout, &mut result.stdout), (&stderr, &mut result.stderr)] {
        let capture = capture.lock().unwrap();
        result.truncated |= capture.truncated;
        *target = String::from_utf8_lossy(&capture.bytes).into_owned();
    }
    result.duration_ms = started.elapsed().as_millis();
    result
}
