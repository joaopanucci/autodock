use crate::{
    doctor::{resolve, ToolStatus},
    memory_capture::{self, Output},
};
use serde::{Deserialize, Serialize};
use std::{process::Command, time::Duration};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LogsRequest {
    pub container_id: String,
    pub tail: u16,
}

impl LogsRequest {
    fn arguments(&self) -> Result<Vec<String>, String> {
        // Accept only Docker IDs, never names, switches, shell fragments or paths.
        if !(12..=64).contains(&self.container_id.len())
            || !self.container_id.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err("Identificador inválido: selecione um container com ID hexadecimal de 12 a 64 caracteres.".into());
        }
        if ![100, 300, 500].contains(&self.tail) {
            return Err("Limite inválido: escolha 100, 300 ou 500 linhas.".into());
        }
        Ok(vec![
            "logs".into(),
            "--tail".into(),
            self.tail.to_string(),
            "--".into(),
            self.container_id.clone(),
        ])
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogsReport {
    pub schema_version: u32,
    pub container_id: String,
    pub tail: u16,
    pub status: ToolStatus,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub truncated: bool,
    pub detail: String,
    pub duration_ms: u128,
}

// No diagnostic reports, disk capture or Console history receives this output.
pub fn query_container_logs(request: LogsRequest) -> Result<LogsReport, String> {
    query_with(request, |args, lines| {
        let Some(docker) = resolve("docker") else {
            return Output::missing();
        };
        memory_capture::run(
            Command::new(docker).args(args),
            Duration::from_secs(15),
            lines,
        )
    })
}

fn query_with(
    request: LogsRequest,
    execute: impl FnOnce(&[String], usize) -> Output,
) -> Result<LogsReport, String> {
    let args = request.arguments()?;
    let output = execute(&args, request.tail as usize);
    Ok(LogsReport {
        schema_version: 1,
        container_id: request.container_id,
        tail: request.tail,
        status: output.status,
        exit_code: output.exit_code,
        stdout: output.stdout,
        stderr: output.stderr,
        truncated: output.truncated,
        detail: output.detail,
        duration_ms: output.duration_ms,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request() -> LogsRequest {
        LogsRequest {
            container_id: "abcdef012345".into(),
            tail: 100,
        }
    }
    #[test]
    fn only_logs_with_controlled_arguments() {
        for tail in [100, 300, 500] {
            let r = LogsRequest { tail, ..request() };
            assert_eq!(
                r.arguments().unwrap(),
                ["logs", "--tail", &tail.to_string(), "--", "abcdef012345"]
            );
        }
    }
    #[test]
    fn rejects_ids_and_limits_before_execution() {
        for id in [
            "",
            "abc",
            "--follow",
            "name",
            "abcdef012345;stop",
            "../abcdef012345",
            "abcdef012345\n",
            &"a".repeat(65),
        ] {
            let r = LogsRequest {
                container_id: id.into(),
                ..request()
            };
            assert!(query_with(r, |_, _| panic!("must not execute")).is_err());
        }
        for tail in [0, 99, 101, 501, u16::MAX] {
            assert!(query_with(LogsRequest { tail, ..request() }, |_, _| panic!(
                "must not execute"
            ))
            .is_err());
        }
    }
    #[test]
    fn rejects_arbitrary_operations_and_extra_fields() {
        for command in ["exec", "stop", "restart", "rm", "prune", "--follow"] {
            let json =
                serde_json::json!({"containerId":"abcdef012345", "tail":100, "command":command});
            assert!(serde_json::from_value::<LogsRequest>(json).is_err());
        }
    }
    #[test]
    fn preserves_empty_timeout_failure_and_missing_states() {
        for status in [
            ToolStatus::Available,
            ToolStatus::Timeout,
            ToolStatus::ExecutionError,
            ToolStatus::Missing,
        ] {
            let report = query_with(request(), |_, _| {
                let mut output = Output::missing();
                output.status = status;
                output
            })
            .unwrap();
            assert_eq!(report.status, status);
            assert!(report.stdout.is_empty());
            assert!(!report.truncated);
            assert_eq!(
                serde_json::to_value(report).unwrap()["containerId"],
                "abcdef012345"
            );
        }
    }
    fn shell(script: &str, millis: u64, lines: usize) -> Output {
        #[cfg(windows)]
        let mut command = {
            let mut c = Command::new("powershell.exe");
            c.args(["-NoProfile", "-NonInteractive", "-Command", script]);
            c
        };
        #[cfg(not(windows))]
        let mut command = {
            let mut c = Command::new("sh");
            c.args(["-c", script]);
            c
        };
        memory_capture::run(&mut command, Duration::from_millis(millis), lines)
    }
    #[test]
    fn captures_memory_streams_and_empty_output() {
        #[cfg(windows)]
        let script = "[Console]::Out.Write('out'); [Console]::Error.Write('err')";
        #[cfg(not(windows))]
        let script = "printf out; printf err >&2";
        let result = shell(script, 8000, 100);
        assert_eq!(result.status, ToolStatus::Available);
        assert_eq!(result.stdout, "out");
        assert_eq!(result.stderr, "err");
        assert_eq!(shell("exit 0", 8000, 100).stdout, "");
        assert_eq!(shell("exit 7", 8000, 100).exit_code, Some(7));
    }
    #[test]
    fn bounds_timeout_and_spawn_failure() {
        #[cfg(windows)]
        let script = "Start-Sleep -Seconds 30";
        #[cfg(not(windows))]
        let script = "sleep 30";
        let result = shell(script, 250, 100);
        assert_eq!(result.status, ToolStatus::Timeout);
        assert!(result.duration_ms < 4000);
        let failed = memory_capture::run(
            &mut Command::new("autodock-invalid-executable-9876"),
            Duration::from_secs(1),
            100,
        );
        assert_eq!(failed.status, ToolStatus::ExecutionError);
    }

    #[cfg(windows)]
    #[test]
    fn timeout_terminates_descendant_without_disk_pid_file() {
        let result = shell("$p = Start-Process powershell.exe -ArgumentList '-NoProfile -NonInteractive -Command Start-Sleep -Seconds 30' -WindowStyle Hidden -PassThru; [Console]::WriteLine($p.Id); Start-Sleep -Seconds 30", 1800, 100);
        assert_eq!(result.status, ToolStatus::Timeout);
        let pid = result.stdout.trim().parse::<u32>().expect("test child PID");
        let check = shell(&format!("if (Get-Process -Id {pid} -ErrorAction SilentlyContinue) {{ exit 1 }} else {{ exit 0 }}"), 8000, 100);
        assert_eq!(check.exit_code, Some(0));
    }

    #[test]
    fn exact_capture_limit_is_valid_and_timeout_retains_partial_output() {
        #[cfg(windows)]
        let script = "[Console]::Out.Write(('x' * 131072))";
        #[cfg(not(windows))]
        let script = "head -c 131072 /dev/zero";
        let result = shell(script, 8000, 100);
        assert_eq!(result.status, ToolStatus::Available);
        assert!(!result.truncated);
        assert_eq!(result.stdout.len(), memory_capture::STREAM_LIMIT);
        #[cfg(windows)]
        let script = "[Console]::Write('partial'); Start-Sleep -Seconds 30";
        #[cfg(not(windows))]
        let script = "printf partial; sleep 30";
        let result = shell(script, 1500, 100);
        assert_eq!(result.status, ToolStatus::Timeout);
        assert_eq!(result.stdout, "partial");
    }
    #[test]
    fn caps_large_output_and_excess_lines_without_deadlock() {
        #[cfg(windows)]
        let script = "[Console]::Out.Write(('x' * 200000)); [Console]::Error.Write(('y' * 200000))";
        #[cfg(not(windows))]
        let script = "head -c 200000 /dev/zero; head -c 200000 /dev/zero >&2";
        let result = shell(script, 8000, 100);
        assert!(result.truncated);
        assert_eq!(result.status, ToolStatus::ExecutionError);
        assert!(
            result.stdout.len() <= memory_capture::STREAM_LIMIT
                && result.stderr.len() <= memory_capture::STREAM_LIMIT
        );
        #[cfg(windows)]
        let lines = "1..500 | ForEach-Object { [Console]::WriteLine('line') }";
        #[cfg(not(windows))]
        let lines = "i=0; while [ $i -lt 500 ]; do echo line; i=$((i+1)); done";
        let result = shell(lines, 8000, 100);
        assert!(result.truncated);
        assert_eq!(result.stdout.lines().count(), 100);
    }
}
