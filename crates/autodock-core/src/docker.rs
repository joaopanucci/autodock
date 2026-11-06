use serde::Serialize;
use serde_json::Value;
use std::time::Duration;

use crate::doctor::{probe, ToolStatus};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DockerContainer {
    pub id: String,
    pub name: String,
    pub image: String,
    pub state: String,
    pub status: String,
    pub ports: String,
    pub project: Option<String>,
    pub service: Option<String>,
    pub health: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DockerContainersReport {
    pub schema_version: u32,
    pub containers: Vec<DockerContainer>,
}

fn label(labels: &str, key: &str) -> Option<String> {
    labels
        .split(',')
        .find_map(|entry| entry.strip_prefix(&format!("{key}=")))
        .map(str::to_owned)
}

fn field(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

fn parse_containers(output: &str) -> Result<DockerContainersReport, String> {
    let mut containers = Vec::new();
    let mut identities = std::collections::HashSet::new();

    for (index, line) in output.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }

        let item: Value = serde_json::from_str(line)
            .map_err(|e| format!("Resposta Docker invalida na linha {}: {e}", index + 1))?;
        if !item.is_object()
            || field(&item, "ID").trim().is_empty()
            || field(&item, "Names").trim().is_empty()
            || !identities.insert(field(&item, "ID"))
        {
            return Err(tr!(
                "Resposta Docker sem identidade válida ou duplicada na linha {}",
                "Docker answer without a valid or unique identity at line {}",
                index + 1
            ));
        }

        let labels = field(&item, "Labels");
        let status = field(&item, "Status");

        let health = if status.contains("(healthy)") {
            Some("healthy".to_owned())
        } else if status.contains("(unhealthy)") {
            Some("unhealthy".to_owned())
        } else if status.contains("(health: starting)") {
            Some("starting".to_owned())
        } else {
            None
        };

        containers.push(DockerContainer {
            id: field(&item, "ID"),
            name: field(&item, "Names"),
            image: field(&item, "Image"),
            state: field(&item, "State"),
            status,
            ports: field(&item, "Ports"),
            project: label(&labels, "com.docker.compose.project"),
            service: label(&labels, "com.docker.compose.service"),
            health,
        });
    }

    Ok(DockerContainersReport {
        schema_version: 1,
        containers,
    })
}

pub fn list_docker_containers() -> Result<DockerContainersReport, String> {
    let result = probe(
        "docker-containers",
        "docker",
        &["ps", "-a", "--no-trunc", "--format", "{{json .}}"],
        Duration::from_secs(10),
    );

    if result.status != ToolStatus::Available {
        return Err(tr!(
            "Consulta Docker indisponível: {} {}",
            "Docker query unavailable: {} {}",
            result.detail,
            result.stderr
        ));
    }

    parse_containers(&result.stdout)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_compose_container() {
        let input = r#"{"ID":"abc123","Names":"app-web-1","Image":"nginx:latest","State":"running","Status":"Up 1 hour (healthy)","Ports":"127.0.0.1:8080->80/tcp","Labels":"com.docker.compose.project=app,com.docker.compose.service=web"}"#;

        let report = parse_containers(input).unwrap();
        assert_eq!(report.containers.len(), 1);
        assert_eq!(report.containers[0].project.as_deref(), Some("app"));
        assert_eq!(report.containers[0].service.as_deref(), Some("web"));
        assert_eq!(report.containers[0].health.as_deref(), Some("healthy"));
    }

    #[test]
    fn accepts_empty_list() {
        let report = parse_containers("").unwrap();
        assert!(report.containers.is_empty());
    }

    #[test]
    fn rejects_invalid_json() {
        assert!(parse_containers("{invalid").is_err());
    }
    #[test]
    fn rejects_wrong_shapes_empty_and_duplicate_identities() {
        for input in [
            "[]",
            "null",
            "{}",
            r#"{"ID":"","Names":"a"}"#,
            r#"{"ID":1,"Names":"a"}"#,
        ] {
            assert!(parse_containers(input).is_err());
        }
        assert!(parse_containers(
            "{\"ID\":\"abc\",\"Names\":\"a\"}\n{\"ID\":\"abc\",\"Names\":\"a\"}"
        )
        .is_err());
    }

    #[test]
    fn supports_standalone_container() {
        let input = r#"{"ID":"xyz","Names":"standalone","Image":"redis","State":"exited","Status":"Exited (0)","Ports":"","Labels":""}"#;
        let report = parse_containers(input).unwrap();
        assert!(report.containers[0].project.is_none());
    }
}
