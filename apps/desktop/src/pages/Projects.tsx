import {
  FileCode2,
  FolderGit2,
  FolderOpen,
  ScanLine,
  Layers3,
  Info,
  AlertTriangle,
  WandSparkles,
} from "lucide-react";
import type { Scan, SessionProject } from "../types";
import {
  displayPath,
  groupModules,
  projectName,
  technologyNames,
} from "../model";
import { Badge, EmptyState, Panel } from "../components/ui";
import { useI18n } from "../i18n";

export function Projects({
  path,
  setPath,
  scan,
  projects,
  busy,
  select,
  analyze,
  inspect,
  prepareDocker,
}: {
  path: string;
  setPath: (value: string) => void;
  scan: Scan | null;
  projects: SessionProject[];
  busy: boolean;
  select: () => void;
  analyze: () => void;
  inspect: (project: SessionProject) => void;
  prepareDocker: () => void;
}) {
  const { t, tn } = useI18n();
  const modules = scan ? groupModules(scan) : [];
  return (
    <>
      <Panel title={t("projects.import.title")} eyebrow={t("projects.import.eyebrow")}>
        <form
          onSubmit={(event) => {
            event.preventDefault();
            if (path.trim() && !busy) analyze();
          }}
        >
          <label className="field-label" htmlFor="project-path">
            {t("projects.path.label")}
          </label>
          <div className="path-control">
            <FolderOpen size={18} aria-hidden="true" />
            <input
              id="project-path"
              value={path}
              disabled={busy}
              onChange={(event) => setPath(event.target.value)}
              placeholder={t("projects.path.placeholder")}
            />
            <button
              type="button"
              className="button secondary"
              disabled={busy}
              onClick={select}
            >
              {t("projects.selectFolder")}
            </button>
            <button
              type="submit"
              className="button primary"
              disabled={busy || !path.trim()}
            >
              <ScanLine size={16} aria-hidden="true" />
              {t("projects.analyze")}
            </button>
          </div>
          <p className="field-hint">
            {t("projects.hint")}
          </p>
        </form>
      </Panel>
      {projects.length > 0 && (
        <div className="project-tabs" aria-label={t("projects.tabs.aria")}>
          {projects.map((p) => (
            <button
              key={p.scan.project}
              className={scan?.project === p.scan.project ? "selected" : ""}
              onClick={() => inspect(p)}
              disabled={busy}
            >
              <FolderGit2 size={15} aria-hidden="true" />
              {projectName(p.scan.project)}
            </button>
          ))}
        </div>
      )}
      {!scan ? (
        <Panel title={t("projects.structure.title")}>
          <EmptyState
            icon={Layers3}
            title={t("projects.empty.title")}
            description={t("projects.empty.description")}
          />
        </Panel>
      ) : (
        <>
          <div className="results-heading">
            <div>
              <p className="eyebrow">{t("projects.done.eyebrow")}</p>
              <h2>{projectName(scan.project)}</h2>
              <p className="mono path-text">{displayPath(scan.project)}</p>
            </div>
            <button className="button primary" onClick={prepareDocker} disabled={busy}>
              <WandSparkles size={16} aria-hidden="true" />
              {t("projects.prepareDocker")}
            </button>
            <Badge tone={scan.warnings.length ? "amber" : "green"}>
              {tn("projects.modules", modules.length)}
            </Badge>
          </div>
          {!!scan.warnings.length && (
            <section className="notice warning" aria-label={t("projects.warnings.aria")}>
              <AlertTriangle size={20} aria-hidden="true" />
              <div>
                <h3>{t("projects.warnings.title")}</h3>
                <ul>
                  {scan.warnings.map((w, i) => (
                    <li key={i}>{w}</li>
                  ))}
                </ul>
              </div>
            </section>
          )}
          {!modules.length && (
            <Panel title={t("projects.result.title")}>
              <EmptyState
                icon={FileCode2}
                title={t("projects.noModules.title")}
                description={t("projects.noModules.description")}
              />
            </Panel>
          )}
          <div className="module-grid">
            {modules.map((module) => {
              const findings =
                scan.findings?.filter((f) => f.module === module.path) ?? [];
              const selected = findings.filter((f) => f.kind !== "dependency");
              const dependencies = findings.filter(
                (f) => f.kind === "dependency",
              );
              return (
                <Panel
                  key={module.path}
                  title={module.path === "." ? t("projects.module.root") : module.path}
                  eyebrow={t("projects.module.eyebrow")}
                  action={<Layers3 size={18} aria-hidden="true" />}
                >
                  <div className="technology-list">
                    {module.technologies.map((tech) => (
                      <Badge key={tech} tone="blue">
                        {t("projects.detected", {
                          name: technologyNames[tech] ?? tech,
                        })}
                      </Badge>
                    ))}
                    {!module.technologies.length && (
                      <Badge>{t("projects.configFound")}</Badge>
                    )}
                  </div>
                  <div className="evidence-list">
                    {module.evidence.map((e) => (
                      <p key={e}>
                        <FileCode2 size={14} aria-hidden="true" />
                        <code>{e}</code>
                      </p>
                    ))}
                  </div>
                  {selected.length > 0 && (
                    <div className="finding-list">
                      {selected.map((f, i) => (
                        <div className="finding" key={i}>
                          <div>
                            <strong>{f.name}</strong>
                            <Badge
                              tone={
                                f.status === "declared" ? "blue" : "neutral"
                              }
                            >
                              {f.status === "declared"
                                ? t("projects.finding.declared")
                                : t("projects.finding.detected")}
                            </Badge>
                          </div>
                          {f.value && <code>{f.value}</code>}
                          <small>{f.source}</small>
                          {f.kind === "container_config" && (
                            <small>
                              {t("projects.finding.containerNote")}
                            </small>
                          )}
                        </div>
                      ))}
                    </div>
                  )}
                  {!!dependencies.length && (
                    <details className="details">
                      <summary>
                        {t("projects.deps", { count: dependencies.length })}
                      </summary>
                      <div className="finding-list">
                        {dependencies.map((f, i) => (
                          <div className="finding" key={i}>
                            <strong>{f.name}</strong>
                            <code>
                              {f.value ?? t("projects.noDepVersion")}
                            </code>
                            <small>{f.source}</small>
                          </div>
                        ))}
                      </div>
                    </details>
                  )}
                </Panel>
              );
            })}
          </div>
          <section className="notice">
            <Info size={18} aria-hidden="true" />
            <div>
              <h3>{t("projects.confirms.title")}</h3>
              <p>
                {t("projects.confirms.body")}
              </p>
            </div>
          </section>
        </>
      )}
    </>
  );
}
