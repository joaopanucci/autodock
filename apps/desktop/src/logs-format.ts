import type { LogsReport } from "./types.ts";

// Logs are plain text, never HTML or terminal instructions. Copy the displayed form.
export function logText(value: string): string {
  return value
    .replace(/\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)/g, "")
    .replace(/\x1b\[[0-?]*[ -/]*[@-~]/g, "")
    .replace(/[\x00-\x08\x0b\x0c\x0e-\x1f\x7f\u202a-\u202e\u2066-\u2069]/g, "");
}

export function logsCopy(report: LogsReport): string {
  return [
    `docker logs --tail ${report.tail} -- ${report.containerId}`,
    `Estado: ${report.status}${report.truncated ? " (saída truncada)" : ""}`,
    `[stdout]\n${logText(report.stdout) || "(sem saída padrão)"}`,
    `[stderr]\n${logText(report.stderr) || "(sem saída de erro)"}`,
    report.detail,
  ]
    .filter(Boolean)
    .join("\n\n");
}
