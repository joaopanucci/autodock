//! Starting, stopping and inspecting the compose project of a folder.
//!
//! These are the only operations that make AutoDock run something in Docker
//! beyond reading: each one needs an explicit confirmation, uses fixed arguments
//! (no shell, no text from the interface in the command line besides a validated
//! file name) and never removes volumes or images.
use crate::{
    doctor::{resolve, ToolStatus},
    generate::{self, regular_file, COMPOSE_NAMES},
    memory_capture::{self, Output},
};
use serde::{Deserialize, Serialize};
use std::{path::Path, process::Command, time::Duration};

/// Compose file AutoDock writes next to an existing one it must not touch.
const ALTERNATIVE: &str = "docker-compose.autodock.yml";
/// Building images can take a while.
const UP_TIMEOUT: Duration = Duration::from_secs(30 * 60);
const STOP_TIMEOUT: Duration = Duration::from_secs(3 * 60);
const STATUS_TIMEOUT: Duration = Duration::from_secs(20);
const OUTPUT_LINES: usize = 400;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ComposeAction {
    /// `up -d --build`: builds the images and starts every service.
    Up,
    /// `stop`: stops the containers and keeps them (and their data).
    Stop,
    /// `down`: removes the containers and the network. Volumes are kept.
    Down,
}

