use serde::Serialize;
use std::{
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolStatus {
    Available,
    Missing,
    Timeout,
    ExecutionError,
}
impl ToolStatus {
    pub fn label(self) -> String {
        match self {
            Self::Available => tr!("Disponível", "Available"),
            Self::Missing => tr!("Ausente", "Missing"),
            Self::Timeout => tr!("Timeout", "Timeout"),
            Self::ExecutionError => tr!("Erro de execução", "Execution error"),
        }
    }
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCheck {
    pub name: String,
    pub executable: Option<String>,
    pub status: ToolStatus,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub detail: String,
    pub duration_ms: u128,
    pub truncated: bool,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DoctorReport {
    pub schema_version: u32,
    pub os: String,
    pub checks: Vec<ToolCheck>,
    pub notes: Vec<String>,
}

// Explicit discovery; Missing means not resolved, never a failed version check.
pub(crate) fn resolve(program: &str) -> Option<PathBuf> {
    which::which(program).ok().or_else(|| {
        if program == "cargo" || program == "rustc" {
            let home = std::env::var_os("CARGO_HOME")
                .map(PathBuf::from)
                .or_else(|| {
                    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
                        .map(|h| PathBuf::from(h).join(".cargo"))
                })?;
            let path = home
                .join("bin")
                .join(format!("{program}{}", std::env::consts::EXE_SUFFIX));
            path.is_file().then_some(path)
        } else if program == "docker" && !cfg!(windows) {
            // GUI apps (Finder, desktop launchers) do not inherit the shell PATH.
            [
                "/usr/local/bin/docker",
                "/opt/homebrew/bin/docker",
                "/Applications/Docker.app/Contents/Resources/bin/docker",
                "/usr/bin/docker",
            ]
            .iter()
            .map(PathBuf::from)
            .find(|path| path.is_file())
        } else if cfg!(windows) && program.eq_ignore_ascii_case("MSBuild.exe") {
            let vswhere = PathBuf::from(std::env::var_os("ProgramFiles(x86)")?)
                .join("Microsoft Visual Studio/Installer/vswhere.exe");
            if !vswhere.is_file() {
                return None;
            }
            let check = probe(
                "vswhere",
                vswhere.to_str()?,
                &[
                    "-latest",
                    "-products",
                    "*",
                    "-find",
                    "MSBuild\\**\\Bin\\MSBuild.exe",
                ],
                Duration::from_secs(3),
            );
            if check.status != ToolStatus::Available {
                return None;
            }
            check
                .stdout
                .lines()
                .map(PathBuf::from)
                .find(|p| p.is_file())
        } else {
            None
        }
    })
}

pub fn probe(name: &str, program: &str, args: &[&str], timeout: Duration) -> ToolCheck {
    let start = Instant::now();
    let mut result = ToolCheck {
        name: name.into(),
        executable: None,
        status: ToolStatus::Missing,
        exit_code: None,
        stdout: String::new(),
        stderr: String::new(),
        detail: tr!(
            "Não encontrado no PATH ou locais conhecidos; não prova ausência da instalação.",
            "Not found in PATH or known locations; this does not prove the tool is not installed."
        ),
        duration_ms: 0,
        truncated: false,
    };
    let Some(path) = resolve(program) else {
        return result;
    };
    result.executable = Some(path.display().to_string());
    result.status = ToolStatus::ExecutionError;
    result.detail.clear();
    let execution = (|| -> Result<(), String> {
        let mut command = Command::new(&path);
        #[cfg(windows)]
        {
            if path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("cmd") || e.eq_ignore_ascii_case("bat"))
            {
                // npm's JS entry point avoids cmd.exe quoting and PowerShell policy entirely.
                let script = path
                    .parent()
                    .unwrap_or(Path::new("."))
                    .join("node_modules/npm/bin/npm-cli.js");
                if name != "npm" || !script.is_file() {
                    return Err(tr!(
                        "Wrapper batch não suportado; executável nativo necessário.",
                        "Batch wrapper not supported; a native executable is required."
                    ));
                }
                let node = resolve("node").ok_or_else(|| {
                    tr!(
                        "npm encontrado, mas node não resolvido",
                        "npm found, but node was not resolved"
                    )
                })?;
                command = Command::new(node);
                command.arg(script);
            }
        }
        command.args(args).stdin(Stdio::null());
        // Files avoid pipe backpressure and inherited pipe handles blocking EOF indefinitely.
        let mut stdout = tempfile::tempfile().map_err(|e| e.to_string())?;
        let mut stderr = tempfile::tempfile().map_err(|e| e.to_string())?;
        command
            .stdout(stdout.try_clone().map_err(|e| e.to_string())?)
            .stderr(stderr.try_clone().map_err(|e| e.to_string())?);
        let mut child =
            crate::memory_capture::spawn_hidden_group(&mut command).map_err(|e| e.to_string())?;
        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    result.exit_code = status.code();
                    result.status = if status.success() {
                        ToolStatus::Available
                    } else {
                        ToolStatus::ExecutionError
                    };
                    break;
                }
                Ok(None) if start.elapsed() < timeout => {
                    std::thread::sleep(Duration::from_millis(20))
                }
                state => {
                    result.status = if state.is_err() {
                        ToolStatus::ExecutionError
                    } else {
                        ToolStatus::Timeout
                    };
                    result.detail = match state {
                        Err(e) => e.to_string(),
                        _ => tr!(
                            "Prazo de {} ms excedido",
                            "Deadline of {} ms exceeded",
                            timeout.as_millis()
                        ),
                    };
                    if let Err(e) = child.kill() {
                        result.detail.push_str(&tr!(
                            "; falha ao encerrar grupo: {e}",
                            "; failed to stop the process group: {e}"
                        ));
                    }
                    let cleanup = Instant::now();
                    while cleanup.elapsed() < Duration::from_secs(2) {
                        if !matches!(child.try_wait(), Ok(None)) {
                            break;
                        }
                        std::thread::sleep(Duration::from_millis(20));
                    }
                    break;
                }
            }
            if stdout.metadata().map_err(|e| e.to_string())?.len()
                + stderr.metadata().map_err(|e| e.to_string())?.len()
                > 2 * 1024 * 1024
            {
                let _ = child.kill();
                result.detail = tr!(
                    "Saída excedeu 2 MiB; verificação interrompida.",
                    "Output exceeded 2 MiB; check interrupted."
                );
                result.status = ToolStatus::ExecutionError;
                let _ = child.try_wait();
                break;
            }
        }
        const CAPTURE_LIMIT: u64 = 64 * 1024;

        let stdout_size = stdout.metadata().map_err(|e| e.to_string())?.len();
        let stderr_size = stderr.metadata().map_err(|e| e.to_string())?.len();

        let truncated = stdout_size > CAPTURE_LIMIT || stderr_size > CAPTURE_LIMIT;
        result.truncated = truncated;

        for (file, target) in [
            (&mut stdout, &mut result.stdout),
            (&mut stderr, &mut result.stderr),
        ] {
            file.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;

            let mut bytes = Vec::new();

            file.take(CAPTURE_LIMIT)
                .read_to_end(&mut bytes)
                .map_err(|e| e.to_string())?;

            *target = String::from_utf8_lossy(&bytes).trim().into();
        }

        if truncated {
            if result.status == ToolStatus::Available {
                result.status = ToolStatus::ExecutionError;
            }

            let message = format!(
                "Saida truncada: stdout={} bytes, stderr={} bytes; limite de captura={} bytes por fluxo.",
                stdout_size, stderr_size, CAPTURE_LIMIT
            );

            if !result.detail.is_empty() {
                result.detail.push_str("; ");
            }

            result.detail.push_str(&message);
        }

        Ok(())
    })();
    if let Err(e) = execution {
        result.status = ToolStatus::ExecutionError;
        result.detail = e;
    }
    result.duration_ms = start.elapsed().as_millis();
    result
}

