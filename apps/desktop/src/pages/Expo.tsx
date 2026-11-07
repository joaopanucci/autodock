import { useCallback, useEffect, useRef, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import {
  AlertTriangle,
  CheckCircle2,
  FolderPlus,
  LoaderCircle,
  Play,
  RefreshCw,
  RotateCcw,
  Trash2,
  X,
} from "lucide-react";
import { call } from "../ipc";
import { Badge, EmptyState, Panel } from "../components/ui";
import { displayPath } from "../model";
import { useI18n } from "../i18n";
import type {
  ExpoBackup,
  ExpoPlan,
  ExpoProject,
  ExpoRestoreReport,
  ExpoUpgradeReport,
  ExpoWatched,
  ExpoWatchState,
  SessionProject,
} from "../types";

const message = (error: unknown) =>
  typeof error === "string" ? error : error instanceof Error ? error.message : String(error);

/** Same folder whatever the slashes, trailing separator or letter case. */
const samePath = (a: string, b: string) => {
  const plain = (value: string) =>
    displayPath(value).replace(/\\/g, "/").replace(/\/+$/, "").toLowerCase();
  return plain(a) === plain(b);
};

export function Expo({
  state,
  setState,
  sessionProjects,
  focusPath,
  clearFocus,
}: {
  state: ExpoWatchState | null;
  setState: (state: ExpoWatchState) => void;
  sessionProjects: SessionProject[];
  /** A project the user asked to upgrade from the alert. */
  focusPath: string | null;
  clearFocus: () => void;
}) {
  const { t, tn, locale } = useI18n();
  const [busy, setBusy] = useState<"check" | "add" | null>(null);
  const [error, setError] = useState("");
  const [suggestions, setSuggestions] = useState<ExpoProject[]>([]);

  const [selected, setSelected] = useState<string | null>(null);
  const [plan, setPlan] = useState<ExpoPlan | null>(null);
  const [planning, setPlanning] = useState(false);
  const [confirmed, setConfirmed] = useState(false);
  const [running, setRunning] = useState(false);
  const [elapsed, setElapsed] = useState(0);
  const [report, setReport] = useState<ExpoUpgradeReport | null>(null);
  const [backups, setBackups] = useState<ExpoBackup[]>([]);
  const [armed, setArmed] = useState<string | null>(null);
  const [restored, setRestored] = useState<ExpoRestoreReport | null>(null);

  const watched = state?.projects ?? [];
  const latest = state?.latest ?? null;

  const date = (seconds: number) => new Date(seconds * 1000).toLocaleString(locale);

  // Session projects that use Expo and are not watched yet.
  const sessionKey = sessionProjects.map((p) => p.scan.project).join("\n");
  const watchedKey = watched.map((p) => p.path).join("\n");
  useEffect(() => {
    let alive = true;
    const candidates = sessionProjects
      .map((p) => displayPath(p.scan.project))
      .filter((path) => !watched.some((w) => samePath(w.path, path)))
      .slice(0, 20);
    void Promise.all(
      candidates.map((path) =>
        call<ExpoProject | null>("expo_inspect", { path }).catch(() => null),
      ),
    ).then((found) => {
      if (alive) setSuggestions(found.filter((p): p is ExpoProject => !!p));
    });
    return () => {
      alive = false;
    };
    // sessionKey/watchedKey stand for the lists above.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sessionKey, watchedKey]);

  // Elapsed time while the upgrade runs.
  useEffect(() => {
    if (!running) return;
    const timer = window.setInterval(() => setElapsed((value) => value + 1), 1000);
    return () => window.clearInterval(timer);
  }, [running]);

  const loadBackups = useCallback(async (path: string) => {
    try {
      setBackups(await call<ExpoBackup[]>("expo_backups", { path }));
    } catch {
      setBackups([]);
    }
  }, []);

  const startUpgrade = useCallback(
    async (path: string) => {
      setSelected(path);
      setPlan(null);
      setReport(null);
      setRestored(null);
      setConfirmed(false);
      setArmed(null);
      setError("");
      setPlanning(true);
      void loadBackups(path);
      try {
        setPlan(await call<ExpoPlan>("expo_upgrade_plan", { path }));
      } catch (e) {
        setError(message(e));
      } finally {
        setPlanning(false);
      }
    },
    [loadBackups],
  );

  // "See upgrade" in the alert opens the plan right away.
  // (The ref keeps a re-run of the effect from planning twice.)
  const handledFocus = useRef<string | null>(null);
  useEffect(() => {
    if (!focusPath) {
      handledFocus.current = null;
      return;
    }
    if (handledFocus.current === focusPath) return;
    handledFocus.current = focusPath;
    clearFocus();
    void startUpgrade(focusPath);
  }, [focusPath, clearFocus, startUpgrade]);

  async function action(kind: "check" | "add" | null, task: () => Promise<ExpoWatchState | undefined>) {
    if (busy) return;
    setBusy(kind);
    setError("");
    try {
      const next = await task();
      if (next) setState(next);
    } catch (e) {
      setError(message(e));
    } finally {
      setBusy(null);
    }
  }

  const check = () => action("check", () => call<ExpoWatchState>("expo_check_latest"));
  const watch = (path: string) => action(null, () => call<ExpoWatchState>("expo_watch_add", { path }));
  const unwatch = (path: string) => {
    if (selected && samePath(selected, path)) closeUpgrade();
    return action(null, () => call<ExpoWatchState>("expo_watch_remove", { path }));
  };
  const dismiss = (path: string) =>
    action(null, () => call<ExpoWatchState>("expo_watch_dismiss", { path }));
  const toggleAuto = (enabled: boolean) =>
    action(null, () => call<ExpoWatchState>("expo_set_check_on_start", { enabled }));
  const addFolder = () =>
    action("add", async () => {
      const folder = await open({
        directory: true,
        multiple: false,
        title: t("expo.add.dialog"),
      });
      return folder ? call<ExpoWatchState>("expo_watch_add", { path: folder }) : undefined;
    });

  function closeUpgrade() {
    setSelected(null);
    setPlan(null);
    setReport(null);
    setRestored(null);
    setConfirmed(false);
    setArmed(null);
    setBackups([]);
  }

  async function runUpgrade() {
    if (!plan || !confirmed || running) return;
    setRunning(true);
    setElapsed(0);
    setReport(null);
    setRestored(null);
    setError("");
    try {
      const result = await call<ExpoUpgradeReport>("expo_upgrade_run", {
        request: { path: plan.project.path, targetSdk: plan.toSdk, confirmed: true },
      });
      setReport(result);
      setConfirmed(false);
      setState(await call<ExpoWatchState>("expo_watch_state"));
      await loadBackups(plan.project.path);
    } catch (e) {
      setError(message(e));
    } finally {
      setRunning(false);
    }
  }

  async function restore(id: string) {
    if (!selected) return;
    if (armed !== id) {
      setArmed(id);
      return;
    }
    setArmed(null);
    setError("");
    try {
      setRestored(
        await call<ExpoRestoreReport>("expo_restore", {
          path: selected,
          backupId: id,
          confirmed: true,
        }),
      );
      setState(await call<ExpoWatchState>("expo_watch_state"));
      await loadBackups(selected);
    } catch (e) {
      setError(message(e));
    }
  }

  const rows = watched;
  const selectedRow = rows.find((row) => selected && samePath(row.path, selected));
  const restorable = backups.filter((b) => b.status !== "failed");

  const status = (row: ExpoWatched) => {
    if (!row.project) return <Badge tone="red">{t("expo.project.missing")}</Badge>;
    if (row.project.sdk === null) return <Badge tone="amber">{t("expo.project.noSdk")}</Badge>;
    if (row.update?.dismissed)
      return <Badge tone="neutral">{t("expo.project.dismissed")}</Badge>;
    if (row.update)
      return <Badge tone="amber">{t("expo.project.behind", { latest: row.update.latestSdk })}</Badge>;
    if (!latest) return <Badge tone="neutral">{t("expo.project.unknown")}</Badge>;
    return <Badge tone="green">{t("expo.project.upToDate")}</Badge>;
  };

  return (
    <div className="expo-page">
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

      <Panel title={t("expo.latest.title")} eyebrow={t("expo.latest.eyebrow")}>
        <div className="expo-latest">
          <div>
            {latest ? (
              <>
                <strong className="expo-latest-sdk">{t("expo.sdk", { sdk: latest.latestSdk })}</strong>
                <p className="field-hint">
                  {t("expo.latest.version", {
                    version: latest.latestVersion,
                    date: date(latest.checkedAt),
                  })}
                </p>
              </>
            ) : (
              <p className="section-description">{t("expo.latest.unknown")}</p>
            )}
          </div>
          <button type="button" className="button secondary" disabled={!!busy} onClick={() => void check()}>
            {busy === "check" ? (
              <LoaderCircle className="spin" size={16} aria-hidden="true" />
            ) : (
              <RefreshCw size={16} aria-hidden="true" />
            )}
            {busy === "check" ? t("expo.latest.checking") : t("expo.latest.check")}
          </button>
        </div>
        <p className="field-hint">{t("expo.latest.hint")}</p>
        {state?.checkError && (
          <p className="field-hint warn">{t("expo.checkError", { detail: state.checkError })}</p>
        )}
        <label className="multi-confirm">
          <input
            type="checkbox"
            checked={!!state?.checkOnStart}
            disabled={!!busy || !state}
            onChange={(e) => void toggleAuto(e.target.checked)}
          />
          <span>{t("expo.auto.label")}</span>
        </label>
        <small className="field-hint">{t("expo.auto.hint")}</small>
      </Panel>

      <Panel
        title={t("expo.projects.title")}
        eyebrow={t("expo.projects.eyebrow")}
        action={
          <button type="button" className="button secondary" disabled={!!busy} onClick={() => void addFolder()}>
            <FolderPlus size={16} aria-hidden="true" /> {t("expo.projects.add")}
          </button>
        }
      >
        <p className="section-description">{t("expo.projects.body")}</p>
        {rows.length === 0 ? (
          <EmptyState
            title={t("expo.projects.empty.title")}
            description={t("expo.projects.empty.body")}
          />
        ) : (
          <ul className="expo-projects">
            {rows.map((row) => {
              const project = row.project;
              const canUpgrade =
                !!project && project.upgradeSupported && project.sdk !== null && !!row.update;
              return (
                <li key={row.path} className={selectedRow === row ? "is-selected" : ""}>
                  <div className="expo-project-main">
                    <div className="expo-project-title">
                      <strong>{row.name}</strong>
                      {project?.sdk != null && (
                        <Badge tone="blue">{t("expo.sdk", { sdk: project.sdk })}</Badge>
                      )}
                      {status(row)}
                    </div>
                    <small className="mono">{displayPath(row.path)}</small>
                    {project && !project.upgradeSupported && project.unsupportedReason && (
                      <small className="field-hint warn">
                        {t("expo.project.unsupported", { reason: project.unsupportedReason })}
                      </small>
                    )}
                  </div>
                  <div className="expo-project-actions">
                    {canUpgrade && (
                      <button
                        type="button"
                        className="button primary"
                        disabled={running || planning}
                        onClick={() => void startUpgrade(row.path)}
                      >
                        <Play size={15} aria-hidden="true" />
                        {t("expo.project.upgrade", { sdk: row.update!.nextSdk })}
                      </button>
                    )}
                    {row.update && !row.update.dismissed && (
                      <button
                        type="button"
                        className="button secondary"
                        disabled={!!busy}
                        onClick={() => void dismiss(row.path)}
                      >
                        {t("expo.project.dismiss")}
                      </button>
                    )}
                    <button
                      type="button"
                      className="button secondary"
                      disabled={!!busy || running}
                      onClick={() => void unwatch(row.path)}
                    >
                      <Trash2 size={15} aria-hidden="true" /> {t("expo.project.remove")}
                    </button>
                  </div>
                </li>
              );
            })}
          </ul>
        )}

        {suggestions.length > 0 && (
          <div className="expo-suggest">
            <p className="field-label">{t("expo.suggest.title")}</p>
            <p className="field-hint">{t("expo.suggest.body")}</p>
            <ul className="expo-projects">
              {suggestions.map((project) => (
                <li key={project.path}>
                  <div className="expo-project-main">
                    <div className="expo-project-title">
                      <strong>{project.name}</strong>
                      {project.sdk !== null && (
                        <Badge tone="blue">{t("expo.sdk", { sdk: project.sdk })}</Badge>
                      )}
                    </div>
                    <small className="mono">{displayPath(project.path)}</small>
                  </div>
                  <div className="expo-project-actions">
                    <button
                      type="button"
                      className="button secondary"
                      disabled={!!busy}
                      onClick={() => void watch(project.path)}
                    >
                      {t("expo.suggest.watch")}
                    </button>
                  </div>
                </li>
              ))}
            </ul>
          </div>
        )}
      </Panel>

      {selected && (
        <Panel
          title={
            plan
              ? t("expo.upgrade.title", {
                  name: plan.project.name,
                  from: plan.fromSdk,
                  to: plan.toSdk,
                })
              : (selectedRow?.name ?? displayPath(selected))
          }
          eyebrow={t("expo.upgrade.eyebrow")}
          action={
            <button
              type="button"
              className="button secondary"
              disabled={running}
              onClick={closeUpgrade}
            >
              {t("expo.upgrade.cancel")}
            </button>
          }
        >
          {planning && (
            <p className="field-hint" role="status">
              <LoaderCircle className="spin" size={13} aria-hidden="true" /> {t("expo.upgrade.loading")}
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
              <p className="field-label">{t("expo.upgrade.steps")}</p>
              <ol className="expo-steps">
                {plan.steps.map((step) => (
                  <li key={step}>{step}</li>
                ))}
              </ol>
              <p className="field-label">{t("expo.upgrade.files")}</p>
              <p>
                {plan.filesTouched.map((file) => (
                  <code key={file} className="expo-file">
                    {file}
                  </code>
                ))}
              </p>
              <p className="field-label">{t("expo.upgrade.notes")}</p>
              <ul className="expo-notes">
                {plan.notes.map((note) => (
                  <li key={note}>{note}</li>
                ))}
              </ul>
              <p className="field-label">{t("expo.upgrade.links")}</p>
              <ul className="expo-notes">
                {plan.links.map((link) => (
                  <li key={link.url}>
                    {link.label}: <code>{link.url}</code>
                  </li>
                ))}
              </ul>
              <details className="expo-command">
                <summary>{t("expo.upgrade.command")}</summary>
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
                <span>{t("expo.upgrade.confirm")}</span>
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
                  {t("expo.upgrade.start", { sdk: plan.toSdk })}
                </button>
              </div>
              {running && (
                <p className="field-hint" role="status">
                  <LoaderCircle className="spin" size={13} aria-hidden="true" />{" "}
                  {t("expo.upgrade.running", { seconds: elapsed })} · {t("expo.upgrade.runningHint")}
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
                      ? t("expo.upgrade.ok", {
                          from: report.fromSdk,
                          to: report.toSdk,
                          seconds: Math.max(1, Math.round(report.durationMs / 1000)),
                        })
                      : t("expo.upgrade.failed")}
                  </strong>
                  {report.detail && <p>{report.detail}</p>}
                  {!report.succeeded && report.restored && <p>{t("expo.upgrade.restoredAuto")}</p>}
                </div>
              </div>

              {report.succeeded && (
                <>
                  <p
                    className={`field-hint ${report.doctorPassed === false ? "warn" : ""}`}
                    role="status"
                  >
                    {report.doctorPassed === false
                      ? t("expo.upgrade.doctorFail")
                      : t("expo.upgrade.doctorOk")}
                  </p>
                  <details open={report.doctorPassed === false}>
                    <summary>{t("expo.upgrade.doctor")}</summary>
                    <pre className="code-preview">
                      <code>{report.doctor || "—"}</code>
                    </pre>
                  </details>
                  <details>
                    <summary>{tn("expo.upgrade.changes", report.changes.length)}</summary>
                    <table className="multi-status-table expo-changes">
                      <thead>
                        <tr>
                          <th>{t("expo.upgrade.col.package")}</th>
                          <th>{t("expo.upgrade.col.from")}</th>
                          <th>{t("expo.upgrade.col.to")}</th>
                        </tr>
                      </thead>
                      <tbody>
                        {report.changes.map((change) => (
                          <tr key={`${change.section}:${change.name}`}>
                            <td className="mono">{change.name}</td>
                            <td className="mono">{change.from ?? t("expo.upgrade.added")}</td>
                            <td className="mono">{change.to ?? t("expo.upgrade.removed")}</td>
                          </tr>
                        ))}
                      </tbody>
                    </table>
                  </details>
                  <p className="field-label">{t("expo.upgrade.next")}</p>
                  <ul className="expo-notes">
                    {report.nextSteps.map((step) => (
                      <li key={step}>{step}</li>
                    ))}
                  </ul>
                </>
              )}
              {!report.succeeded && report.output && (
                <details open>
                  <summary>{t("expo.upgrade.output")}</summary>
                  <pre className="code-preview">
                    <code>{report.output}</code>
                  </pre>
                </details>
              )}
            </div>
          )}

          {(restorable.length > 0 || restored) && (
            <div className="expo-backups">
              <p className="field-label">{t("expo.backups.title")}</p>
              <p className="field-hint">{t("expo.backups.body")}</p>
              <ul>
                {restorable.map((backup) => (
                  <li key={backup.id}>
                    <div>
                      <strong>
                        {t("expo.backups.item", {
                          date: date(backup.createdAt),
                          from: backup.fromSdk,
                          to: backup.toSdk,
                        })}
                      </strong>
                      <small>{t(`expo.backups.status.${backup.status}` as "expo.backups.status.done")}</small>
                      {backup.modifiedSinceUpgrade && (
                        <small className="field-hint warn">{t("expo.backups.modified")}</small>
                      )}
                    </div>
                    <button
                      type="button"
                      className={`button ${armed === backup.id ? "primary" : "secondary"}`}
                      disabled={running}
                      onClick={() => void restore(backup.id)}
                    >
                      <RotateCcw size={15} aria-hidden="true" />
                      {armed === backup.id ? t("expo.backups.confirm") : t("expo.backups.restore")}
                    </button>
                  </li>
                ))}
              </ul>
              {restored && (
                <div className="feedback success" role="status">
                  <CheckCircle2 size={18} aria-hidden="true" />
                  <div>
                    <strong>{restored.restored.join(", ")}</strong>
                    <p>{restored.detail}</p>
                  </div>
                </div>
              )}
            </div>
          )}
        </Panel>
      )}
    </div>
  );
}
