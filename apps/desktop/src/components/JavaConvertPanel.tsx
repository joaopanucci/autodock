import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { AlertTriangle, CheckCircle2, FolderOpen, LoaderCircle, Play } from "lucide-react";
import { call } from "../ipc";
import { Badge } from "./ui";
import { displayPath } from "../model";
import { useI18n } from "../i18n";
import type { ConvertDependency, ConvertOptions, ConvertPlan, ConvertReport, JavaMap } from "../types";

const message = (error: unknown) =>
  typeof error === "string" ? error : error instanceof Error ? error.message : String(error);

/**
 * Turns a folder without a pom.xml into a Maven project, in a copy: the plan is shown (and its
 * coordinates can be changed) before anything is written, and the copy is made after a confirmation.
 */
export function JavaConvertPanel({
  map,
  onContinue,
}: {
  map: JavaMap;
  /** The copy is a Maven project now: go on to the upgrade with it. */
  onContinue: (path: string) => void;
}) {
  const { t } = useI18n();
  const [plan, setPlan] = useState<ConvertPlan | null>(null);
  const [report, setReport] = useState<ConvertReport | null>(null);
  const [planning, setPlanning] = useState(false);
  const [running, setRunning] = useState(false);
  const [confirmed, setConfirmed] = useState(false);
  const [error, setError] = useState("");
  const [into, setInto] = useState<string | null>(null);
  const [group, setGroup] = useState("");
  const [artifact, setArtifact] = useState("");
  const [version, setVersion] = useState("");
  const [level, setLevel] = useState("");

  const optionsFor = (folder: string | null): ConvertOptions => ({
    into: folder,
    groupId: group.trim() || null,
    artifactId: artifact.trim() || null,
    version: version.trim() || null,
    javaLevel: /^\d+$/.test(level.trim()) ? Number(level.trim()) : null,
  });

  async function makePlan(folder: string | null = into, keepFields = false) {
    setPlanning(true);
    setError("");
    setConfirmed(false);
    try {
      const made = await call<ConvertPlan>("java_convert_plan", {
        path: map.path,
        options: optionsFor(folder),
      });
      setPlan(made);
      if (!keepFields) {
        setGroup(made.groupId);
        setArtifact(made.artifactId);
        setVersion(made.version);
        setLevel(String(made.javaLevel));
      }
    } catch (e) {
      setError(message(e));
    } finally {
      setPlanning(false);
    }
  }

  async function chooseDestination() {
    try {
      const folder = await open({ directory: true, multiple: false, title: t("java.convert.destination.dialog") });
      if (folder) {
        setInto(folder);
        await makePlan(folder, true);
      }
    } catch (e) {
      setError(message(e));
    }
  }

  async function run() {
    if (!plan || running) return;
    setRunning(true);
    setError("");
    try {
      setReport(
        await call<ConvertReport>("java_convert_run", {
          request: { path: map.path, options: optionsFor(into), confirmed: true },
        }),
      );
    } catch (e) {
      setError(message(e));
    } finally {
      setRunning(false);
    }
  }

  const scopeLabel = (scope: ConvertDependency["scope"]) => t(`java.convert.scope.${scope}`);

  return (
    <div className="java-convert" data-testid="java-convert">
      <p className="section-description">{t("java.convert.body")}</p>

      {error && (
        <div className="feedback error" role="alert">
          <AlertTriangle size={18} aria-hidden="true" />
          <div>
            <p>{error}</p>
          </div>
        </div>
      )}

      {!plan && !report && (
        <button type="button" className="button primary" disabled={planning} onClick={() => void makePlan(null)}>
          {planning ? <LoaderCircle className="spin" size={16} aria-hidden="true" /> : <Play size={16} aria-hidden="true" />}
          {t("java.convert.plan")}
        </button>
      )}

      {plan && !report && (
        <>
          <div className="expo-project-title">
            <Badge tone="neutral">{t("java.convert.packaging", { packaging: plan.packaging })}</Badge>
            <Badge tone={plan.javaLevelDeclared ? "neutral" : "amber"}>
              {plan.javaLevelDeclared
                ? t("java.project.java", { level: String(plan.javaLevel) })
                : t("java.convert.levelAssumed", { level: plan.javaLevel })}
            </Badge>
            <Badge tone="neutral">{plan.encoding}</Badge>
          </div>

          <p className="field-label">{t("java.convert.coordinates")}</p>
          <div className="java-convert-fields">
            <label>
              <span>groupId</span>
              <input value={group} disabled={running} onChange={(e) => setGroup(e.target.value)} />
            </label>
            <label>
              <span>artifactId</span>
              <input value={artifact} disabled={running} onChange={(e) => setArtifact(e.target.value)} />
            </label>
            <label>
              <span>version</span>
              <input value={version} disabled={running} onChange={(e) => setVersion(e.target.value)} />
            </label>
            <label>
              <span>{t("java.convert.level")}</span>
              <input value={level} inputMode="numeric" disabled={running} onChange={(e) => setLevel(e.target.value)} />
            </label>
            <button
              type="button"
              className="button secondary"
              disabled={planning || running}
              onClick={() => void makePlan(into, true)}
            >
              {t("java.convert.update")}
            </button>
          </div>

          <p className="field-label">{t("java.convert.what")}</p>
          <ul className="expo-notes">
            <li>{t("java.convert.counts.sources", { main: plan.counts.mainSources, test: plan.counts.testSources })}</li>
            {plan.counts.mainResources + plan.counts.testResources > 0 && (
              <li>{t("java.convert.counts.resources", { count: plan.counts.mainResources + plan.counts.testResources })}</li>
            )}
            {plan.counts.webappFiles > 0 && <li>{t("java.convert.counts.web", { count: plan.counts.webappFiles })}</li>}
            {plan.counts.movedToTheirPackage > 0 && (
              <li>{t("java.convert.counts.moved", { count: plan.counts.movedToTheirPackage })}</li>
            )}
            {plan.counts.notCopied > 0 && <li>{t("java.convert.counts.left", { count: plan.counts.notCopied })}</li>}
          </ul>

          {plan.dependencies.length > 0 && (
            <>
              <p className="field-label">{t("java.convert.dependencies", { count: plan.dependencies.length })}</p>
              <ul className="expo-notes">
                {plan.dependencies.map((dependency) => (
                  <li key={`${dependency.groupId}:${dependency.artifactId}:${dependency.jar}`}>
                    <code>
                      {dependency.groupId}:{dependency.artifactId}:{dependency.version}
                    </code>{" "}
                    · {scopeLabel(dependency.scope)}
                    {!dependency.known && <> · {t("java.convert.local")}</>}
                  </li>
                ))}
              </ul>
            </>
          )}

          {plan.warnings.length > 0 && (
            <div className="notice warning">
              <AlertTriangle size={18} aria-hidden="true" />
              <ul className="expo-notes">
                {plan.warnings.slice(0, 8).map((warning) => (
                  <li key={warning}>{warning}</li>
                ))}
              </ul>
            </div>
          )}

          <p className="field-label">{t("java.convert.destination")}</p>
          <p className="mono">{displayPath(plan.destination)}</p>
          <div className="multi-run-actions">
            <button
              type="button"
              className="button secondary"
              disabled={planning || running}
              onClick={() => void chooseDestination()}
            >
              <FolderOpen size={16} aria-hidden="true" /> {t("java.convert.destination.change")}
            </button>
            {into && (
              <button
                type="button"
                className="button secondary"
                disabled={planning || running}
                onClick={() => {
                  setInto(null);
                  void makePlan(null, true);
                }}
              >
                {t("java.convert.destination.default")}
              </button>
            )}
          </div>

          <ul className="expo-notes">
            {plan.notes.map((note) => (
              <li key={note}>{note}</li>
            ))}
          </ul>
          <details className="expo-command">
            <summary>{t("java.convert.pom")}</summary>
            <pre className="code-preview">
              <code>{plan.pom}</code>
            </pre>
          </details>
          {plan.moves.length > 0 && (
            <details className="expo-command">
              <summary>{t("java.convert.moves", { count: plan.moves.length })}</summary>
              <ul className="expo-notes">
                {plan.moves.map((move) => (
                  <li key={move.from}>
                    <code>{move.from}</code> → <code>{move.to}</code>
                  </li>
                ))}
              </ul>
            </details>
          )}

          <label className="multi-confirm">
            <input
              type="checkbox"
              checked={confirmed}
              disabled={running}
              onChange={(e) => setConfirmed(e.target.checked)}
            />
            <span>{t("java.convert.confirm")}</span>
          </label>
          <div className="multi-run-actions">
            <button type="button" className="button primary" disabled={!confirmed || running || planning} onClick={() => void run()}>
              {running ? <LoaderCircle className="spin" size={16} aria-hidden="true" /> : <Play size={16} aria-hidden="true" />}
              {t("java.convert.start")}
            </button>
            <button type="button" className="button secondary" disabled={running} onClick={() => setPlan(null)}>
              {t("java.convert.cancel")}
            </button>
          </div>
        </>
      )}

      {report && (
        <div className="expo-report">
          <div className="feedback success" role="status">
            <CheckCircle2 size={18} aria-hidden="true" />
            <div>
              <strong>{t("java.convert.done", { count: report.filesWritten })}</strong>
              <p className="mono">{displayPath(report.destination)}</p>
            </div>
          </div>
          <ul className="expo-notes">
            {report.nextSteps.map((step) => (
              <li key={step}>{step}</li>
            ))}
          </ul>
          <div className="multi-run-actions">
            <button type="button" className="button primary" onClick={() => onContinue(report.destination)}>
              <Play size={16} aria-hidden="true" /> {t("java.convert.continue")}
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