pub fn doctor() -> DoctorReport {
    let mut tools: Vec<(&str, &str, Vec<&str>)> = vec![
        ("rustc", "rustc", vec!["--version"]),
        ("cargo", "cargo", vec!["--version"]),
        ("node", "node", vec!["--version"]),
        (
            "npm",
            if cfg!(windows) { "npm.cmd" } else { "npm" },
            vec!["--version"],
        ),
        ("git", "git", vec!["--version"]),
        ("docker", "docker", vec!["--version"]),
        ("compose", "docker", vec!["compose", "version"]),
        (
            "docker-engine",
            "docker",
            vec!["info", "--format", "{{.ServerVersion}}"],
        ),
    ];
    if cfg!(windows) {
        tools.push(("msbuild", "MSBuild.exe", vec!["-version", "-nologo"]));
    }
    if cfg!(target_os = "macos") {
        tools.push(("xcode-tools", "xcode-select", vec!["-p"]));
    }
    DoctorReport { schema_version: 1, os: std::env::consts::OS.into(), checks: tools.into_iter().map(|(n,p,a)| probe(n,p,&a,Duration::from_secs(8))).collect(), notes: vec![
        tr!("Checks somente leitura; Docker CLI e acesso ao Engine são resultados distintos.", "Read-only checks; the Docker CLI and Engine access are separate results."),
        tr!("MSBuild fora do PATH pode existir. O bootstrap Windows também consulta vswhere. WebView2, SDK e linker precisam de validação do build desktop.", "MSBuild may exist outside PATH. The Windows bootstrap also queries vswhere. WebView2, the SDK and the linker need validation by the desktop build."),
    ] }
}
