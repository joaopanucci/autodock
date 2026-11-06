import {
  ArrowRight,
  Boxes,
  CheckCheck,
  FolderPlus,
  FolderGit2,
  Layers3,
  ShieldCheck,
  SlidersHorizontal,
  CircleHelp,
  WandSparkles,
  CheckCircle2,
  AlertCircle,
} from "lucide-react";
import type { Doctor, Page, SessionProject } from "../types";
import { displayPath, groupModules, projectName, toolNames } from "../model";
import { Badge, EmptyState, Metric, Panel } from "../components/ui";
import { useI18n } from "../i18n";

export function Overview({
  projects,
  doctor,
  navigate,
  importProject,
  inspect,
  diagnose,
  busy,
}: {
  projects: SessionProject[];
  doctor: Doctor | null;
  navigate: (page: Page) => void;
  importProject: () => void;
  inspect: (project: SessionProject) => void;
  diagnose: () => void;
  busy: boolean;
}) {
  const { t } = useI18n();
  const available = doctor?.checks.filter(
    (c) => c.status === "available",
  ).length;
  const modules = projects.reduce(
    (count, p) => count + groupModules(p.scan).length,
    0,
  );
  return (
    <>
      <div className="metrics-grid">
        <Metric
          icon={FolderGit2}
          title={t("overview.metric.projects.title")}
          value={projects.length}
          detail={t("overview.metric.projects.detail")}
        />
        <Metric
          icon={Layers3}
          title={t("overview.metric.modules.title")}
          value={modules}
          detail={t("overview.metric.modules.detail")}
        />
        <Metric
          icon={CheckCheck}
          title={t("overview.metric.checks.title")}
          value={doctor ? `${available}/${doctor.checks.length}` : "—"}
          detail={
            doctor
              ? t("overview.metric.checks.done")
              : t("overview.metric.checks.pending")
          }
        />
        <Metric
          icon={Boxes}
          title={t("overview.metric.docker.title")}
          value={projects.reduce(
            (n, p) =>
              n +
              (p.scan.findings ?? []).filter(
                (f) => f.kind === "container_config",
              ).length,
            0,
          )}
          detail={t("overview.metric.docker.detail")}
        />
      </div>
      <div className="overview-grid">
        <Panel
          title={t("overview.hero.title")}
          eyebrow={t("overview.hero.eyebrow")}
          className="import-panel"
        >
          <p className="lead">
            {t("overview.hero.lead")}
          </p>
          <div className="import-zone">
            <div className="folder-art">
              <FolderGit2 size={36} strokeWidth={1.4} aria-hidden="true" />
            </div>
            <h3>{t("overview.zone.title")}</h3>
            <p>
              {t("overview.zone.body")}
            </p>
            <button
              className="button primary"
              disabled={busy}
              onClick={() => navigate("setup")}
            >
              <WandSparkles size={17} aria-hidden="true" />
              {t("overview.zone.assistant")}
            </button>
            <button
              className="button secondary"
              disabled={busy}
              onClick={importProject}
            >
              <FolderPlus size={17} aria-hidden="true" />
              {t("overview.zone.select")}
            </button>
            <span className="micro-copy">
              <ShieldCheck size={13} aria-hidden="true" />
              {t("overview.zone.micro")}
            </span>
          </div>
          <div className="flow">
            <span>
              <b>01</b> {t("overview.flow.1")}
            </span>
            <ArrowRight size={14} />
            <span>
              <b>02</b> {t("overview.flow.2")}
            </span>
            <ArrowRight size={14} />
            <span>
              <b>03</b> {t("overview.flow.3")}
            </span>
            <ArrowRight size={14} />
            <span>
              <b>04</b> {t("overview.flow.4")}
            </span>
          </div>
        </Panel>
        <Panel
          title={t("overview.env.title")}
          eyebrow={t("overview.env.eyebrow")}
          action={<SlidersHorizontal size={19} aria-hidden="true" />}
        >
          <div className="environment-mini">
            {["node", "git", "docker", "compose", "docker-engine"].map(
              (name) => {
                const check = doctor?.checks.find((c) => c.name === name);
                const Icon = !check
                  ? CircleHelp
                  : check.status === "available"
                    ? CheckCircle2
                    : AlertCircle;
                return (
                  <div key={name}>
                    <span
                      className={`tool-indicator ${check?.status === "available" ? "good" : ""}`}
                    >
                      <Icon size={17} aria-hidden="true" />
                    </span>
                    <strong>{toolNames[name]}</strong>
                    <span className="mini-status">
                      {!check
                        ? t("check.notVerified")
                        : check.status === "available"
                          ? t("overview.env.verified")
                          : check.status === "missing"
                            ? t("check.missing")
                            : t("overview.env.attention")}
                    </span>
                  </div>
                );
              },
            )}
          </div>
          <p className="card-note">
            {t("overview.env.note")}
          </p>
          <button
            className="button secondary full"
            disabled={busy}
            onClick={diagnose}
          >
            <SlidersHorizontal size={16} aria-hidden="true" />
            {doctor ? t("env.update") : t("env.verify")}
          </button>
        </Panel>
      </div>
      <div className="overview-bottom">
        <Panel
          title={t("overview.bottom.title")}
          eyebrow={t("overview.bottom.eyebrow")}
          action={
            <button
              className="text-button"
              onClick={() => navigate("projects")}
            >
              {t("overview.bottom.view")} <ArrowRight size={14} aria-hidden="true" />
            </button>
          }
        >
          {!projects.length ? (
            <EmptyState
              icon={FolderGit2}
              title={t("overview.bottom.empty.title")}
              description={t("overview.bottom.empty.description")}
            />
          ) : (
            <div className="session-list">
              {projects.map((p) => (
                <button key={p.scan.project} onClick={() => inspect(p)} disabled={busy}>
                  <span className="project-symbol">
                    <FolderGit2 size={19} aria-hidden="true" />
                  </span>
                  <span>
                    <strong>{projectName(p.scan.project)}</strong>
                    <small title={displayPath(p.scan.project)}>
                      {displayPath(p.scan.project)}
                    </small>
                  </span>
                  <Badge tone={p.scan.warnings.length ? "amber" : "green"}>
                    {t("overview.modulesBadge", { count: groupModules(p.scan).length })}
                  </Badge>
                  <ArrowRight size={16} aria-hidden="true" />
                </button>
              ))}
            </div>
          )}
        </Panel>
        <Panel
          title={t("overview.trust.title")}
          eyebrow={t("overview.trust.eyebrow")}
          className="trust-panel"
        >
          <ShieldCheck size={27} aria-hidden="true" />
          <p>
            {t("overview.trust.body")}
          </p>
          <ul className="check-list">
            <li>
              <CheckCheck size={15} />
              {t("overview.trust.1")}
            </li>
            <li>
              <CheckCheck size={15} />
              {t("overview.trust.2")}
            </li>
            <li>
              <CheckCheck size={15} />
              {t("overview.trust.3")}
            </li>
          </ul>
          <button className="text-button" onClick={() => navigate("settings")}>
            {t("overview.trust.link")} <ArrowRight size={14} aria-hidden="true" />
          </button>
        </Panel>
      </div>
    </>
  );
}