impl ComposeAction {
    fn arguments(self) -> &'static [&'static str] {
        match self {
            Self::Up => &["up", "-d", "--build"],
            Self::Stop => &["stop"],
            Self::Down => &["down"],
        }
    }

    fn timeout(self) -> Duration {
        match self {
            Self::Up => UP_TIMEOUT,
            Self::Stop | Self::Down => STOP_TIMEOUT,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ComposeRequest {
    /// Project folder.
    pub path: String,
    /// Compose file at the root of the project (one of the usual names, or the
    /// AutoDock alternative).
    pub file: String,
    pub action: ComposeAction,
    pub confirmed: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComposeReport {
    pub schema_version: u32,
    pub action: ComposeAction,
    /// The command that ran, for display.
    pub command: String,
    pub status: ToolStatus,
    pub exit_code: Option<i32>,
    pub succeeded: bool,
    pub output: String,
    pub detail: String,
    pub duration_ms: u128,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceState {
    pub service: String,
    pub container: String,
    /// `running`, `exited`, `created`…
    pub state: String,
    pub status: String,
    pub health: Option<String>,
    /// Published ports such as `127.0.0.1:5432->5432/tcp`.
    pub ports: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComposeStatus {
    pub schema_version: u32,
    pub services: Vec<ServiceState>,
    pub status: ToolStatus,
    pub detail: String,
}

/// Compose files of the folder that AutoDock is willing to run.
pub fn compose_files(path: &str) -> Result<Vec<String>, String> {
    let root = generate::root_of(path)?;
    Ok(COMPOSE_NAMES
        .iter()
        .copied()
        .chain([ALTERNATIVE])
        .filter(|name| regular_file(&root, name))
        .map(str::to_owned)
        .collect())
}

fn checked_file(root: &Path, file: &str) -> Result<(), String> {
    let allowed = COMPOSE_NAMES.contains(&file) || file == ALTERNATIVE;
    if !allowed {
        return Err(tr!(
            "Arquivo compose não permitido: {file}.",
            "Compose file not allowed: {file}."
        ));
    }
    if !regular_file(root, file) {
        return Err(tr!(
            "{file} não existe no projeto (ou não é um arquivo comum).",
            "{file} does not exist in the project (or is not a regular file)."
        ));
    }
    Ok(())
}

fn docker_args(file: &str, action: ComposeAction, quiet: bool) -> Vec<String> {
    let mut args: Vec<String> = vec!["compose".into(), "--ansi".into(), "never".into()];
    if quiet {
        // One line per event would exceed the capture limit of a long build.
        args.extend(["--progress".into(), "quiet".into()]);
    }
    args.extend(["-f".into(), file.into()]);
    args.extend(action.arguments().iter().map(|a| a.to_string()));
    args
}

fn display(args: &[String]) -> String {
    std::iter::once("docker")
        .chain(args.iter().map(String::as_str))
        .collect::<Vec<_>>()
        .join(" ")
}

fn run(root: &Path, args: &[String], timeout: Duration) -> Output {
    let Some(docker) = resolve("docker") else {
        return Output::missing();
    };
    let mut command = Command::new(docker);
    command.args(args).current_dir(root);
    memory_capture::run(&mut command, timeout, OUTPUT_LINES)
}

fn unknown_progress_flag(output: &Output) -> bool {
    output.stderr.contains("unknown flag: --progress")
        || output.stderr.contains("unknown flag --progress")
}

pub fn run_compose(request: ComposeRequest) -> Result<ComposeReport, String> {
    if !request.confirmed {
        return Err(tr!(
            "Confirmação explícita obrigatória.",
            "Explicit confirmation required."
        ));
    }
    let root = generate::root_of(&request.path)?;
    checked_file(&root, &request.file)?;
    let mut args = docker_args(&request.file, request.action, true);
    let mut output = run(&root, &args, request.action.timeout());
    if unknown_progress_flag(&output) {
        // Older Compose versions do not know --progress.
        args = docker_args(&request.file, request.action, false);
        output = run(&root, &args, request.action.timeout());
    }
    Ok(report(request.action, &args, output))
}

fn report(action: ComposeAction, args: &[String], output: Output) -> ComposeReport {
    let succeeded = output.status == ToolStatus::Available && output.exit_code == Some(0);
    let text = [output.stdout.trim(), output.stderr.trim()]
        .iter()
        .filter(|part| !part.is_empty())
        .copied()
        .collect::<Vec<_>>()
        .join("\n");
    let detail = match output.status {
        ToolStatus::Missing => output.detail.clone(),
        ToolStatus::Timeout => tr!(
            "Prazo excedido. O resultado é indeterminado: confira o estado dos serviços antes de repetir.",
            "Time limit exceeded. The result is undetermined: check the state of the services before repeating."
        ),
        _ if succeeded => String::new(),
        _ => tr!(
            "O Docker Compose terminou com erro. Veja a saída.",
            "Docker Compose finished with an error. See the output."
        ),
    };
    ComposeReport {
        schema_version: 1,
        action,
        command: display(args),
        status: output.status,
        exit_code: output.exit_code,
        succeeded,
        output: text,
        detail,
        duration_ms: output.duration_ms,
    }
}

/// Read-only: lists the services of the compose project and their state.
pub fn compose_status(path: &str, file: &str) -> Result<ComposeStatus, String> {
    let root = generate::root_of(path)?;
    checked_file(&root, file)?;
    let args: Vec<String> = [
        "compose", "--ansi", "never", "-f", file, "ps", "--all", "--format", "json",
    ]
    .iter()
    .map(|a| a.to_string())
    .collect();
    let output = run(&root, &args, STATUS_TIMEOUT);
    if output.status != ToolStatus::Available || output.exit_code != Some(0) {
        return Ok(ComposeStatus {
            schema_version: 1,
            services: vec![],
            status: output.status,
            detail: if output.status == ToolStatus::Missing {
                output.detail
            } else {
                tr!(
                    "Não foi possível consultar o estado dos serviços.",
                    "Could not query the state of the services."
                )
            },
        });
    }
    match parse_ps(&output.stdout) {
        Ok(services) => Ok(ComposeStatus {
            schema_version: 1,
            services,
            status: ToolStatus::Available,
            detail: String::new(),
        }),
        Err(detail) => Ok(ComposeStatus {
            schema_version: 1,
            services: vec![],
            status: ToolStatus::ExecutionError,
            detail,
        }),
    }
}

/// `docker compose ps --format json` prints one JSON object per line in recent
/// versions and a single JSON array in older ones.
fn parse_ps(text: &str) -> Result<Vec<ServiceState>, String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(vec![]);
    }
    let values: Vec<serde_json::Value> = if trimmed.starts_with('[') {
        serde_json::from_str(trimmed).map_err(|_| invalid_status())?
    } else {
        trimmed
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| serde_json::from_str(line).map_err(|_| invalid_status()))
            .collect::<Result<_, _>>()?
    };
    let text_of = |value: &serde_json::Value, key: &str| {
        value
            .get(key)
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string()
    };
    let mut services: Vec<ServiceState> = values
        .iter()
        .map(|value| {
            let health = text_of(value, "Health");
            let ports = value
                .get("Publishers")
                .and_then(|p| p.as_array())
                .map(|list| {
                    list.iter()
                        .filter(|p| {
                            p.get("PublishedPort").and_then(|n| n.as_u64()).unwrap_or(0) > 0
                        })
                        .map(|p| {
                            let url = text_of(p, "URL");
                            let published =
                                p.get("PublishedPort").and_then(|n| n.as_u64()).unwrap_or(0);
                            let target = p.get("TargetPort").and_then(|n| n.as_u64()).unwrap_or(0);
                            let protocol = text_of(p, "Protocol");
                            let host = if url.is_empty() || url == "0.0.0.0" || url == "::" {
                                String::new()
                            } else {
                                format!("{url}:")
                            };
                            format!("{host}{published}->{target}/{protocol}")
                        })
                        .collect()
                })
                .unwrap_or_default();
            ServiceState {
                service: text_of(value, "Service"),
                container: text_of(value, "Name"),
                state: text_of(value, "State"),
                status: text_of(value, "Status"),
                health: (!health.is_empty()).then_some(health),
                ports,
            }
        })
        .collect();
    for service in &mut services {
        // IPv4 and IPv6 publish the same port twice.
        let mut seen = std::collections::HashSet::new();
        service.ports.retain(|port| seen.insert(port.clone()));
    }
    services.sort_by(|a, b| a.service.cmp(&b.service));
    Ok(services)
}

fn invalid_status() -> String {
    tr!(
        "Resposta inesperada do Docker Compose.",
        "Unexpected answer from Docker Compose."
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn project(files: &[&str]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for name in files {
            fs::write(dir.path().join(name), "services: {}\n").unwrap();
        }
        dir
    }

    fn request(dir: &tempfile::TempDir, file: &str, confirmed: bool) -> ComposeRequest {
        ComposeRequest {
            path: dir.path().to_string_lossy().into(),
            file: file.into(),
            action: ComposeAction::Up,
            confirmed,
        }
    }

    #[test]
    fn nothing_runs_without_an_explicit_confirmation() {
        let dir = project(&["docker-compose.yml"]);
        let error = run_compose(request(&dir, "docker-compose.yml", false)).unwrap_err();
        assert!(error.contains("confirmation"));
    }

    #[test]
    fn only_compose_files_of_the_project_root_are_accepted() {
        let dir = project(&["docker-compose.yml", "other.yml"]);
        for bad in [
            "other.yml",
            "../docker-compose.yml",
            "sub/docker-compose.yml",
            "",
            "-f",
            "docker-compose.yml;ls",
        ] {
            assert!(
                run_compose(request(&dir, bad, true)).is_err(),
                "{bad} must be rejected"
            );
            assert!(compose_status(&dir.path().to_string_lossy(), bad).is_err());
        }
        // an allowed name that does not exist
        let missing = run_compose(request(&dir, "compose.yaml", true)).unwrap_err();
        assert!(missing.contains("does not exist"));
    }

    #[cfg(unix)]
    #[test]
    fn a_symbolic_link_is_not_accepted_as_the_compose_file() {
        let dir = project(&[]);
        let outside = project(&["real.yml"]);
        std::os::unix::fs::symlink(
            outside.path().join("real.yml"),
            dir.path().join("docker-compose.yml"),
        )
        .unwrap();
        assert!(run_compose(request(&dir, "docker-compose.yml", true)).is_err());
    }

    #[test]
    fn lists_the_compose_files_it_would_run() {
        let dir = project(&["compose.yaml", "docker-compose.autodock.yml", "notes.yml"]);
        let files = compose_files(&dir.path().to_string_lossy()).unwrap();
        assert_eq!(files, ["compose.yaml", "docker-compose.autodock.yml"]);
    }

    #[test]
    fn commands_use_fixed_arguments_and_the_validated_file_name() {
        assert_eq!(
            display(&docker_args("docker-compose.yml", ComposeAction::Up, true)),
            "docker compose --ansi never --progress quiet -f docker-compose.yml up -d --build"
        );
        assert_eq!(
            display(&docker_args("compose.yaml", ComposeAction::Stop, false)),
            "docker compose --ansi never -f compose.yaml stop"
        );
        let down = docker_args("compose.yaml", ComposeAction::Down, true).join(" ");
        assert!(down.ends_with(" down"));
        assert!(!down.contains("-v") && !down.contains("--volumes") && !down.contains("--rmi"));
    }

    #[test]
    fn parses_one_json_object_per_line() {
        let text = r#"{"Name":"shop-api","Service":"api","State":"running","Status":"Up 5 seconds (healthy)","Health":"healthy","Publishers":[{"URL":"0.0.0.0","TargetPort":8080,"PublishedPort":8080,"Protocol":"tcp"},{"URL":"::","TargetPort":8080,"PublishedPort":8080,"Protocol":"tcp"}]}
{"Name":"shop-db","Service":"postgres","State":"running","Status":"Up 8 seconds","Health":"","Publishers":[{"URL":"127.0.0.1","TargetPort":5432,"PublishedPort":5433,"Protocol":"tcp"}]}
{"Name":"shop-job","Service":"job","State":"exited","Status":"Exited (0) 1 minute ago","Publishers":[]}
"#;
        let services = parse_ps(text).unwrap();
        assert_eq!(services.len(), 3);
        assert_eq!(services[0].service, "api");
        assert_eq!(services[0].health.as_deref(), Some("healthy"));
        assert_eq!(
            services[0].ports,
            ["8080->8080/tcp"],
            "IPv4 and IPv6 duplicates are merged"
        );
        assert_eq!(services[1].service, "job");
        assert_eq!(
            (services[1].state.as_str(), services[1].health.clone()),
            ("exited", None)
        );
        assert_eq!(services[2].ports, ["127.0.0.1:5433->5432/tcp"]);
    }

    #[test]
    fn parses_the_array_format_of_older_compose_versions_and_empty_output() {
        let array =
            r#"[{"Name":"a","Service":"b","State":"running","Status":"Up","Publishers":null}]"#;
        let services = parse_ps(array).unwrap();
        assert_eq!(services.len(), 1);
        assert!(services[0].ports.is_empty());
        assert!(parse_ps("").unwrap().is_empty());
        assert!(parse_ps("   \n").unwrap().is_empty());
        assert!(parse_ps("not json").is_err());
        assert!(parse_ps("[1,").is_err());
    }

    #[test]
    fn reports_distinguish_success_failure_timeout_and_missing_docker() {
        let args = docker_args("compose.yaml", ComposeAction::Up, true);
        let output = |status, exit_code: Option<i32>, stdout: &str, stderr: &str| Output {
            status,
            exit_code,
            stdout: stdout.into(),
            stderr: stderr.into(),
            truncated: false,
            detail: "no docker".into(),
            duration_ms: 5,
        };
        let ok = report(
            ComposeAction::Up,
            &args,
            output(ToolStatus::Available, Some(0), "started\n", ""),
        );
        assert!(ok.succeeded && ok.detail.is_empty() && ok.output == "started");
        let failed = report(
            ComposeAction::Up,
            &args,
            output(ToolStatus::Available, Some(1), "", "build failed\n"),
        );
        assert!(!failed.succeeded && failed.output == "build failed" && !failed.detail.is_empty());
        let timeout = report(
            ComposeAction::Up,
            &args,
            output(ToolStatus::Timeout, None, "", ""),
        );
        assert!(!timeout.succeeded && timeout.detail.contains("undetermined"));
        let missing = report(
            ComposeAction::Up,
            &args,
            output(ToolStatus::Missing, None, "", ""),
        );
        assert!(!missing.succeeded && missing.detail == "no docker");
        assert!(unknown_progress_flag(&output(
            ToolStatus::Available,
            Some(1),
            "",
            "unknown flag: --progress"
        )));
        assert!(!unknown_progress_flag(&output(
            ToolStatus::Available,
            Some(1),
            "",
            "network error"
        )));
    }
}
