import { useEffect, useMemo, useState } from "react";
import {
  AlertTriangle,
  ArrowLeft,
  ArrowRight,
  Check,
  CheckCircle2,
  LoaderCircle,
  Play,
  RefreshCw,
  Square,
  Trash2,
} from "lucide-react";
import { call } from "../ipc";
import { Badge, Panel } from "../components/ui";
import { TechLogo } from "../components/TechLogo";
import { displayPath } from "../model";
import { useI18n, type TranslationKey } from "../i18n";
import { SERVICES, STACKS } from "../stacks";
import type {
  ApplyReport,
  AppCandidate,
  AppsReport,
  ComposeAction,
  ComposeReport,
  ComposeStatus,
  GeneratePlan,
  MultiRequest,
  ServiceId,
} from "../types";

const STEP_KEYS: TranslationKey[] = [
  "multi.step.apps",
  "multi.step.services",
  "multi.step.review",
  "multi.step.run",
];

type Row = AppCandidate & { enabled: boolean; nameText: string; portText: string };

const validName = (value: string) => /^[a-z](?:[a-z0-9]|-(?=[a-z0-9])){0,39}$/.test(value);
// Names compose services already use: an app cannot take them.
const SERVICE_NAMES = ["postgres", "mysql", "mongodb", "redis", "rabbitmq", "adminer", "mailhog", "rabbitmq-management"];
const COMPOSE_NAME = /^(docker-compose|compose)[\w.-]*\.ya?ml$/;

const logoFor = (app: AppCandidate) =>
  app.framework === "nextjs" || app.framework === "nestjs" ? app.framework : app.stack;

const message = (error: unknown) =>
  typeof error === "string" ? error : error instanceof Error ? error.message : String(error);

