use serde::Serialize;
use serde_json::Value;
use std::time::Duration;

use crate::doctor::{probe, ToolStatus};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContainerMetrics {
    pub id: String,
    pub name: String,
    pub cpu_percent: Option<f64>,
    pub memory_usage_bytes: Option<u64>,
    pub memory_limit_bytes: Option<u64>,
    pub memory_percent: Option<f64>,
    pub network_rx_bytes: Option<u64>,
    pub network_tx_bytes: Option<u64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitoringReport {
    pub schema_version: u32,
    pub containers: Vec<ContainerMetrics>,
    pub collected_at_unix_ms: u64,
}

fn text(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

fn parse_percent(input: &str) -> Option<f64> {
    let value = input.trim().trim_end_matches('%').trim();
    let parsed = value.parse::<f64>().ok()?;

    if parsed.is_finite() && parsed >= 0.0 {
        Some(parsed)
    } else {
        None
    }
}

fn parse_bytes(input: &str) -> Option<u64> {
    let normalized = input.trim().replace(' ', "");

    if normalized.is_empty() {
        return None;
    }

    let split = normalized.find(|c: char| !c.is_ascii_digit() && c != '.')?;

    let number = normalized[..split].parse::<f64>().ok()?;
    let unit = normalized[split..].to_ascii_lowercase();

    let multiplier: f64 = match unit.as_str() {
        "b" => 1.0,
        "kb" => 1_000.0,
        "mb" => 1_000_000.0,
        "gb" => 1_000_000_000.0,
        "tb" => 1_000_000_000_000.0,
        "kib" => 1_024.0,
        "mib" => 1_048_576.0,
        "gib" => 1_073_741_824.0,
        "tib" => 1_099_511_627_776.0,
        _ => return None,
    };

    let bytes = number * multiplier;

    if !bytes.is_finite() || bytes < 0.0 || bytes > u64::MAX as f64 {
        return None;
    }

    Some(bytes.round() as u64)
}

fn parse_pair(input: &str) -> (Option<u64>, Option<u64>) {
    let Some((left, right)) = input.split_once('/') else {
        return (None, None);
    };

    (parse_bytes(left), parse_bytes(right))
}

fn parse_stats(output: &str) -> Result<MonitoringReport, String> {
    let mut containers = Vec::new();
    let mut identities = std::collections::HashSet::new();

    for (index, line) in output.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }

        let item: Value = serde_json::from_str(line)
            .map_err(|error| format!("JSON invalido na linha {}: {error}", index + 1))?;
        if !item.is_object()
            || text(&item, "ID").trim().is_empty()
            || text(&item, "Name").trim().is_empty()
            || !identities.insert(text(&item, "ID"))
        {
            return Err(tr!(
                "Métrica sem identidade válida ou duplicada na linha {}",
                "Metric without a valid or unique identity at line {}",
                index + 1
            ));
        }

        let (memory_usage_bytes, memory_limit_bytes) = parse_pair(&text(&item, "MemUsage"));

        let (network_rx_bytes, network_tx_bytes) = parse_pair(&text(&item, "NetIO"));

        containers.push(ContainerMetrics {
            id: text(&item, "ID"),
            name: text(&item, "Name"),
            cpu_percent: parse_percent(&text(&item, "CPUPerc")),
            memory_usage_bytes,
            memory_limit_bytes,
            memory_percent: parse_percent(&text(&item, "MemPerc")),
            network_rx_bytes,
            network_tx_bytes,
        });
    }

    Ok(MonitoringReport {
        schema_version: 1,
        containers,
        collected_at_unix_ms: 0,
    })
}

pub fn collect_container_metrics() -> Result<MonitoringReport, String> {
    let result = probe(
        "docker-monitoring",
        "docker",
        &["stats", "--no-stream", "--format", "{{json .}}"],
        Duration::from_secs(15),
    );

    if result.status != ToolStatus::Available {
        return Err(format!(
            "Monitoramento Docker indisponivel: {} {}",
            result.detail, result.stderr
        ));
    }

    let mut report = parse_stats(&result.stdout)?;

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_millis();

    report.collected_at_unix_ms = u64::try_from(timestamp).map_err(|error| error.to_string())?;

    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_cpu_percentage() {
        assert_eq!(parse_percent("12.50%"), Some(12.5));
        assert_eq!(parse_percent("--"), None);
        assert_eq!(parse_percent("NaN%"), None);
    }

    #[test]
    fn parses_binary_memory() {
        assert_eq!(parse_bytes("1MiB"), Some(1_048_576));
        assert_eq!(parse_bytes("2GiB"), Some(2_147_483_648));
    }

    #[test]
    fn parses_decimal_network() {
        assert_eq!(parse_bytes("1.5MB"), Some(1_500_000));
        assert_eq!(parse_bytes("300kB"), Some(300_000));
    }

    #[test]
    fn parses_docker_stats_record() {
        let input = r#"{"ID":"abc123","Name":"app-web","CPUPerc":"2.50%","MemUsage":"100MiB / 1GiB","MemPerc":"9.77%","NetIO":"1.5MB / 300kB"}"#;

        let report = parse_stats(input).unwrap();

        assert_eq!(report.containers.len(), 1);
        assert_eq!(report.containers[0].name, "app-web");
        assert_eq!(report.containers[0].cpu_percent, Some(2.5));
        assert_eq!(report.containers[0].memory_usage_bytes, Some(104_857_600));
        assert_eq!(report.containers[0].network_rx_bytes, Some(1_500_000));
    }

    #[test]
    fn accepts_empty_stats() {
        assert!(parse_stats("").unwrap().containers.is_empty());
    }

    #[test]
    fn rejects_invalid_json() {
        assert!(parse_stats("{invalid").is_err());
    }
    #[test]
    fn rejects_wrong_shapes_and_duplicate_metrics() {
        for input in ["null", "[]", "{}", r#"{"ID":"a","Name":" "}"#] {
            assert!(parse_stats(input).is_err());
        }
        assert!(
            parse_stats("{\"ID\":\"a\",\"Name\":\"a\"}\n{\"ID\":\"a\",\"Name\":\"a\"}").is_err()
        );
    }
}
