import { useRef, useState } from "react";
import { call } from "../ipc";
import {
  AlertTriangle,
  CheckCircle2,
  Clock3,
  Copy,
  LoaderCircle,
  Play,
  ShieldCheck,
  TerminalSquare,
  Trash2,
} from "lucide-react";
import { Badge, Panel } from "../components/ui";
import { parseConsoleContainers } from "../console-format";
import type { ConsoleOperation, ConsoleReport } from "../types";
import { ContainerLogs } from "./ContainerLogs";
import { useI18n } from "../i18n";
import type { Translate } from "../i18n";

type Entry = {
  id: number;
  report: ConsoleReport;
  executedAt: string;
};

function getOperations(t: Translate): {
  id: ConsoleOperation;
  title: string;
  command: string;
  description: string;
}[] {
  return [
    {
      id: "containers",
      title: t("console.op.containers.title"),
      command: "docker ps -a --no-trunc --format {{json .}}",
      description: t("console.op.containers.description"),
    },
    {
      id: "engine_info",
      title: t("console.op.engine.title"),
      command: "docker info",
      description: t("console.op.engine.description"),
    },
    {
      id: "docker_version",
      title: t("console.op.version.title"),
      command: "docker version",
      description: t("console.op.version.description"),
    },
  ];
}

function statusLabel(status: ConsoleReport["status"], t: Translate) {
  switch (status) {
    case "available":
      return t("console.status.available");
    case "missing":
      return t("console.status.missing");
    case "timeout":
      return t("console.status.timeout");
    default:
      return t("console.status.failed");
  }
}

