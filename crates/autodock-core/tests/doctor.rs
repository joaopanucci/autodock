use autodock_core::{doctor::probe, ToolStatus};
use std::time::Duration;
fn shell(script: &str, timeout: u64) -> autodock_core::doctor::ToolCheck {
    #[cfg(windows)]
    let (program, args) = (
        "powershell.exe",
        vec![
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            script,
        ],
    );
    #[cfg(not(windows))]
    let (program, args) = ("sh", vec!["-c", script]);
    probe("test", program, &args, Duration::from_millis(timeout))
}
#[test]
fn missing_is_not_execution_error() {
    assert_eq!(
        probe(
            "missing",
            "autodock-nonexistent-829c4c",
            &[],
            Duration::from_secs(1)
        )
        .status,
        ToolStatus::Missing
    );
}
#[test]
fn captures_success_and_stderr() {
    #[cfg(windows)]
    let script = "[Console]::Out.Write('out'); [Console]::Error.Write('err'); exit 0";
    #[cfg(not(windows))]
    let script = "printf out; printf err >&2; exit 0";
    let result = shell(script, 8000);
    assert_eq!(result.status, ToolStatus::Available);
    assert_eq!(result.stdout, "out");
    assert_eq!(result.stderr, "err");
}
#[test]
fn nonzero_is_execution_error() {
    let result = shell("exit 7", 8000);
    assert_eq!(result.status, ToolStatus::ExecutionError);
    assert_eq!(result.exit_code, Some(7));
}
#[test]
fn terminates_timeout() {
    #[cfg(windows)]
    let script = "Start-Sleep -Seconds 30";
    #[cfg(not(windows))]
    let script = "sleep 30";
    let result = shell(script, 200);
    assert_eq!(result.status, ToolStatus::Timeout);
    assert!(result.duration_ms < 5000);
}
#[test]
fn output_larger_than_pipe_buffer_does_not_deadlock() {
    #[cfg(windows)]
    let script = "[Console]::Out.Write(('x' * 100000)); [Console]::Error.Write(('y' * 100000))";
    #[cfg(not(windows))]
    let script = "head -c 100000 /dev/zero; head -c 100000 /dev/zero >&2";

    let result = shell(script, 8000);

    assert_eq!(result.status, ToolStatus::ExecutionError);
    assert_eq!(result.stdout.len(), 65536);
    assert_eq!(result.stderr.len(), 65536);
    assert!(result.detail.contains("Saida truncada"));
}

#[test]
fn output_exactly_at_capture_limit_is_valid() {
    #[cfg(windows)]
    let script = "[Console]::Out.Write(('x' * 65536))";
    #[cfg(not(windows))]
    let script = "head -c 65536 /dev/zero";

    let result = shell(script, 8000);

    assert_eq!(result.status, ToolStatus::Available);
    assert_eq!(result.stdout.len(), 65536);
}

#[test]
fn oversized_stderr_invalidates_success() {
    #[cfg(windows)]
    let script = "[Console]::Error.Write(('y' * 70000))";
    #[cfg(not(windows))]
    let script = "head -c 70000 /dev/zero >&2";

    let result = shell(script, 8000);

    assert_eq!(result.status, ToolStatus::ExecutionError);
    assert!(result.detail.contains("Saida truncada"));
    assert_eq!(result.stderr.len(), 65536);
}
#[test]
fn serializes_diagnostic_status() {
    let value = serde_json::to_value(probe(
        "missing",
        "autodock-nonexistent-829c4c",
        &[],
        Duration::from_secs(1),
    ))
    .unwrap();
    assert_eq!(value["status"], "missing");
    assert!(value["exitCode"].is_null());
}
