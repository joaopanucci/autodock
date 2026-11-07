import { useCallback, useEffect, useMemo, useState, useRef } from "react";
import { call } from "../ipc";
import {
  Boxes,
  RefreshCw,
  Search,
  Server,
  Activity,
  AlertCircle,
} from "lucide-react";
import type { DockerContainer, DockerContainersReport } from "../types";
import { Badge, Panel } from "../components/ui";
import { useI18n } from "../i18n";
import type { Translate } from "../i18n";

type Filter = "all" | "running" | "exited";

function statusTone(container: DockerContainer) {
  if (container.health === "unhealthy") return "red" as const;
  if (container.state === "running") return "green" as const;
  if (container.state === "exited") return "amber" as const;
  return "neutral" as const;
}

function statusLabel(container: DockerContainer, t: Translate) {
  if (container.health === "unhealthy") return t("containers.status.unhealthy");
  if (container.health === "healthy") return t("containers.status.healthy");
  if (container.state === "running") return t("containers.status.running");
  if (container.state === "exited") return t("containers.status.exited");
  return container.state || t("containers.status.unknown");
}

function groupName(container: DockerContainer, t: Translate) {
  return container.project || t("containers.group.standalone");
}

export function Containers() {
  const { t, tn, locale } = useI18n();
  const [report, setReport] = useState<DockerContainersReport | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const [search, setSearch] = useState("");
  const [filter, setFilter] = useState<Filter>("all");
  const [updatedAt, setUpdatedAt] = useState<Date | null>(null);
  const mounted = useRef(false);
  const inFlight = useRef(false);

  const refresh = useCallback(async () => {
    if (inFlight.current) return;
    inFlight.current = true;
    setLoading(true);
    setError("");

    try {
      const result = await call<DockerContainersReport>(
        "list_docker_containers",
      );
      if (!mounted.current) return;
      setReport(result);
      setUpdatedAt(new Date());
    } catch (cause) {
      if (!mounted.current) return;
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      inFlight.current = false;
      if (mounted.current) setLoading(false);
    }
  }, []);

  useEffect(() => {
    mounted.current = true;
    void refresh();
    return () => {
      mounted.current = false;
    };
  }, [refresh]);

  const containers = report?.containers ?? [];

  const running = containers.filter(
    (container) => container.state === "running",
  ).length;

  const stopped = containers.filter(
    (container) => container.state === "exited",
  ).length;

  const unhealthy = containers.filter(
    (container) => container.health === "unhealthy",
  ).length;

  const groups = useMemo(() => {
    const filtered = containers.filter((container) => {
      if (filter === "running" && container.state !== "running") return false;

      if (filter === "exited" && container.state !== "exited") return false;

      const query = search.trim().toLowerCase();

      if (!query) return true;

      return [
        container.name,
        container.image,
        container.project,
        container.service,
        container.id,
      ]
        .filter(Boolean)
        .some((value) => value!.toLowerCase().includes(query));
    });

    const grouped = new Map<string, DockerContainer[]>();

    for (const container of filtered) {
      const name = groupName(container, t);
      const current = grouped.get(name) ?? [];
      current.push(container);
      grouped.set(name, current);
    }

    return [...grouped.entries()].sort(([a], [b]) => a.localeCompare(b));
  }, [containers, filter, search, t]);

  const filterButton = (value: Filter, label: string) => (
    <button
      type="button"
      className={`button ${filter === value ? "primary" : "secondary"}`}
      onClick={() => setFilter(value)}
      aria-pressed={filter === value}
      style={{ padding: "9px 14px" }}
    >
      {label}
    </button>
  );

  return (
    <div style={{ display: "grid", gap: 22 }}>
      <div
        style={{
          display: "flex",
          justifyContent: "space-between",
          alignItems: "center",
          gap: 16,
          flexWrap: "wrap",
        }}
      >
        <div>
          <p className="eyebrow">{t("containers.eyebrow")}</p>
          <p className="section-description">
            {t("containers.intro")}
          </p>
          {updatedAt && (
            <small>
              {t("containers.lastQuery", {
                time: updatedAt.toLocaleTimeString(locale),
              })}
            </small>
          )}
        </div>

        <button
          type="button"
          className="button primary"
          disabled={loading}
          onClick={() => void refresh()}
        >
          <RefreshCw size={17} aria-hidden="true" />
          {loading ? t("containers.querying") : t("containers.refresh")}
        </button>
      </div>

      {error && (
        <div className="feedback error" role="alert">
          <AlertCircle size={20} aria-hidden="true" />
          <div>
            <strong>{t("containers.error.title")}</strong>
            <p>{error}</p>
          </div>
        </div>
      )}

      <div
        style={{
          display: "grid",
          gridTemplateColumns: "repeat(auto-fit, minmax(175px, 1fr))",
          gap: 14,
        }}
      >
        {[
          { label: t("containers.metric.total"), value: containers.length, icon: Boxes },
          { label: t("containers.metric.running"), value: running, icon: Activity },
          { label: t("containers.metric.stopped"), value: stopped, icon: Server },
          { label: t("containers.metric.unhealthy"), value: unhealthy, icon: AlertCircle },
        ].map(({ label, value, icon: Icon }) => (
          <div className="metric" key={label}>
            <div className="metric-label">
              <span>{label}</span>
              <Icon size={18} aria-hidden="true" />
            </div>
            <strong>{report ? value : "—"}</strong>
            <p>{t("containers.metric.engine")}</p>
          </div>
        ))}
      </div>

      <Panel title={t("containers.explore.title")} eyebrow={t("containers.explore.eyebrow")}>
        <div
          style={{
            display: "flex",
            gap: 12,
            flexWrap: "wrap",
            alignItems: "center",
            marginBottom: 20,
          }}
        >
          <div style={{ position: "relative", flex: "1 1 220px" }}>
            <Search
              size={17}
              aria-hidden="true"
              style={{
                position: "absolute",
                left: 12,
                top: "50%",
                transform: "translateY(-50%)",
                opacity: 0.65,
              }}
            />
            <input
              aria-label={t("containers.search.label")}
              value={search}
              onChange={(event) => setSearch(event.target.value)}
              placeholder={t("containers.search.placeholder")}
              style={{
                width: "100%",
                padding: "11px 12px 11px 38px",
                borderRadius: 3,
                border: "1px solid var(--line)",
                background: "var(--panel)",
                color: "var(--ink)",
                boxSizing: "border-box",
              }}
            />
          </div>
          {filterButton("all", t("containers.filter.all"))}
          {filterButton("running", t("containers.filter.running"))}
          {filterButton("exited", t("containers.filter.exited"))}
        </div>

        {loading && !report && (
          <p role="status">{t("containers.loading")}</p>
        )}

        {!loading && !error && report && containers.length === 0 && (
          <p>{t("containers.none")}</p>
        )}

        {report && groups.length === 0 && containers.length > 0 && (
          <p>{t("containers.noMatch")}</p>
        )}

        <div style={{ display: "grid", gap: 20 }}>
          {groups.map(([project, items]) => (
            <section
              key={project}
              style={{
                border: "1px solid var(--line)",
                borderRadius: 3,
                overflow: "hidden",
              }}
            >
              <div
                style={{
                  padding: "16px 18px",
                  background: "rgba(30, 34, 40, 0.035)",
                  display: "flex",
                  justifyContent: "space-between",
                  alignItems: "center",
                  gap: 12,
                  flexWrap: "wrap",
                }}
              >
                <div
                  style={{
                    display: "flex",
                    alignItems: "center",
                    gap: 10,
                    minWidth: 0,
                  }}
                >
                  <Boxes size={19} aria-hidden="true" />
                  <strong style={{ overflowWrap: "anywhere" }}>
                    {project}
                  </strong>
                </div>
                <Badge tone="blue">
                  {tn("containers.count", items.length)}
                </Badge>
              </div>

              <div style={{ display: "grid" }}>
                {items.map((container) => (
                  <div
                    key={container.id}
                    style={{
                      padding: "17px 18px",
                      borderTop: "1px solid var(--line)",
                      display: "grid",
                      gap: 9,
                    }}
                  >
                    <div
                      style={{
                        display: "flex",
                        justifyContent: "space-between",
                        gap: 12,
                        alignItems: "center",
                        flexWrap: "wrap",
                      }}
                    >
                      <strong style={{ overflowWrap: "anywhere" }}>
                        {container.name}
                      </strong>
                      <Badge tone={statusTone(container)}>
                        {statusLabel(container, t)}
                      </Badge>
                    </div>

                    <div
                      style={{
                        display: "grid",
                        gridTemplateColumns:
                          "repeat(auto-fit, minmax(180px, 1fr))",
                        gap: 12,
                        fontSize: 13,
                      }}
                    >
                      <div>
                        <span style={{ opacity: 0.6 }}>{t("containers.image")}</span>
                        <div style={{ overflowWrap: "anywhere" }}>
                          {container.image}
                        </div>
                      </div>
                      <div>
                        <span style={{ opacity: 0.6 }}>{t("containers.service")}</span>
                        <div>{container.service || t("containers.standalone")}</div>
                      </div>
                      <div>
                        <span style={{ opacity: 0.6 }}>{t("containers.ports")}</span>
                        <div style={{ overflowWrap: "anywhere" }}>
                          {container.ports || t("containers.noPorts")}
                        </div>
                      </div>
                    </div>

                    <small style={{ opacity: 0.65 }}>
                      {t("containers.idLine", {
                        status: container.status,
                        id: container.id.slice(0, 12),
                      })}
                    </small>
                  </div>
                ))}
              </div>
            </section>
          ))}
        </div>
      </Panel>

      <div className="notice">
        <Server size={18} aria-hidden="true" />
        <p>
          {t("containers.readonly")}
        </p>
      </div>
    </div>
  );
}
