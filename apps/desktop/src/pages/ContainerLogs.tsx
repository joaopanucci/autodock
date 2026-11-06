import { useEffect, useRef, useState } from "react";
import { call } from "../ipc";
import {
  AlertTriangle,
  Copy,
  FileText,
  LoaderCircle,
  RefreshCw,
  ShieldCheck,
  Trash2,
} from "lucide-react";
import { Badge, EmptyState, Panel } from "../components/ui";
import { useI18n } from "../i18n";
import { logsCopy, logText } from "../logs-format";
import type {
  DockerContainer,
  DockerContainersReport,
  LogsReport,
} from "../types";

export function ContainerLogs() {
  const { t } = useI18n();
  const [containers, setContainers] = useState<DockerContainer[] | null>(null);
  const [id, setId] = useState("");
  const [tail, setTail] = useState<100 | 300 | 500>(100);
  const [result, setResult] = useState<LogsReport | null>(null);
  const [busy, setBusy] = useState<"list" | "logs" | null>(null);
  const [error, setError] = useState("");
  const [copied, setCopied] = useState(false);
  const inFlight = useRef(false);
  const mounted = useRef(true);
  const generation = useRef(0);

  useEffect(() => {
    mounted.current = true;

    return () => {
      mounted.current = false;
      generation.current += 1;
    };
  }, []);

  const selected = containers?.find((c) => c.id === id);
  function clearResult() {
    setResult(null);
    setCopied(false);
    setError("");
  }
  async function run(
    kind: "list" | "logs",
    task: (isCurrent: () => boolean) => Promise<void>,
  ) {
    if (inFlight.current || !mounted.current) return;

    inFlight.current = true;
    const token = ++generation.current;
    const isCurrent = () =>
      mounted.current && generation.current === token;

    setBusy(kind);
    setError("");
    setCopied(false);

    try {
      await task(isCurrent);
    } catch (cause) {
      if (isCurrent()) {
        setError(cause instanceof Error ? cause.message : String(cause));
      }
    } finally {
      inFlight.current = false;
      if (isCurrent()) {
        setBusy(null);
      }
    }
  }
  function refreshContainers() {
    void run("list", async (isCurrent) => {
      const report = await call<DockerContainersReport>(
        "list_docker_containers",
      );
      if (!isCurrent()) return;
      setContainers(report.containers);
      setId((current) =>
        report.containers.some((c) => c.id === current)
          ? current
          : (report.containers[0]?.id ?? ""),
      );
      setResult(null);
    });
  }
  function query() {
    if (!selected) return;
    void run("logs", async (isCurrent) => {
      setResult(null);
      const report = await call<LogsReport>("query_container_logs", {
        request: { containerId: selected.id, tail },
      });
      if (!isCurrent()) return;
      if (
        report.schemaVersion !== 1 ||
        report.containerId !== selected.id ||
        report.tail !== tail
      )
        throw new Error(t("logs.error.mismatch"));
      setResult(report);
    });
  }
  async function copy() {
    if (!result) return;
    try {
      await navigator.clipboard.writeText(logsCopy(result, t));
      if (mounted.current) setCopied(true);
    } catch {
      if (!mounted.current) return;
      setError(t("logs.error.copy"));
    }
  }
  return (
    <div className="container-logs">
      <Panel
        title={t("logs.title")}
        eyebrow={t("logs.eyebrow")}
        action={
          <button
            className="button secondary"
            disabled={!!busy}
            onClick={refreshContainers}
          >
            <RefreshCw size={16} aria-hidden="true" />
            {containers ? t("logs.refreshList") : t("logs.loadContainers")}
          </button>
        }
      >
        <p className="section-description">
          {t("logs.intro")}
        </p>
        {containers === null ? (
          <EmptyState
            icon={FileText}
            title={t("logs.empty.title")}
            description={t("logs.empty.description")}
          />
        ) : !containers.length ? (
          <p role="status">
            {t("logs.noContainers")}
          </p>
        ) : (
          <>
            <div className="logs-controls">
              <div className="logs-field">
                <label htmlFor="logs-container">{t("logs.container")}</label>
                <select
                  id="logs-container"
                  value={id}
                  disabled={!!busy}
                  onChange={(e) => {
                    setId(e.target.value);
                    clearResult();
                  }}
                >
                  {containers.map((c) => (
                    <option value={c.id} key={c.id}>
                      {c.name} · {c.state || t("logs.stateUnknownLower")} ·{" "}
                      {c.id.slice(0, 12)}
                    </option>
                  ))}
                </select>
              </div>
              <div className="logs-field">
                <label htmlFor="logs-tail">{t("logs.lastLines")}</label>
                <select
                  id="logs-tail"
                  value={tail}
                  disabled={!!busy}
                  onChange={(e) => {
                    setTail(Number(e.target.value) as 100 | 300 | 500);
                    clearResult();
                  }}
                >
                  {[100, 300, 500].map((n) => (
                    <option value={n} key={n}>
                      {t("logs.lines", { n })}
                    </option>
                  ))}
                </select>
              </div>
              <button
                className="button primary"
                disabled={!!busy || !selected}
                onClick={query}
              >
                {busy === "logs" ? (
                  <LoaderCircle size={16} className="spin" aria-hidden="true" />
                ) : (
                  <RefreshCw size={16} aria-hidden="true" />
                )}
                {result ? t("logs.refreshLogs") : t("logs.queryLogs")}
              </button>
            </div>
            {selected && (
              <div className="logs-selection">
                <strong>{selected.name}</strong>
                <Badge
                  tone={selected.state === "running" ? "green" : "neutral"}
                >
                  {selected.state || t("logs.stateUnknown")}
                </Badge>
                <code>{selected.id}</code>
                <small>
                  {t("logs.stateNote")}
                </small>
              </div>
            )}
          </>
        )}
        <p className="card-note">
          {t("logs.limits")}
        </p>
      </Panel>
      <div role="status" aria-live="polite">
        {busy ? (
          <p>
            {busy === "list" ? t("logs.loadingList") : t("logs.loadingLogs")}
          </p>
        ) : copied ? (
          <p>{t("logs.copiedStatus")}</p>
        ) : null}
      </div>
      {error && (
        <div className="feedback error" role="alert">
          <AlertTriangle size={18} aria-hidden="true" />
          <p>{error}</p>
        </div>
      )}
      {result && (
        <Panel
          title={t("logs.result.title")}
          eyebrow={t("logs.result.eyebrow")}
          action={
            <div className="logs-actions">
              <button className="button secondary" onClick={() => void copy()}>
                <Copy size={15} aria-hidden="true" />
                {copied ? t("logs.copied") : t("logs.copy")}
              </button>
              <button className="button secondary" onClick={clearResult}>
                <Trash2 size={15} aria-hidden="true" />
                {t("logs.clear")}
              </button>
            </div>
          }
        >
          <div className="console-result-meta">
            <Badge tone={result.status === "available" ? "green" : "amber"}>
              {result.status === "available"
                ? t("logs.status.done")
                : result.status === "timeout"
                  ? t("logs.status.timeout")
                  : result.status === "missing"
                    ? t("logs.status.missing")
                    : result.truncated
                      ? t("logs.status.truncated")
                      : t("logs.status.error")}
            </Badge>
            <span>{result.durationMs} ms</span>
            <span>{t("logs.exitCode", { code: result.exitCode ?? "—" })}</span>
            <span>{t("logs.tailRequested", { tail: result.tail })}</span>
          </div>
          {(result.detail || result.truncated) && (
            <div className="notice warning" role="status">
              <AlertTriangle size={17} aria-hidden="true" />
              <p>{result.detail || t("logs.truncatedLimit")}</p>
            </div>
          )}
          {result.status !== "available" && (
            <p className="card-note">
              {t("logs.incomplete")}
            </p>
          )}
          <div className="logs-streams">
            <section aria-label={t("logs.stdoutLabel")}>
              <h3>stdout</h3>
              <pre tabIndex={0}>
                {logText(result.stdout) || t("logs.noStdout")}
              </pre>
            </section>
            <section aria-label={t("logs.stderrLabel")}>
              <h3>stderr</h3>
              <pre tabIndex={0}>
                {logText(result.stderr) || t("logs.noStderr")}
              </pre>
            </section>
          </div>
          {!result.stdout &&
            !result.stderr &&
            result.status === "available" && (
              <p role="status">{t("logs.noLogs")}</p>
            )}
          <p className="card-note">
            {t("logs.terminalNote")}
          </p>
        </Panel>
      )}
      <div className="notice">
        <ShieldCheck size={18} aria-hidden="true" />
        <p>
          {t("logs.privacy")}
        </p>
      </div>
    </div>
  );
}
