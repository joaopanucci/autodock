import {
  ArrowRight,
  CheckCircle2,
  CircleHelp,
  RefreshCw,
  ShieldCheck,
  SlidersHorizontal,
} from "lucide-react";
import type { Doctor, Scan } from "../types";
import {
  checkLabel,
  checkSummary,
  compatibility,
  recommendation,
  toolNames,
  projectName,
} from "../model";
import { Badge, EmptyState, Panel } from "../components/ui";
import { useI18n } from "../i18n";

export function Environment({
  doctor,
  scan,
  diagnose,
  busy,
  checkedAt,
}: {
  doctor: Doctor | null;
  scan: Scan | null;
  diagnose: () => void;
  busy: boolean;
  checkedAt: string | null;
}) {
  const { t, locale } = useI18n();
  const comparisons = scan ? compatibility(scan, doctor, t) : [];
  const technologies = new Set(scan?.components.map((c) => c.technology));
  const localTools = [
    ...(technologies.has("nodejs") ? ["node"] : []),
    ...(technologies.has("maven") ? ["java", "mvn"] : []),
  ];
  function requirement(name: string) {
    const check = doctor?.checks.find((c) => c.name === name);
    return (
      <div className="requirement-row" key={name}>
        <strong>
          {toolNames[name] ??
            { java: "Java / JDK", mvn: "Maven" }[name] ??
            name}
        </strong>
        <Badge
          tone={
            check?.status === "available"
              ? "green"
              : check?.status === "missing"
                ? "amber"
                : "neutral"
          }
        >
          {check ? checkLabel(check, t) : t("check.notVerified")}
        </Badge>
      </div>
    );
  }
  return (
    <>
      <Panel
        title={t("env.title")}
        eyebrow={t("env.eyebrow")}
        action={
          <button
            className="button secondary"
            onClick={diagnose}
            disabled={busy}
          >
            <RefreshCw size={16} aria-hidden="true" />
            {doctor ? t("env.update") : t("env.verify")}
          </button>
        }
      >
        <p className="section-description">
          {t("env.intro")}
        </p>
        {doctor ? (
          <>
            <div className="diagnostic-meta">
              <Badge tone="blue">{doctor.os}</Badge>
              <span>
                {t("env.lastCheck", {
                  time: checkedAt
                    ? new Date(checkedAt).toLocaleTimeString(locale)
                    : t("env.noTime"),
                })}
              </span>
              <span>
                {t("env.checksOk", {
                  ok: doctor.checks.filter((c) => c.status === "available")
                    .length,
                  total: doctor.checks.length,
                })}
              </span>
            </div>
            <div className="tool-grid">
              {doctor.checks.map((check) => (
                <article className="tool-card" key={check.name}>
                  <div className="tool-card-heading">
                    <span
                      className={`tool-icon ${check.status === "available" ? "good" : ""}`}
                    >
                      {check.status === "available" ? (
                        <CheckCircle2 size={18} aria-hidden="true" />
                      ) : (
                        <CircleHelp size={18} aria-hidden="true" />
                      )}
                    </span>
                    <h3>{toolNames[check.name] ?? check.name}</h3>
                  </div>
                  <Badge
                    tone={
                      check.status === "available"
                        ? "green"
                        : check.status === "missing"
                          ? "amber"
                          : "red"
                    }
                  >
                    {checkLabel(check, t)}
                  </Badge>
                  <p className="tool-version mono">{checkSummary(check, t)}</p>
                  <p className="recommendation">{recommendation(check, t)}</p>
                  <details className="details">
                    <summary>{t("env.evidence")}</summary>
                    <dl>
                      <dt>{t("env.executable")}</dt>
                      <dd>{check.executable ?? t("env.unresolved")}</dd>
                      <dt>{t("env.exitCode")}</dt>
                      <dd>{check.exitCode ?? t("env.notAvailable")}</dd>
                      <dt>{t("env.duration")}</dt>
                      <dd>
                        {check.durationMs === undefined
                          ? t("env.notReported")
                          : `${check.durationMs} ms`}
                      </dd>
                    </dl>
                    {check.stdout && (
                      <>
                        <h4>{t("env.stdout")}</h4>
                        <pre>{check.stdout}</pre>
                      </>
                    )}
                    {check.stderr && (
                      <>
                        <h4>{t("env.stderr")}</h4>
                        <pre>{check.stderr}</pre>
                      </>
                    )}
                    {check.detail && <p>{check.detail}</p>}
                  </details>
                </article>
              ))}
            </div>
            {doctor.notes.map((note) => (
              <p className="card-note" key={note}>
                {note}
              </p>
            ))}
          </>
        ) : (
          <EmptyState
            icon={SlidersHorizontal}
            title={t("env.empty.title")}
            description={t("env.empty.description")}
          />
        )}
      </Panel>
      <div className="requirements-grid">
        <Panel
          title={t("env.local.title")}
          eyebrow={scan ? projectName(scan.project) : t("env.local.eyebrow")}
        >
          {scan && localTools.length ? (
            <>
              <p className="section-description">
                {t("env.local.intro")}
              </p>
              {localTools.map(requirement)}
              <p className="card-note">
                {t("env.local.note")}
              </p>
            </>
          ) : (
            <EmptyState
              icon={SlidersHorizontal}
              title={t("env.local.empty.title")}
              description={t("env.local.empty.description")}
            />
          )}
        </Panel>
        <Panel title={t("env.docker.title")} eyebrow={t("env.docker.eyebrow")}>
          <p className="section-description">
            {t("env.docker.intro")}
          </p>
          {["docker", "compose", "docker-engine"].map(requirement)}
          <p className="card-note">
            {scan
              ? t("env.docker.files", {
                  count: (scan.findings ?? []).filter(
                    (f) => f.kind === "container_config",
                  ).length,
                })
              : t("env.docker.notChecked")}
          </p>
        </Panel>
      </div>
      <Panel title={t("env.compat.title")} eyebrow={t("env.compat.eyebrow")}>
        {!comparisons.length ? (
          <EmptyState
            icon={ShieldCheck}
            title={t("env.compat.empty.title")}
            description={t("env.compat.empty.description")}
          />
        ) : (
          <div className="comparison-list">
            {comparisons.map((c, i) => (
              <article key={i}>
                <div>
                  <h3>{c.name}</h3>
                  <Badge
                    tone={
                      c.status === "incompatible"
                        ? "red"
                        : c.status === "matches"
                          ? "green"
                          : "neutral"
                    }
                  >
                    {c.status === "incompatible"
                      ? t("env.compat.incompatible")
                      : c.status === "matches"
                        ? t("env.compat.matches")
                        : t("check.notVerified")}
                  </Badge>
                </div>
                <p>
                  <code>{c.declared}</code>
                  <ArrowRight size={14} aria-hidden="true" />
                  <code>{c.installed ?? t("env.compat.notInstalled")}</code>
                </p>
                <small>{c.source}</small>
                <p className="card-note">{c.reason}</p>
              </article>
            ))}
          </div>
        )}
      </Panel>
    </>
  );
}
