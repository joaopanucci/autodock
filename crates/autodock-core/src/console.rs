use crate::doctor::{probe, ToolStatus};
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConsoleOperation {
    Containers,
    EngineInfo,
    DockerVersion,
}

impl ConsoleOperation {
    fn arguments(self) -> &'static [&'static str] {
        match self {
            Self::Containers => &["ps", "-a", "--no-trunc", "--format", "{{json .}}"],
            Self::EngineInfo => &["info"],
            Self::DockerVersion => &["version"],
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Containers => "Listar containers",
            Self::EngineInfo => "Informações do Docker Engine",
            Self::DockerVersion => "Versão do Docker",
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsoleReport {
    pub schema_version: u32,
    pub operation: ConsoleOperation,
    pub label: String,
    pub command: String,
    pub status: ToolStatus,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub detail: String,
    pub duration_ms: u128,
    pub truncated: bool,
}

pub fn execute_console_operation(operation: ConsoleOperation) -> ConsoleReport {
    let args = operation.arguments();

    let result = probe("autodock-console", "docker", args, Duration::from_secs(15));

    ConsoleReport {
        schema_version: 1,
        operation,
        label: operation.label().into(),
        command: format!("docker {}", args.join(" ")),
        status: result.status,
        exit_code: result.exit_code,
        stdout: result.stdout,
        stderr: result.stderr,
        detail: result.detail,
        duration_ms: result.duration_ms,
        truncated: result.truncated,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operations_have_fixed_arguments() {
        assert_eq!(
            ConsoleOperation::Containers.arguments(),
            &["ps", "-a", "--no-trunc", "--format", "{{json .}}"]
        );

        assert_eq!(ConsoleOperation::EngineInfo.arguments(), &["info"]);

        assert_eq!(ConsoleOperation::DockerVersion.arguments(), &["version"]);
    }

    #[test]
    fn rejects_unknown_operation() {
        let parsed = serde_json::from_str::<ConsoleOperation>(r#""container_delete""#);

        assert!(parsed.is_err());
    }

    #[test]
    fn rejects_arbitrary_command() {
        let parsed = serde_json::from_str::<ConsoleOperation>(r#""docker system prune""#);

        assert!(parsed.is_err());
    }

    #[test]
    fn serializes_operation_contract() {
        let value = serde_json::to_value(ConsoleOperation::EngineInfo).expect("serializable");

        assert_eq!(value, "engine_info");
    }

    #[test]
    fn all_operations_are_read_only_queries() {
        for operation in [
            ConsoleOperation::Containers,
            ConsoleOperation::EngineInfo,
            ConsoleOperation::DockerVersion,
        ] {
            let args = operation.arguments();

            assert!(!args.is_empty());

            assert!(matches!(args[0], "ps" | "info" | "version"));
        }
    }
}