export function MultiApp({
  path,
  report,
  onBack,
}: {
  path: string;
  report: AppsReport;
  onBack: () => void;
}) {
  const { t, tn } = useI18n();
  const [step, setStep] = useState(0);
  const [projectName, setProjectName] = useState(report.projectName);
  const [rows, setRows] = useState<Row[]>(() =>
    report.apps.map((app) => ({ ...app, enabled: app.selected, nameText: app.name, portText: String(app.port) })),
  );
  const [services, setServices] = useState<ServiceId[]>([]);
  const [plan, setPlan] = useState<GeneratePlan | null>(null);
  const [picked, setPicked] = useState<string[]>([]);
  const [active, setActive] = useState("");
  const [result, setResult] = useState<ApplyReport | null>(null);
  const [busy, setBusy] = useState<"preview" | "apply" | null>(null);
  const [error, setError] = useState("");

  const [composeFiles, setComposeFiles] = useState<string[]>([]);
  const [composeFile, setComposeFile] = useState("");
  const [confirmed, setConfirmed] = useState(false);
  const [running, setRunning] = useState<ComposeAction | null>(null);
  const [runReport, setRunReport] = useState<ComposeReport | null>(null);
  const [status, setStatus] = useState<ComposeStatus | null>(null);
  const [elapsed, setElapsed] = useState(0);

  const enabled = rows.filter((row) => row.enabled);

  const problem = useMemo(() => {
    if (!enabled.length) return "multi.error.none" as const;
    if (!validName(projectName)) return "multi.error.project" as const;
    const names = enabled.map((row) => row.nameText);
    if (
      names.some((name) => !validName(name) || SERVICE_NAMES.includes(name)) ||
      new Set(names).size !== names.length
    ) {
      return "multi.error.name" as const;
    }
    if (
      enabled.some((row) => {
        const port = Number(row.portText);
        return !Number.isInteger(port) || port < 1 || port > 65535;
      })
    ) {
      return "multi.error.port" as const;
    }
    return null;
  }, [enabled, projectName]);

  function request(): MultiRequest {
    return {
      path,
      projectName,
      apps: enabled.map((row) => ({
        path: row.path,
        name: row.nameText,
        stack: row.stack,
        role: row.role,
        framework: row.framework,
        port: Number(row.portText),
        startScript: row.startScript,
        packageManager: row.packageManager,
        pythonFramework: row.pythonFramework,
      })),
      services,
    };
  }

  const updateRow = (path: string, stack: string, patch: Partial<Row>) => {
    setPlan(null);
    setResult(null);
    setRows((current) =>
      current.map((row) => (row.path === path && row.stack === stack ? { ...row, ...patch } : row)),
    );
  };

  const toggleService = (id: ServiceId) => {
    setPlan(null);
    setResult(null);
    setServices((current) => (current.includes(id) ? current.filter((s) => s !== id) : [...current, id]));
  };

  async function review() {
    setBusy("preview");
    setError("");
    try {
      const generated = await call<GeneratePlan>("preview_multi_setup", { request: request() });
      setPlan(generated);
      setPicked(generated.files.filter((f) => !f.exists).map((f) => f.name));
      setActive(generated.files.find((f) => COMPOSE_NAME.test(f.name))?.name ?? generated.files[0]?.name ?? "");
      setResult(null);
      setStep(2);
    } catch (e) {
      setError(message(e));
    } finally {
      setBusy(null);
    }
  }

  async function apply() {
    if (!plan || !picked.length) return;
    setBusy("apply");
    setError("");
    try {
      // The seed and ports come from the preview, so the written files are identical.
      const report = await call<ApplyReport>("apply_multi_setup", {
        request: { ...request(), seed: plan.seed, ports: plan.ports },
        files: picked,
      });
      setResult(report);
    } catch (e) {
      setError(message(e));
    } finally {
      setBusy(null);
    }
  }

  async function goToRun() {
    setError("");
    try {
      const found = await call<string[]>("compose_files", { path });
      setComposeFiles(found);
      const generated = plan?.files.find((f) => COMPOSE_NAME.test(f.name))?.name;
      setComposeFile(generated && found.includes(generated) ? generated : (found[0] ?? ""));
      setConfirmed(false);
      setRunReport(null);
      setStep(3);
    } catch (e) {
      setError(message(e));
    }
  }

  async function run(action: ComposeAction) {
    if (!composeFile || !confirmed || running) return;
    setRunning(action);
    setElapsed(0);
    setRunReport(null);
    setError("");
    try {
      setRunReport(
        await call<ComposeReport>("compose_run", {
          request: { path, file: composeFile, action, confirmed: true },
        }),
      );
    } catch (e) {
      setError(message(e));
    } finally {
      setRunning(null);
    }
  }

  // Elapsed time while Docker works.
  useEffect(() => {
    if (!running) return;
    const timer = window.setInterval(() => setElapsed((value) => value + 1), 1000);
    return () => window.clearInterval(timer);
  }, [running]);

  // Live state of the services while the run step is open.
  useEffect(() => {
    if (step !== 3 || !composeFile) return;
    let alive = true;
    const refresh = async () => {
      try {
        const current = await call<ComposeStatus>("compose_status", { path, file: composeFile });
        if (alive) setStatus(current);
      } catch {
        // The next tick tries again.
      }
    };
    void refresh();
    const timer = window.setInterval(() => void refresh(), 4000);
    return () => {
      alive = false;
      window.clearInterval(timer);
    };
  }, [step, composeFile, path, running]);

  const grouped = useMemo(() => {
    const groups = new Map<TranslationKey, typeof SERVICES>();
    for (const service of SERVICES) {
      groups.set(service.group, [...(groups.get(service.group) ?? []), service]);
    }
    return [...groups.entries()];
  }, []);

  const activeFile = plan?.files.find((f) => f.name === active);
  const appUrls = (plan?.ports ?? []).filter((p) => enabled.some((row) => row.nameText === p.name));
  const command = composeFile ? `docker compose -f ${composeFile} up -d --build` : "";

  const stateTone = (state: string, health: string | null) =>
    health === "unhealthy" || state === "dead"
      ? "red"
      : state === "running"
        ? "green"
        : state === "exited" || state === "created"
          ? "amber"
          : "neutral";

  return (
    <div className="wizard multi-app">
      <ol className="wizard-steps" aria-label={t("multi.steps.aria")}>
        {STEP_KEYS.map((labelKey, index) => (
          <li
            key={labelKey}
            className={index === step ? "current" : index < step ? "done" : ""}
            aria-current={index === step ? "step" : undefined}
          >
            <span>{index < step ? <Check size={14} aria-hidden="true" /> : index + 1}</span>
            {t(labelKey)}
          </li>
        ))}
      </ol>

      {error && (
        <div className="feedback error" role="alert">
          <AlertTriangle size={19} aria-hidden="true" />
          <div>
            <strong>{t("wizard.error.title")}</strong>
            <p>{error}</p>
          </div>
        </div>
      )}

      {step === 0 && (
        <Panel title={t("multi.apps.title")} eyebrow={t("multi.stepOf", { n: 1 })}>
          <p className="section-description">{t("multi.apps.body")}</p>
          {report.existingCompose && (
            <div className="notice">
              <p>{t("multi.existingCompose", { file: report.existingCompose })}</p>
            </div>
          )}
          <div className="question-grid">
            <label className="question">
              <span>{t("multi.apps.project")}</span>
              <input
                value={projectName}
                onChange={(e) => {
                  setPlan(null);
                  setProjectName(e.target.value.toLowerCase());
                }}
                aria-invalid={!validName(projectName)}
              />
              <small>{t("multi.apps.projectHint", { name: projectName || "projeto" })}</small>
            </label>
          </div>
          <p className="field-label">{t("multi.apps.list")}</p>
          <ul className="multi-apps">
            {rows.map((row) => {
              const stack = STACKS.find((s) => s.id === row.stack);
              const nameBad = row.enabled && (!validName(row.nameText) || SERVICE_NAMES.includes(row.nameText));
              return (
                <li key={`${row.path}:${row.stack}`} className={row.enabled ? "" : "is-off"}>
                  <label className="multi-app-pick">
                    <input
                      type="checkbox"
                      checked={row.enabled}
                      aria-label={row.nameText}
                      onChange={() => updateRow(row.path, row.stack, { enabled: !row.enabled })}
                    />
                    <TechLogo id={logoFor(row)} />
                  </label>
                  <div className="multi-app-main">
                    <div className="multi-app-title">
                      <strong className="mono">{row.path}</strong>
                      <Badge tone={row.role === "backend" ? "blue" : row.role === "frontend" ? "green" : "amber"}>
                        {t(`multi.role.${row.role}`)}
                      </Badge>
                      <span className="multi-app-stack">{stack?.title}</span>
                      {row.hasDockerfile && <Badge tone="amber">{t("multi.badge.dockerfile")}</Badge>}
                    </div>
                    <small>{row.evidence}</small>
                    {row.note && <p className="field-hint warn">{row.note}</p>}
                  </div>
                  <label className="multi-app-field">
                    <span>{t("multi.col.name")}</span>
                    <input
                      value={row.nameText}
                      disabled={!row.enabled}
                      aria-invalid={nameBad}
                      onChange={(e) => updateRow(row.path, row.stack, { nameText: e.target.value.toLowerCase() })}
                    />
                  </label>
                  <label className="multi-app-field">
                    <span>{t("multi.col.port")}</span>
                    <input
                      inputMode="numeric"
                      value={row.portText}
                      disabled={!row.enabled}
                      onChange={(e) =>
                        updateRow(row.path, row.stack, { portText: e.target.value.replace(/\D/g, "") })
                      }
                    />
                  </label>
                </li>
              );
            })}
          </ul>
          {report.skipped.length > 0 && (
            <details className="multi-skipped">
              <summary>{tn("multi.skipped.title", report.skipped.length)}</summary>
              <p className="field-hint">{t("multi.skipped.body")}</p>
              <ul>
                {report.skipped.map((item) => (
                  <li key={item.path}>
                    <code>{item.path}</code> — {item.reason}
                  </li>
                ))}
              </ul>
            </details>
          )}
          {problem && <p className="field-hint warn">{t(problem)}</p>}
        </Panel>
      )}

      {step === 1 && (
        <Panel title={t("multi.services.title")} eyebrow={t("multi.stepOf", { n: 2 })}>
          <p className="section-description">{t("multi.services.body")}</p>
          {grouped.map(([group, items]) => (
            <fieldset className="service-group" key={group}>
              <legend>{t(group)}</legend>
              <div className="choice-grid">
                {items.map((s) => (
                  <label key={s.id} className={`choice with-logo ${services.includes(s.id) ? "selected" : ""}`}>
                    <input type="checkbox" checked={services.includes(s.id)} onChange={() => toggleService(s.id)} />
                    <TechLogo id={s.id} />
                    <strong>{s.title}</strong>
                    <span>{t(s.hint)}</span>
                  </label>
                ))}
              </div>
            </fieldset>
          ))}
        </Panel>
      )}

      {step === 2 && plan && (
        <Panel title={t("multi.review.title")} eyebrow={t("multi.stepOf", { n: 3 })}>
          <p className="section-description">
            {t("wizard.review.before")} <span className="mono">{displayPath(plan.project)}</span>.{" "}
            {t("wizard.review.after")}
          </p>
          <ul className="file-list">
            {plan.files.map((file) => (
              <li key={file.name}>
                <label>
                  <input
                    type="checkbox"
                    disabled={file.exists || !!result}
                    checked={picked.includes(file.name)}
                    onChange={() =>
                      setPicked((c) => (c.includes(file.name) ? c.filter((n) => n !== file.name) : [...c, file.name]))
                    }
                  />
                  <span className="mono">{file.name}</span>
                </label>
                <button type="button" className="link-button" onClick={() => setActive(file.name)}>
                  {t("wizard.review.view")}
                </button>
                <small>{file.description}</small>
                <span className="file-badges">
                  {file.sensitive && <Badge tone="red">{t("wizard.badge.secrets")}</Badge>}
                  {file.exists ? (
                    <Badge tone="amber">{t("wizard.badge.exists")}</Badge>
                  ) : (
                    <Badge tone="blue">{t("wizard.badge.new")}</Badge>
                  )}
                </span>
              </li>
            ))}
          </ul>
          {!!plan.ports?.length && (
            <div className="port-box">
              <p className="field-label">{t("wizard.ports.title")}</p>
              <ul className="port-list">
                {(plan.ports ?? []).map((item) => (
                  <li key={item.name} className={item.hostPort !== item.preferredPort ? "changed" : ""}>
                    <span>{item.name}</span>
                    <code>
                      localhost:{item.hostPort} → {item.containerPort}
                    </code>
                    {item.hostPort !== item.preferredPort && (
                      <small>{t("wizard.ports.changed", { port: item.preferredPort })}</small>
                    )}
                  </li>
                ))}
              </ul>
            </div>
          )}
          {activeFile && (
            <pre className="code-preview" aria-label={t("wizard.contentAria", { name: activeFile.name })}>
              <code>{activeFile.content}</code>
            </pre>
          )}
          {plan.notes.length > 0 && (
            <div className="notice">
              <ul>
                {plan.notes.map((n) => (
                  <li key={n}>{n}</li>
                ))}
              </ul>
            </div>
          )}
          {result && (
            <div className="feedback success" role="status">
              <CheckCircle2 size={18} aria-hidden="true" />
              <div>
                <strong>
                  {result.written.length
                    ? tn("multi.result.created", result.written.length)
                    : t("multi.result.none")}
                </strong>
                {result.skipped.map((s) => (
                  <p key={s.name}>
                    {s.name}: {s.reason}
                  </p>
                ))}
              </div>
            </div>
          )}
        </Panel>
      )}

      {step === 3 && (
        <Panel title={t("multi.run.title")} eyebrow={t("multi.stepOf", { n: 4 })}>
          {!composeFiles.length ? (
            <p className="section-description">{t("multi.run.none")}</p>
          ) : (
            <>
              <p className="section-description">{t("multi.run.body")}</p>
              {composeFiles.length > 1 && (
                <label className="question">
                  <span>{t("multi.run.file")}</span>
                  <select value={composeFile} onChange={(e) => setComposeFile(e.target.value)}>
                    {composeFiles.map((file) => (
                      <option key={file}>{file}</option>
                    ))}
                  </select>
                </label>
              )}
              <p className="field-label">{t("multi.run.command")}</p>
              <p className="command">
                <code>{command}</code>
              </p>
              <label className="multi-confirm">
                <input
                  type="checkbox"
                  checked={confirmed}
                  disabled={!!running}
                  onChange={(e) => setConfirmed(e.target.checked)}
                />
                <span>{t("multi.run.confirm")}</span>
              </label>
              <div className="multi-run-actions">
                <button
                  type="button"
                  className="button primary"
                  disabled={!confirmed || !!running}
                  onClick={() => void run("up")}
                >
                  {running === "up" ? <LoaderCircle className="spin" size={16} aria-hidden="true" /> : <Play size={16} aria-hidden="true" />}
                  {t("multi.run.up")}
                </button>
                <button
                  type="button"
                  className="button secondary"
                  disabled={!confirmed || !!running}
                  onClick={() => void run("stop")}
                >
                  {running === "stop" ? <LoaderCircle className="spin" size={16} aria-hidden="true" /> : <Square size={15} aria-hidden="true" />}
                  {t("multi.run.stop")}
                </button>
                <button
                  type="button"
                  className="button secondary"
                  disabled={!confirmed || !!running}
                  title={t("multi.run.downHint")}
                  onClick={() => void run("down")}
                >
                  {running === "down" ? <LoaderCircle className="spin" size={16} aria-hidden="true" /> : <Trash2 size={15} aria-hidden="true" />}
                  {t("multi.run.down")}
                </button>
              </div>
              <small className="field-hint">{t("multi.run.downHint")}</small>

              {running && (
                <p className="field-hint" role="status">
                  <LoaderCircle className="spin" size={13} aria-hidden="true" />{" "}
                  {t("multi.run.running", { seconds: elapsed })}
                </p>
              )}
              {runReport && (
                <div className={`feedback ${runReport.succeeded ? "success" : "error"}`} role="status">
                  {runReport.succeeded ? <CheckCircle2 size={18} aria-hidden="true" /> : <AlertTriangle size={18} aria-hidden="true" />}
                  <div>
                    <strong>
                      {runReport.succeeded
                        ? t("multi.run.ok", { seconds: Math.max(1, Math.round(runReport.durationMs / 1000)) })
                        : t("multi.run.failed")}
                    </strong>
                    {runReport.detail && <p>{runReport.detail}</p>}
                    {runReport.output && (
                      <details open={!runReport.succeeded}>
                        <summary>{t("multi.run.output")}</summary>
                        <pre className="code-preview">
                          <code>{`$ ${runReport.command}\n${runReport.output}`}</code>
                        </pre>
                      </details>
                    )}
                  </div>
                </div>
              )}

              <div className="multi-status">
                <div className="multi-status-head">
                  <p className="field-label">{t("multi.status.title")}</p>
                  <button
                    type="button"
                    className="link-button"
                    onClick={() =>
                      void call<ComposeStatus>("compose_status", { path, file: composeFile })
                        .then(setStatus)
                        .catch((e) => setError(message(e)))
                    }
                  >
                    <RefreshCw size={13} aria-hidden="true" /> {t("multi.run.refresh")}
                  </button>
                </div>
                {status && status.detail && status.services.length === 0 && (
                  <p className="field-hint warn">{t("multi.status.error", { detail: status.detail })}</p>
                )}
                {status && status.services.length === 0 && !status.detail && (
                  <p className="field-hint">{t("multi.status.empty")}</p>
                )}
                {status && status.services.length > 0 && (
                  <table className="multi-status-table">
                    <thead>
                      <tr>
                        <th>{t("multi.status.service")}</th>
                        <th>{t("multi.status.state")}</th>
                        <th>{t("multi.status.health")}</th>
                        <th>{t("multi.status.ports")}</th>
                      </tr>
                    </thead>
                    <tbody>
                      {status.services.map((service) => (
                        <tr key={service.container}>
                          <td className="mono">{service.service}</td>
                          <td>
                            <Badge tone={stateTone(service.state, service.health)}>{service.state}</Badge>
                          </td>
                          <td>{service.health ?? "—"}</td>
                          <td className="mono">{service.ports.join(", ") || "—"}</td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                )}
              </div>

              {status?.services.some((s) => s.state === "running") && appUrls.length > 0 && (
                <div className="notice">
                  <p>{t("multi.run.open")}</p>
                  <ul>
                    {appUrls.map((item) => (
                      <li key={item.name}>
                        {item.name}: <code>http://localhost:{item.hostPort}</code>
                      </li>
                    ))}
                  </ul>
                </div>
              )}
            </>
          )}
        </Panel>
      )}

      <div className="wizard-actions">
        <button
          type="button"
          className="button secondary"
          disabled={!!busy || !!running}
          onClick={() => (step === 0 ? onBack() : setStep((s) => s - 1))}
        >
          <ArrowLeft size={16} aria-hidden="true" /> {step === 0 ? t("multi.back") : t("multi.back.step")}
        </button>
        {step === 0 && (
          <button type="button" className="button primary" disabled={!!problem} onClick={() => setStep(1)}>
            {t("multi.next")} <ArrowRight size={16} aria-hidden="true" />
          </button>
        )}
        {step === 1 && (
          <button type="button" className="button primary" disabled={!!busy} onClick={() => void review()}>
            {busy === "preview" ? <LoaderCircle className="spin" size={16} aria-hidden="true" /> : null}
            {t("multi.review.button")} <ArrowRight size={16} aria-hidden="true" />
          </button>
        )}
        {step === 2 && plan && !result && (
          <button
            type="button"
            className="button primary"
            disabled={!picked.length || !!busy}
            onClick={() => void apply()}
          >
            {busy === "apply" ? <LoaderCircle className="spin" size={16} aria-hidden="true" /> : null}
            {tn("multi.review.create", picked.length)}
          </button>
        )}
        {step === 2 && plan && (!!result || !picked.length) && (
          <button type="button" className="button primary" onClick={() => void goToRun()}>
            {t("multi.toRun")} <ArrowRight size={16} aria-hidden="true" />
          </button>
        )}
      </div>
    </div>
  );
}
