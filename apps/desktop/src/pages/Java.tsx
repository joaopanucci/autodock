import { useCallback, useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { AlertTriangle, CheckCircle2, FolderOpen, GitCompare, LoaderCircle, Play, X } from "lucide-react";
import { call } from "../ipc";
import { Badge, EmptyState, Panel } from "../components/ui";
import { JavaExplainer } from "../components/JavaExplainer";
import { JavaConvertPanel } from "../components/JavaConvertPanel";
import { JavaMapPanel } from "../components/JavaMapPanel";
import { displayPath } from "../model";
import { useI18n } from "../i18n";
import type { JavaChanges, JavaMap, JavaPlan, JavaProject, JavaUpgradeReport, SessionProject } from "../types";

const message = (error: unknown) =>
  typeof error === "string" ? error : error instanceof Error ? error.message : String(error);

export function Java({
  sessionProjects,
  onReview,
}: {
  sessionProjects: SessionProject[];
  /** Opens the changes of an upgrade in the editor, side by side with the original. */
  onReview: (original: string, copy: string, changes: JavaChanges) => void;
}) {
  const { t } = useI18n();
  const [error, setError] = useState("");
  const [project, setProject] = useState<JavaProject | null>(null);
  /** A folder with Java code but no Maven project: what is in it. */
  const [map, setMap] = useState<JavaMap | null>(null);
  const [inspecting, setInspecting] = useState(false);
  const [suggestions, setSuggestions] = useState<JavaProject[]>([]);

  const [plan, setPlan] = useState<JavaPlan | null>(null);
  // Folder chosen to hold the upgraded copy (null: next to the project).
  const [into, setInto] = useState<string | null>(null);
  const [planning, setPlanning] = useState(false);
  const [confirmed, setConfirmed] = useState(false);
  const [running, setRunning] = useState(false);
  const [elapsed, setElapsed] = useState(0);
  const [report, setReport] = useState<JavaUpgradeReport | null>(null);

  // Maven projects among the ones analyzed in this session.
  const sessionKey = sessionProjects.map((p) => p.scan.project).join("\n");
  useEffect(() => {
    let alive = true;
    const candidates = sessionProjects.map((p) => displayPath(p.scan.project)).slice(0, 20);
    void Promise.all(
      candidates.map((path) =>
        call<JavaProject | null>("java_inspect", { path }).catch(() => null),
      ),
    ).then((found) => {
      if (alive) {
        setSuggestions(found.filter((p): p is JavaProject => !!p && p.buildTool === "maven"));
      }
    });
    return () => {
      alive = false;
    };
    // sessionKey stands for sessionProjects.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sessionKey]);

  // Elapsed time while the upgrade runs.
  useEffect(() => {
    if (!running) return;
    const timer = window.setInterval(() => setElapsed((value) => value + 1), 1000);
    return () => window.clearInterval(timer);
  }, [running]);

  const reset = () => {
    setPlan(null);
    setReport(null);
    setConfirmed(false);
  };

  const choose = useCallback(async (path: string) => {
    setError("");
    setProject(null);
    setMap(null);
    setInto(null);
    reset();
    setInspecting(true);
    try {
      const found = await call<JavaProject | null>("java_inspect", { path });
      // Anything that is not a Maven project is mapped, so the person sees what is in the folder.
      let mapped: JavaMap | null = null;
      if (!found || found.buildTool !== "maven") {
        mapped = await call<JavaMap>("java_map", { path }).catch(() => null);
        if (mapped && mapped.counts.java === 0) mapped = null;
      }
      if (!found && !mapped) {
        setError(t("java.pick.empty.body"));
      }
      setProject(found);
      setMap(mapped);
    } catch (e) {
      setError(message(e));
    } finally {
      setInspecting(false);
    }
  }, [t]);

  async function pickFolder() {
    try {
      const folder = await open({ directory: true, multiple: false, title: t("java.pick.dialog") });
      if (folder) await choose(folder);
    } catch (e) {
      setError(message(e));
    }
  }

  async function makePlan(folder: string | null | undefined = undefined) {
    if (!project || planning) return;
    const holder = folder === undefined ? into : folder;
    reset();
    setError("");
    setPlanning(true);
    try {
      setPlan(
        await call<JavaPlan>("java_upgrade_plan", {
          path: project.path,
          ...(holder ? { destination: holder } : {}),
        }),
      );
    } catch (e) {
      setError(message(e));
    } finally {
      setPlanning(false);
    }
  }

  async function chooseDestination() {
    try {
      const folder = await open({
        directory: true,
        multiple: false,
        title: t("java.upgrade.destination.dialog"),
      });
      if (folder) {
        setInto(folder);
        await makePlan(folder);
      }
    } catch (e) {
      setError(message(e));
    }
  }

  async function backToDefaultDestination() {
    setInto(null);
    await makePlan(null);
  }

  async function runUpgrade() {
    if (!plan || !confirmed || running) return;
    setRunning(true);
    setElapsed(0);
    setReport(null);
    setError("");
    try {
      setReport(
        await call<JavaUpgradeReport>("java_upgrade_run", {
          request: {
            path: plan.project.path,
            confirmed: true,
            ...(into ? { destination: into } : {}),
          },
        }),
      );
      setConfirmed(false);
    } catch (e) {
      setError(message(e));
    } finally {
      setRunning(false);
    }
  }

  const planTitle = (p: JavaPlan) => {
    const from = p.fromJava ?? "?";
    return p.toSpringBoot && p.fromSpringBoot
      ? t("java.upgrade.titleBoot", {
          name: p.project.name,
          from,
          to: p.toJava,
          bootFrom: p.fromSpringBoot,
          bootTo: p.toSpringBoot,
        })
      : t("java.upgrade.title", { name: p.project.name, from, to: p.toJava });
  };

  const stepTone = (status: string) =>
    status === "ok" ? "green" : status === "failed" ? "red" : "neutral";

  return (
    <div className="expo-page java-page">
      {error && (
        <div className="feedback error" role="alert">
          <AlertTriangle size={19} aria-hidden="true" />
          <div>
            <strong>{t("wizard.error.title")}</strong>
            <p>{error}</p>
          </div>
          <button className="icon-button" aria-label={t("app.closeError")} onClick={() => setError("")}>
            <X size={16} />
          </button>
        </div>
      )}

      <Panel
        title={t("java.pick.title")}
        eyebrow={t("java.pick.eyebrow")}
        action={
          <button
            type="button"
            className="button secondary"
            disabled={inspecting || running}
            onClick={() => void pickFolder()}
          >
            <FolderOpen size={16} aria-hidden="true" /> {t("java.pick.button")}
          </button>
        }
      >
        <p className="section-description">{t("java.pick.body")}</p>

        {inspecting && (
          <p className="field-hint" role="status">
            <LoaderCircle className="spin" size={13} aria-hidden="true" /> {t("java.inspect.loading")}
          </p>
        )}

        {!project && !map && !inspecting && (
          <EmptyState title={t("java.pick.empty.title")} description={t("java.pick.empty.body")} />
        )}

        {project && (
          <ul className="expo-projects">
            <li className="is-selected">
              <div className="expo-project-main">
                <div className="expo-project-title">
                  <strong>{project.name}</strong>
                  <Badge tone="neutral">
                    {project.buildTool === "maven" ? t("java.project.maven") : project.buildTool}
                  </Badge>
                  {project.packaging === "war" && <Badge tone="neutral">{t("java.project.war")}</Badge>}
                  <Badge tone={project.javaLevel !== null && project.javaLevel >= 17 ? "green" : "amber"}>
                    {project.javaDeclared
                      ? t("java.project.java", { level: project.javaDeclared })
                      : t("java.project.javaUnknown")}
                  </Badge>
                  {project.springBoot && (
                    <Badge tone="blue">{t("java.project.boot", { version: project.springBoot })}</Badge>
                  )}
                  {project.springBootPinnedBy && (
                    <Badge tone="amber">
                      {t("java.project.pinned", { bom: project.springBootPinnedBy })}
                    </Badge>
                  )}
                  {project.modules.length > 0 && (
                    <Badge tone="neutral">{t("java.project.modules", { count: project.modules.length })}</Badge>
                  )}
                  {project.upgradeSupported && !project.upgradeAvailable && (
                    <Badge tone="green">{t("java.project.upToDate")}</Badge>
                  )}
                </div>
                <small className="mono">{displayPath(project.path)}</small>
                {!project.upgradeSupported && project.unsupportedReason && (
                  <small className="field-hint warn">
                    {t("java.project.unsupported", { reason: project.unsupportedReason })}
                  </small>
                )}
              </div>
              <div className="expo-project-actions">
                {project.upgradeSupported && project.upgradeAvailable && (
                  <button
                    type="button"
                    className="button primary"
                    disabled={planning || running}
                    onClick={() => void makePlan(undefined)}
                  >
                    <Play size={15} aria-hidden="true" /> {t("java.project.plan")}
                  </button>
                )}
              </div>
            </li>
          </ul>
        )}

        {project && project.jdkApis.length > 0 && (
          <>
            <p className="field-label">{t("java.apis.title")}</p>
            <ul className="expo-notes">
              {project.jdkApis.map((api) => (
                <li key={api.name}>
                  <code>{api.name}</code> · {t("java.apis.files", { count: api.files })} ·{" "}
                  {api.fixable ? t("java.apis.fixable") : t("java.apis.manual")}
                </li>
              ))}
            </ul>
          </>
        )}

        {suggestions.length > 0 && !project && (
          <div className="expo-suggest">
            <p className="field-label">{t("java.suggest.title")}</p>
            <p className="field-hint">{t("java.suggest.body")}</p>
            <ul className="expo-projects">
              {suggestions.map((item) => (
                <li key={item.path}>
                  <div className="expo-project-main">
                    <div className="expo-project-title">
                      <strong>{item.name}</strong>
                      {item.javaDeclared && (
                        <Badge tone="blue">{t("java.project.java", { level: item.javaDeclared })}</Badge>
                      )}
                    </div>
                    <small className="mono">{displayPath(item.path)}</small>
                  </div>
                  <div className="expo-project-actions">
                    <button
                      type="button"
                      className="button secondary"
                      disabled={inspecting}
                      onClick={() => void choose(item.path)}
                    >
                      {t("java.suggest.use")}
                    </button>
                  </div>
                </li>
              ))}
            </ul>
          </div>
        )}
      </Panel>

      {map && (
        <Panel title={t("java.map.title")} eyebrow={t("java.map.eyebrow")}>
          <JavaMapPanel map={map} />
        </Panel>
      )}

      {map && map.layout !== "maven" && (
        <Panel title={t("java.convert.title")} eyebrow={t("java.convert.eyebrow")}>
          <JavaConvertPanel key={map.path} map={map} onContinue={(path) => void choose(path)} />
        </Panel>
      )}

      {(planning || plan) && (
        <Panel
          title={plan ? planTitle(plan) : (project?.name ?? "")}
          eyebrow={t("java.upgrade.eyebrow")}
          action={
            <button type="button" className="button secondary" disabled={running} onClick={reset}>
              {t("java.upgrade.cancel")}
            </button>
          }
        >
          {planning && (
            <p className="field-hint" role="status">
              <LoaderCircle className="spin" size={13} aria-hidden="true" /> {t("java.upgrade.loading")}
            </p>
          )}

          {plan && !report && (
            <>
              {plan.warnings.map((warning) => (
                <div className="notice warning" key={warning}>
                  <AlertTriangle size={18} aria-hidden="true" />
                  <p>{warning}</p>
                </div>
              ))}
              <p className="field-label">{t("java.upgrade.steps")}</p>
              <ol className="expo-steps">
                {plan.hops.map((hop) => (
                  <li key={hop.id}>
                    {hop.label}
                    {hop.checked && <small className="field-hint"> · {t("java.upgrade.compiles")}</small>}
                  </li>
                ))}
              </ol>
              {plan.fixups.length > 0 && (
                <>
                  <p className="field-label">{t("java.upgrade.fixups")}</p>
                  <ul className="expo-notes">
                    {plan.fixups.map((fixup) => (
                      <li key={`${fixup.module}:${fixup.dependency}`}>
                        <code>{fixup.dependency}</code> ({fixup.because}
                        {fixup.module ? `, ${fixup.module}` : ""})
                      </li>
                    ))}
                  </ul>
                </>
              )}
              <p className="field-label">{t("java.upgrade.destination")}</p>
              <p>
                <code className="expo-file">{displayPath(plan.destination)}</code>
              </p>
              <p className="field-hint">{t("java.upgrade.destination.hint")}</p>
              <div className="multi-run-actions">
                <button
                  type="button"
                  className="button secondary"
                  disabled={running || planning}
                  onClick={() => void chooseDestination()}
                >
                  <FolderOpen size={16} aria-hidden="true" /> {t("java.upgrade.destination.change")}
                </button>
                {into && (
                  <button
                    type="button"
                    className="button secondary"
                    disabled={running || planning}
                    onClick={() => void backToDefaultDestination()}
                  >
                    {t("java.upgrade.destination.default")}
                  </button>
                )}
              </div>
              <p className="field-label">{t("java.upgrade.notes")}</p>
              <ul className="expo-notes">
                {plan.notes.map((note) => (
                  <li key={note}>{note}</li>
                ))}
              </ul>
              <p className="field-label">{t("java.upgrade.links")}</p>
              <ul className="expo-notes">
                {plan.links.map((link) => (
                  <li key={link.url}>
                    {link.label}: <code>{link.url}</code>
                  </li>
                ))}
              </ul>
              <details className="expo-command">
                <summary>{t("java.upgrade.command")}</summary>
                <pre className="code-preview">
                  <code>{plan.command}</code>
                </pre>
              </details>
              <label className="multi-confirm">
                <input
                  type="checkbox"
                  checked={confirmed}
                  disabled={running}
                  onChange={(e) => setConfirmed(e.target.checked)}
                />
                <span>{t("java.upgrade.confirm")}</span>
              </label>
              <div className="multi-run-actions">
                <button
                  type="button"
                  className="button primary"
                  disabled={!confirmed || running}
                  onClick={() => void runUpgrade()}
                >
                  {running ? (
                    <LoaderCircle className="spin" size={16} aria-hidden="true" />
                  ) : (
                    <Play size={16} aria-hidden="true" />
                  )}
                  {t("java.upgrade.start")}
                </button>
              </div>
              {running && (
                <p className="field-hint" role="status">
                  <LoaderCircle className="spin" size={13} aria-hidden="true" />{" "}
                  {t("java.upgrade.running", { seconds: elapsed })} · {t("java.upgrade.runningHint")}
                </p>
              )}
            </>
          )}

          {report && (
            <div className="expo-report">
              <div className={`feedback ${report.succeeded ? "success" : "error"}`} role="status">
                {report.succeeded ? (
                  <CheckCircle2 size={18} aria-hidden="true" />
                ) : (
                  <AlertTriangle size={18} aria-hidden="true" />
                )}
                <div>
                  <strong>
                    {report.succeeded
                      ? t("java.upgrade.ok", { seconds: Math.max(1, Math.round(report.durationMs / 1000)) })
                      : report.rolledBack && report.reached
                        ? t("java.upgrade.partial")
                        : t("java.upgrade.failed")}
                  </strong>
                  {report.detail && <p>{report.detail}</p>}
                </div>
              </div>

              <p className="field-label">{t("java.report.destination")}</p>
              <p>
                <code className="expo-file">{displayPath(report.destination)}</code>
              </p>

              <p className="field-label">{t("java.report.steps")}</p>
              <ul className="java-hops">
                {report.hops.map((hop) => (
                  <li key={hop.id}>
                    <Badge tone={stepTone(hop.status)}>
                      {t(`java.report.step.${hop.status}` as "java.report.step.ok")}
                    </Badge>
                    <span>{hop.label}</span>
                    {hop.compiled !== null && (
                      <small className="field-hint">
                        {hop.compiled ? t("java.report.compiled") : t("java.report.notCompiled")}
                      </small>
                    )}
                  </li>
                ))}
              </ul>

              {report.fixups.length > 0 && (
                <>
                  <p className="field-label">{t("java.upgrade.fixups")}</p>
                  <ul className="expo-notes">
                    {report.fixups.map((fixup) => (
                      <li key={`${fixup.module}:${fixup.dependency}`}>
                        <code>{fixup.dependency}</code> ({fixup.because})
                      </li>
                    ))}
                  </ul>
                </>
              )}

              {(report.succeeded || (report.rolledBack && report.reached)) && (
                <>
                  <p className="field-label">{t("java.report.tests")}</p>
                  <p
                    className={`field-hint ${report.tests && !report.tests.passed ? "warn" : ""}`}
                    role="status"
                  >
                    {!report.tests
                      ? t("java.report.testsNone")
                      : report.tests.passed
                        ? t("java.report.testsOk", {
                            run: report.tests.run,
                            skipped: report.tests.skipped,
                          })
                        : t("java.report.testsFail", {
                            run: report.tests.run,
                            failures: report.tests.failures,
                            errors: report.tests.errors,
                          })}
                  </p>
                  {report.testsNeedDocker && (
                    <p className="field-hint">{t("java.report.testsDocker")}</p>
                  )}
                  {report.changes.total > 0 && (
                    <div className="multi-run-actions">
                      <button
                        type="button"
                        className="button primary"
                        onClick={() => onReview(plan?.project.path ?? project?.path ?? "", report.destination, report.changes)}
                      >
                        <GitCompare size={16} aria-hidden="true" /> {t("java.report.review")}
                      </button>
                    </div>
                  )}
                  <details>
                    <summary>{t("java.report.changes", { total: report.changes.total })}</summary>
                    {(
                      [
                        ["modified", report.changes.modified],
                        ["added", report.changes.added],
                        ["removed", report.changes.removed],
                      ] as const
                    ).map(
                      ([kind, files]) =>
                        files.length > 0 && (
                          <div key={kind}>
                            <p className="field-label">
                              {t(`java.report.${kind}` as "java.report.modified")} ({files.length})
                            </p>
                            <ul className="expo-notes">
                              {files.map((file) => (
                                <li key={file}>
                                  <code>{file}</code>
                                </li>
                              ))}
                            </ul>
                          </div>
                        ),
                    )}
                    {report.changes.truncated && (
                      <p className="field-hint">{t("java.report.truncated")}</p>
                    )}
                  </details>
                </>
              )}

              {report.nextSteps.length > 0 && (
                <>
                  <p className="field-label">{t("java.report.next")}</p>
                  <ul className="expo-notes">
                    {report.nextSteps.map((step) => (
                      <li key={step}>{step}</li>
                    ))}
                  </ul>
                </>
              )}

              {!report.succeeded && report.output && (
                <details open>
                  <summary>{t("java.report.output")}</summary>
                  <pre className="code-preview">
                    <code>{report.output}</code>
                  </pre>
                </details>
              )}
            </div>
          )}
        </Panel>
      )}

      <JavaExplainer open={!project && !map && !plan && !report} />
    </div>
  );
}
