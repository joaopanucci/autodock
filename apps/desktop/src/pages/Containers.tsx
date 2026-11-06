import { useCallback, useEffect, useMemo, useState, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
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

type Filter = "all" | "running" | "exited";

function statusTone(container: DockerContainer) {
  if (container.health === "unhealthy") return "red" as const;
  if (container.state === "running") return "green" as const;
  if (container.state === "exited") return "amber" as const;
  return "neutral" as const;
}

function statusLabel(container: DockerContainer) {
  if (container.health === "unhealthy") return "Não saudável";
  if (container.health === "healthy") return "Saudável";
  if (container.state === "running") return "Em execução";
  if (container.state === "exited") return "Parado";
  return container.state || "Desconhecido";
}

function groupName(container: DockerContainer) {
  return container.project || "Containers independentes";
}

export function Containers() {
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
      const result = await invoke<DockerContainersReport>(
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
      const name = groupName(container);
      const current = grouped.get(name) ?? [];
      current.push(container);
      grouped.set(name, current);
    }

    return [...grouped.entries()].sort(([a], [b]) => a.localeCompare(b));
  }, [containers, filter, search]);

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
          <p className="eyebrow">DOCKER ENGINE LOCAL</p>
          <p className="section-description">
            Containers reais encontrados no ambiente Docker.
          </p>
          {updatedAt && (
            <small>
              Última consulta: {updatedAt.toLocaleTimeString("pt-BR")}
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
          {loading ? "Consultando..." : "Atualizar"}
        </button>
      </div>

      {error && (
        <div className="feedback error" role="alert">
          <AlertCircle size={20} aria-hidden="true" />
          <div>
            <strong>Não foi possível consultar o Docker</strong>
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
          { label: "Total", value: containers.length, icon: Boxes },
          { label: "Executando", value: running, icon: Activity },
          { label: "Parados", value: stopped, icon: Server },
          { label: "Não saudáveis", value: unhealthy, icon: AlertCircle },
        ].map(({ label, value, icon: Icon }) => (
          <div className="metric" key={label}>
            <div className="metric-label">
              <span>{label}</span>
              <Icon size={18} aria-hidden="true" />
            </div>
            <strong>{report ? value : "—"}</strong>
            <p>Docker Engine</p>
          </div>
        ))}
      </div>

      <Panel title="Explorar containers" eyebrow="INVENTÁRIO LOCAL">
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
              aria-label="Pesquisar containers"
              value={search}
              onChange={(event) => setSearch(event.target.value)}
              placeholder="Pesquisar nome, imagem ou projeto"
              style={{
                width: "100%",
                padding: "11px 12px 11px 38px",
                borderRadius: 10,
                border: "1px solid #334155",
                background: "#111827",
                color: "#f8fafc",
                boxSizing: "border-box",
              }}
            />
          </div>
          {filterButton("all", "Todos")}
          {filterButton("running", "Executando")}
          {filterButton("exited", "Parados")}
        </div>

        {loading && !report && (
          <p role="status">Consultando Docker Engine...</p>
        )}

        {!loading && !error && report && containers.length === 0 && (
          <p>Nenhum container encontrado neste Docker Engine.</p>
        )}

        {report && groups.length === 0 && containers.length > 0 && (
          <p>Nenhum container corresponde aos filtros.</p>
        )}

        <div style={{ display: "grid", gap: 20 }}>
          {groups.map(([project, items]) => (
            <section
              key={project}
              style={{
                border: "1px solid #334155",
                borderRadius: 14,
                overflow: "hidden",
              }}
            >
              <div
                style={{
                  padding: "16px 18px",
                  background: "rgba(148,163,184,0.06)",
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
                  {items.length} container{items.length !== 1 ? "s" : ""}
                </Badge>
              </div>

              <div style={{ display: "grid" }}>
                {items.map((container) => (
                  <div
                    key={container.id}
                    style={{
                      padding: "17px 18px",
                      borderTop: "1px solid #334155",
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
                        {statusLabel(container)}
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
                        <span style={{ opacity: 0.6 }}>Imagem</span>
                        <div style={{ overflowWrap: "anywhere" }}>
                          {container.image}
                        </div>
                      </div>
                      <div>
                        <span style={{ opacity: 0.6 }}>Serviço</span>
                        <div>{container.service || "Independente"}</div>
                      </div>
                      <div>
                        <span style={{ opacity: 0.6 }}>Portas</span>
                        <div style={{ overflowWrap: "anywhere" }}>
                          {container.ports || "Nenhuma publicada"}
                        </div>
                      </div>
                    </div>

                    <small style={{ opacity: 0.65 }}>
                      {container.status} · ID: {container.id.slice(0, 12)}
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
          Consulta somente leitura. Nenhum container será iniciado,
          interrompido, reiniciado ou removido pelo AutoDock.
        </p>
      </div>
    </div>
  );
}