export function Console() {
  const { t, locale } = useI18n();
  const operations = getOperations(t);
  const [area, setArea] = useState<"queries" | "logs">("queries");
  const [operation, setOperation] = useState<ConsoleOperation>("containers");
  const [history, setHistory] = useState<Entry[]>([]);
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [copied, setCopied] = useState(false);
  const [view, setView] = useState<"formatted" | "raw">("formatted");
  const nextId = useRef(0);
  const inFlight = useRef(false);

  const selected = history.find((item) => item.id === selectedId) ?? null;
  const current = operations.find((item) => item.id === operation)!;

  async function execute() {
    if (inFlight.current) return;

    inFlight.current = true;
    setBusy(true);
    setError("");
    setCopied(false);

    try {
      const report = await call<ConsoleReport>("execute_console_operation", {
        operation,
      });

      const entry: Entry = {
        id: ++nextId.current,
        report,
        executedAt: new Date().toLocaleTimeString(locale),
      };

      setHistory((previous) => [entry, ...previous].slice(0, 20));
      setSelectedId(entry.id);
      setView("formatted");
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      inFlight.current = false;
      setBusy(false);
    }
  }

  async function copyOutput() {
    if (!selected) return;

    const report = selected.report;
    const output = [
      `$ ${report.command}`,
      report.stdout,
      report.stderr ? `[stderr]\n${report.stderr}` : "",
      report.detail ? `${t("console.details")}\n${report.detail}` : "",
    ]
      .filter(Boolean)
      .join("\n\n");

    try {
      await navigator.clipboard.writeText(output);
      setCopied(true);
    } catch {
      setError(t("console.error.copy"));
    }
  }

  const formatted =
    selected?.report.operation === "containers" &&
    selected.report.status === "available" &&
    !selected.report.truncated &&
    !/truncad|truncat/i.test(selected.report.detail)
      ? parseConsoleContainers(selected.report.stdout)
      : null;

  return (
    <div className="console-page">
      <div
        className="console-view-switch"
        role="group"
        aria-label={t("console.view.aria")}
      >
        <button
          type="button"
          aria-pressed={area === "queries"}
          className={area === "queries" ? "selected" : ""}
          onClick={() => setArea("queries")}
        >
          {t("console.view.queries")}
        </button>
        <button
          type="button"
          aria-pressed={area === "logs"}
          className={area === "logs" ? "selected" : ""}
          onClick={() => setArea("logs")}
        >
          {t("logs.title")}
        </button>
      </div>
      <div hidden={area !== "logs"}>
        <ContainerLogs />
      </div>
      <div hidden={area !== "queries"}>
        <div className="console-intro">
          <div>
            <p className="eyebrow">{t("containers.eyebrow")}</p>
            <p className="section-description">
              {t("console.intro")}
            </p>
          </div>
          <Badge tone="green">
            <ShieldCheck size={13} aria-hidden="true" />
            {t("console.readonly")}
          </Badge>
        </div>

        <Panel title={t("console.select.title")} eyebrow={t("console.select.eyebrow")}>
          <div className="console-operations">
            {operations.map((item) => (
              <button
                key={item.id}
                type="button"
                className={`console-operation ${
                  operation === item.id ? "selected" : ""
                }`}
                onClick={() => setOperation(item.id)}
                aria-pressed={operation === item.id}
                disabled={busy}
              >
                <strong>{item.title}</strong>
                <code>{item.command}</code>
                <span>{item.description}</span>
              </button>
            ))}
          </div>

          <div className="console-action">
            <div>
              <span>{t("console.selectedCommand")}</span>
              <code>$ {current.command}</code>
            </div>
            <button
              type="button"
              className="button primary"
              onClick={() => void execute()}
              disabled={busy}
            >
              {busy ? (
                <LoaderCircle className="spin" size={17} />
              ) : (
                <Play size={17} />
              )}
              {busy ? t("console.running") : t("console.run")}
            </button>
          </div>
        </Panel>

        {error && (
          <div className="feedback error" role="alert">
            <AlertTriangle size={19} aria-hidden="true" />
            <div>
              <strong>{t("console.error.title")}</strong>
              <p>{error}</p>
            </div>
          </div>
        )}
        {busy && (
          <p role="status" aria-live="polite">
            {t("console.runningStatus")}
          </p>
        )}

        <div className="console-results">
          <Panel
            title={t("console.output.title")}
            eyebrow={t("console.output.eyebrow")}
            action={
              selected ? (
                <button
                  type="button"
                  className="button secondary"
                  onClick={() => void copyOutput()}
                >
                  <Copy size={15} aria-hidden="true" />
                  {copied ? t("logs.copied") : t("console.copy")}
                </button>
              ) : undefined
            }
          >
            {selected ? (
              <>
                <div className="console-result-meta">
                  <Badge
                    tone={
                      selected.report.status === "available" ? "green" : "amber"
                    }
                  >
                    {selected.report.status === "available" ? (
                      <CheckCircle2 size={13} />
                    ) : (
                      <AlertTriangle size={13} />
                    )}
                    {statusLabel(selected.report.status, t)}
                  </Badge>
                  <span>
                    <Clock3 size={14} aria-hidden="true" />
                    {selected.report.durationMs} ms
                  </span>
                  <span>{selected.executedAt}</span>
                  <span>{t("console.exitCode", { code: selected.report.exitCode ?? "—" })}</span>
                </div>
                {selected.report.operation === "containers" &&
                  formatted === null && (
                    <p className="notice warning" role="status">
                      {t("console.formattedUnavailable")}
                    </p>
                  )}
                {
                  <div
                    className="console-view-switch"
                    role="group"
                    aria-label={t("console.resultView")}
                  >
                    <button
                      type="button"
                      className={view === "formatted" ? "selected" : ""}
                      aria-pressed={view === "formatted"}
                      disabled={formatted === null}
                      onClick={() => setView("formatted")}
                    >
                      {t("console.formatted")}
                    </button>
                    <button
                      type="button"
                      className={view === "raw" ? "selected" : ""}
                      aria-pressed={view === "raw"}
                      onClick={() => setView("raw")}
                    >
                      {t("console.raw")}
                    </button>
                  </div>
                }
                {formatted && view === "formatted" ? (
                  <>
                    <div className="console-table-scroll">
                      <table className="console-table">
                        <thead>
                          <tr>
                            <th>{t("console.th.container")}</th>
                            <th>{t("console.th.image")}</th>
                            <th>{t("console.th.state")}</th>
                            <th>{t("console.th.ports")}</th>
                          </tr>
                        </thead>
                        <tbody>
                          {formatted.length === 0 ? (
                            <tr>
                              <td colSpan={4}>{t("console.none")}</td>
                            </tr>
                          ) : (
                            formatted.map((container) => (
                              <tr key={container.id}>
                                <td title={container.id}>{container.name}</td>
                                <td>{container.image}</td>
                                <td>{container.status || container.state}</td>
                                <td>{container.ports || "—"}</td>
                              </tr>
                            ))
                          )}
                        </tbody>
                      </table>
                    </div>
                    {selected.report.stderr && (
                      <div className="notice warning">
                        <p>stderr: {selected.report.stderr}</p>
                      </div>
                    )}
                    {selected.report.detail && (
                      <p className="card-note">{selected.report.detail}</p>
                    )}
                  </>
                ) : (
                  <div className="console-terminal">
                    <div className="console-terminal-heading">
                      <TerminalSquare size={16} aria-hidden="true" />
                      {t("console.dockerOutput")}
                    </div>
                    <pre>
                      <span className="console-command">
                        $ {selected.report.command}
                      </span>
                      {"\n\n"}
                      {selected.report.stdout || t("logs.noStdout")}
                      {selected.report.stderr && (
                        <>
                          {"\n\n[stderr]\n"}
                          {selected.report.stderr}
                        </>
                      )}
                      {selected.report.detail && (
                        <>
                          {`\n\n${t("console.details")}\n`}
                          {selected.report.detail}
                        </>
                      )}
                    </pre>
                  </div>
                )}
              </>
            ) : (
              <div className="console-empty">
                <TerminalSquare size={28} aria-hidden="true" />
                <strong>{t("console.empty.title")}</strong>
                <p>{t("console.empty.description")}</p>
              </div>
            )}
          </Panel>

          <Panel
            title={t("console.history.title")}
            eyebrow={t("console.history.eyebrow")}
            action={
              history.length > 0 ? (
                <button
                  type="button"
                  className="button secondary"
                  onClick={() => {
                    setHistory([]);
                    setSelectedId(null);
                    setCopied(false);
                  }}
                  disabled={busy}
                >
                  <Trash2 size={15} aria-hidden="true" />
                  {t("console.history.clear")}
                </button>
              ) : undefined
            }
          >
            {history.length === 0 ? (
              <p className="section-description">
                {t("console.history.empty")}
              </p>
            ) : (
              <div className="console-history">
                {history.map((entry) => (
                  <button
                    key={entry.id}
                    type="button"
                    className={`console-history-item ${
                      selectedId === entry.id ? "selected" : ""
                    }`}
                    onClick={() => {
                      setSelectedId(entry.id);
                      setCopied(false);
                    }}
                  >
                    <strong>{entry.report.label}</strong>
                    <small>{entry.executedAt}</small>
                    <span>{statusLabel(entry.report.status, t)}</span>
                  </button>
                ))}
              </div>
            )}
          </Panel>
        </div>

        <div className="notice">
          <ShieldCheck size={18} aria-hidden="true" />
          <p>
            {t("console.note")}
          </p>
        </div>
      </div>
    </div>
  );
}
