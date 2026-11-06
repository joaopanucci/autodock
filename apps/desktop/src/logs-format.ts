import type { Translate } from "./i18n/core.ts";
import type { LogsReport } from "./types.ts";

// Logs are plain text, never HTML or terminal instructions. Copy the displayed form.
export function logText(value: string): string {
  return value
    .replace(/\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)/g, "")
    .replace(/\x1b\[[0-?]*[ -/]*[@-~]/g, "")
    .replace(/[\x00-\x08\x0b\x0c\x0e-\x1f\x7f\u202a-\u202e\u2066-\u2069]/g, "");
}

export function logsCopy(report: LogsReport, t: Translate): string {
  return [
    `docker logs --tail ${report.tail} -- ${report.containerId}`,
    `${t("logs.copyHeader.state", { status: report.status })}${report.truncated ? ` ${t("logs.copyHeader.truncated")}` : ""}`,
    `[stdout]\n${logText(report.stdout) || t("logs.noStdout")}`,
    `[stderr]\n${logText(report.stderr) || t("logs.noStderr")}`,
    report.detail,
  ]
    .filter(Boolean)
    .join("\n\n");
}
