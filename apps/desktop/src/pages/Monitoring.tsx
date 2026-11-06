import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  Activity,
  Cpu,
  HardDrive,
  Network,
  RefreshCw,
  AlertCircle,
} from "lucide-react";
import { Badge, Panel } from "../components/ui";
import type { ContainerMetrics, MonitoringReport } from "../types";

type Sample = {
  time: number;
  cpu: number | null;
  memory: number | null;
};

type NetworkRate = {
  rx: number | null;
  tx: number | null;
};

type Snapshot = {
  metric: ContainerMetrics;
  at: number;
};

const MAX_SAMPLES = 40;

function bytes(value: number | null): string {
  if (value === null || !Number.isFinite(value) || value < 0) {
    return "Indisponível";
  }

  if (value < 1024) return `${value.toFixed(0)} B`;

  const units = ["KB", "MB", "GB", "TB"];
  let result = value;
  let index = -1;

  do {
    result /= 1024;
    index++;
  } while (result >= 1024 && index < units.length - 1);

  return `${result.toFixed(result >= 100 ? 0 : 1)} ${units[index]}`;
}

function percent(value: number | null): string {
  return value === null || !Number.isFinite(value)
    ? "Indisponível"
    : `${value.toFixed(2)}%`;
}

function networkRate(
  previous: Snapshot | undefined,
  current: ContainerMetrics,
  now: number,
): NetworkRate {
  if (!previous) return { rx: null, tx: null };

  const seconds = (now - previous.at) / 1000;

  // Não calcular taxas sobre intervalos inválidos ou muito longos.
  if (seconds > 120) return { rx: null, tx: null };

  if (seconds <= 0) return { rx: null, tx: null };

  const rate = (before: number | null, after: number | null) => {
    if (before === null || after === null || after < before) {
      return null;
    }

    return (after - before) / seconds;
  };

  return {
    rx: rate(previous.metric.networkRxBytes, current.networkRxBytes),
    tx: rate(previous.metric.networkTxBytes, current.networkTxBytes),
  };
}

function Trend({
  samples,
  field,
  color,
}: {
  samples: Sample[];
  field: "cpu" | "memory";
  color: string;
}) {
  const valid = samples.filter(
    (sample) =>
      sample[field] !== null &&
      Number.isFinite(sample[field]) &&
      Number.isFinite(sample.time),
  );

  if (valid.length < 2) {
    return (
      <div className="ad-monitor-empty">
        Aguardando pelo menos duas amostras válidas.
      </div>
    );
  }

  const width = 600;
  const height = 190;
  const left = 48;
  const right = 585;
  const top = 12;
  const bottom = 150;

  const firstTime = samples[0].time;
  const lastTime = samples[samples.length - 1].time;
  const duration = Math.max(1, lastTime - firstTime);

  const values = valid.map((sample) => sample[field] as number);

  // CPU Docker pode ultrapassar 100% em containers multicore.
  const maximum =
    field === "cpu"
      ? Math.max(100, Math.ceil(Math.max(...values) / 25) * 25)
      : 100;

  const x = (time: number) =>
    left + ((time - firstTime) / duration) * (right - left);

  const y = (value: number) => bottom - (value / maximum) * (bottom - top);

  const segments: string[][] = [];
  let current: string[] = [];

  samples.forEach((sample) => {
    const value = sample[field];

    if (
      value === null ||
      !Number.isFinite(value) ||
      !Number.isFinite(sample.time)
    ) {
      if (current.length >= 2) segments.push(current);
      current = [];
      return;
    }

    current.push(`${x(sample.time).toFixed(2)},${y(value).toFixed(2)}`);
  });

  if (current.length >= 2) segments.push(current);

  const timeLabel = (time: number) =>
    new Date(time).toLocaleTimeString("pt-BR", {
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
    });

  return (
    <div className="ad-monitor-trend">
      <svg
        className="ad-monitor-chart"
        viewBox={`0 0 ${width} ${height}`}
        role="img"
        aria-label={`Histórico de ${field === "cpu" ? "CPU" : "memória"} com ${valid.length} amostras válidas`}
        preserveAspectRatio="xMidYMid meet"
      >
        {[0, 0.5, 1].map((fraction) => {
          const value = maximum * (1 - fraction);
          const lineY = top + fraction * (bottom - top);

          return (
            <g key={fraction}>
              <line
                x1={left}
                x2={right}
                y1={lineY}
                y2={lineY}
                stroke="currentColor"
                strokeOpacity="0.18"
                strokeDasharray="4 5"
              />
              <text
                x={left - 7}
                y={lineY + 4}
                textAnchor="end"
                fill="currentColor"
                fontSize="11"
              >
                {value.toFixed(0)}%
              </text>
            </g>
          );
        })}

        {segments.map((points, index) => (
          <polyline
            key={index}
            points={points.join(" ")}
            fill="none"
            stroke={color}
            strokeWidth="2.5"
            strokeLinecap="round"
            strokeLinejoin="round"
            vectorEffect="non-scaling-stroke"
          />
        ))}

        <text x={left} y={height - 12} fill="currentColor" fontSize="11">
          {timeLabel(firstTime)}
        </text>

        <text
          x={right}
          y={height - 12}
          textAnchor="end"
          fill="currentColor"
          fontSize="11"
        >
          {timeLabel(lastTime)}
        </text>
      </svg>
    </div>
  );
}

export function Monitoring() {
  const [report, setReport] = useState<MonitoringReport | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const [updatedAt, setUpdatedAt] = useState<number | null>(null);
  const [lastFailureAt, setLastFailureAt] = useState<number | null>(null);
  const [intervalSeconds, setIntervalSeconds] = useState(10);
  const [automatic, setAutomatic] = useState(false);
  const [selected, setSelected] = useState("");
  const [history, setHistory] = useState<Record<string, Sample[]>>({});
  const [rates, setRates] = useState<Record<string, NetworkRate>>({});

  const mounted = useRef(false);
  const busy = useRef(false);
  const snapshots = useRef<Map<string, Snapshot>>(new Map());

  const refresh = useCallback(async () => {
    if (busy.current) return;

    busy.current = true;
    setLoading(true);

    try {
      const result = await invoke<MonitoringReport>(
        "collect_container_metrics",
      );

      if (!mounted.current) return;

      if (result.schemaVersion !== 1 || !Array.isArray(result.containers)) {
        throw new Error("Formato de monitoramento não reconhecido.");
      }

      const now = result.collectedAtUnixMs;

      if (!Number.isSafeInteger(now) || now <= 0 || now > Date.now() + 60_000) {
        throw new Error("Timestamp de coleta inválido.");
      }
      const nextRates: Record<string, NetworkRate> = {};
      const nextSnapshots = new Map<string, Snapshot>();

      for (const metric of result.containers) {
        nextRates[metric.id] = networkRate(
          snapshots.current.get(metric.id),
          metric,
          now,
        );

        nextSnapshots.set(metric.id, { metric, at: now });
      }

      snapshots.current = nextSnapshots;
      setRates(nextRates);
      setReport(result);
      setUpdatedAt(now);
      setLastFailureAt(null);
      setError("");

      setHistory((previous) => {
        const next: Record<string, Sample[]> = {};

        for (const metric of result.containers) {
          const samples = previous[metric.id] ?? [];

          next[metric.id] = [
            ...samples,
            {
              time: now,
              cpu: metric.cpuPercent,
              memory: metric.memoryPercent,
            },
          ].slice(-MAX_SAMPLES);
        }

        return next;
      });

      setSelected((current) =>
        result.containers.some((item) => item.id === current)
          ? current
          : (result.containers[0]?.id ?? ""),
      );
    } catch (cause) {
      if (mounted.current) {
        setError(cause instanceof Error ? cause.message : String(cause));
        setLastFailureAt(Date.now());
      }
    } finally {
      busy.current = false;
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

  useEffect(() => {
    if (!automatic) return;

    const timer = window.setInterval(() => {
      void refresh();
    }, intervalSeconds * 1000);

    return () => window.clearInterval(timer);
  }, [automatic, intervalSeconds, refresh]);

  const containers = report?.containers ?? [];
  const current = containers.find((item) => item.id === selected);
  const currentHistory = current ? (history[current.id] ?? []) : [];
  const currentRates = current
    ? (rates[current.id] ?? { rx: null, tx: null })
    : { rx: null, tx: null };

  const totalMemory = containers.reduce(
    (sum, metric) => sum + (metric.memoryUsageBytes ?? 0),
    0,
  );

  const knownMemory = containers.some(
    (metric) => metric.memoryUsageBytes !== null,
  );

  return (
    <div className="ad-monitor">
      <div className="ad-monitor-toolbar">
        <div>
          <p className="eyebrow">DOCKER ENGINE · SOMENTE LEITURA</p>
          <p className="section-description">
            Monitoramento dos containers em execução.
          </p>
          <small>
            {updatedAt
              ? `Última amostra: ${new Date(updatedAt).toLocaleTimeString("pt-BR")}`
              : "Nenhuma amostra coletada"}
            {loading && " · Coletando agora"}
            {!loading && error && " · Dados anteriores"}
            {!loading && !error && lastFailureAt && " · Verificar coleta"}
          </small>
        </div>

        <div className="ad-monitor-actions">
          <label className="ad-monitor-auto">
            <input
              type="checkbox"
              checked={automatic}
              onChange={(event) => setAutomatic(event.target.checked)}
            />
            Atualização automática
          </label>

          <select
            aria-label="Intervalo de atualização"
            value={intervalSeconds}
            onChange={(event) => setIntervalSeconds(Number(event.target.value))}
            disabled={!automatic}
          >
            <option value={5}>5 segundos</option>
            <option value={10}>10 segundos</option>
            <option value={30}>30 segundos</option>
          </select>

          <button
            className="button primary"
            type="button"
            onClick={() => void refresh()}
            disabled={loading}
          >
            <RefreshCw size={16} aria-hidden="true" />
            {loading ? "Consultando..." : "Atualizar"}
          </button>
        </div>
      </div>

      {error && (
        <div className="ad-monitor-error" role="alert">
          <AlertCircle size={19} aria-hidden="true" />
          <div>
            <strong>Falha na coleta de métricas</strong>
            <p>{error}</p>
            {report && (
              <p>Os dados abaixo pertencem à última consulta bem-sucedida.</p>
            )}
          </div>
        </div>
      )}

      <div className="ad-monitor-summary">
        <div className="metric">
          <div className="metric-label">
            <span>Containers monitorados</span>
            <Activity size={18} aria-hidden="true" />
          </div>
          <strong>{report ? containers.length : "—"}</strong>
          <p>Em execução na última coleta</p>
        </div>

        <div className="metric">
          <div className="metric-label">
            <span>Memória utilizada</span>
            <HardDrive size={18} aria-hidden="true" />
          </div>
          <strong>{report && knownMemory ? bytes(totalMemory) : "—"}</strong>
          <p>Soma dos valores disponíveis</p>
        </div>

        <div className="metric">
          <div className="metric-label">
            <span>Coleta</span>
            <Cpu size={18} aria-hidden="true" />
          </div>
          <strong>
            {loading ? "Em andamento" : automatic ? "Automática" : "Manual"}
          </strong>
          <p>
            {automatic ? `Intervalo de ${intervalSeconds}s` : "Sob demanda"}
          </p>
        </div>
      </div>

      <Panel
        title="Desempenho por container"
        eyebrow="MÉTRICAS REAIS"
        action={
          <Badge tone={error ? "amber" : report ? "green" : "neutral"}>
            {error
              ? report
                ? "Dados anteriores"
                : "Falha na coleta"
              : report
                ? "Consulta concluída"
                : "Aguardando"}
          </Badge>
        }
      >
        {!report && !error && (
          <p className="section-description" role="status">
            Consultando métricas do Docker...
          </p>
        )}

        {report && containers.length === 0 && (
          <p className="section-description">
            Nenhum container em execução retornou métricas.
          </p>
        )}

        {containers.length > 0 && (
          <>
            <div className="ad-monitor-picker">
              <label htmlFor="ad-monitor-select">Container</label>
              <select
                id="ad-monitor-select"
                value={selected}
                onChange={(event) => setSelected(event.target.value)}
              >
                {containers.map((item) => (
                  <option key={item.id} value={item.id}>
                    {item.name}
                  </option>
                ))}
              </select>
            </div>

            {current && (
              <>
                <div className="ad-monitor-values">
                  <div>
                    <span>CPU</span>
                    <strong>{percent(current.cpuPercent)}</strong>
                  </div>
                  <div>
                    <span>Memória</span>
                    <strong>{percent(current.memoryPercent)}</strong>
                    <small>
                      {bytes(current.memoryUsageBytes)} /{" "}
                      {bytes(current.memoryLimitBytes)}
                    </small>
                  </div>
                  <div>
                    <span>Recebimento</span>
                    <strong>
                      {currentRates.rx === null
                        ? "Aguardando amostras"
                        : `${bytes(currentRates.rx)}/s`}
                    </strong>
                    <small>Total: {bytes(current.networkRxBytes)}</small>
                  </div>
                  <div>
                    <span>Envio</span>
                    <strong>
                      {currentRates.tx === null
                        ? "Aguardando amostras"
                        : `${bytes(currentRates.tx)}/s`}
                    </strong>
                    <small>Total: {bytes(current.networkTxBytes)}</small>
                  </div>
                </div>

                <div className="ad-monitor-graphs">
                  <div>
                    <div className="ad-monitor-graph-heading">
                      <strong>Histórico de CPU</strong>
                      <Badge tone="blue">{percent(current.cpuPercent)}</Badge>
                    </div>
                    <Trend
                      samples={currentHistory}
                      field="cpu"
                      color="#6498ff"
                    />
                  </div>
                  <div>
                    <div className="ad-monitor-graph-heading">
                      <strong>Histórico de memória</strong>
                      <Badge tone="green">
                        {percent(current.memoryPercent)}
                      </Badge>
                    </div>
                    <Trend
                      samples={currentHistory}
                      field="memory"
                      color="#4ac6ad"
                    />
                  </div>
                </div>
              </>
            )}
          </>
        )}
      </Panel>

      <Panel title="Todos os containers" eyebrow="ÚLTIMA AMOSTRA">
        <div className="ad-monitor-list">
          {containers.map((metric) => (
            <div className="ad-monitor-list-item" key={metric.id}>
              <div>
                <strong>{metric.name}</strong>
                <small>{metric.id.slice(0, 12)}</small>
              </div>
              <div>
                <span>CPU</span>
                <strong>{percent(metric.cpuPercent)}</strong>
              </div>
              <div>
                <span>Memória</span>
                <strong>{bytes(metric.memoryUsageBytes)}</strong>
              </div>
              <div>
                <span>Rede recebida</span>
                <strong>{bytes(metric.networkRxBytes)}</strong>
              </div>
              <div>
                <span>Rede enviada</span>
                <strong>{bytes(metric.networkTxBytes)}</strong>
              </div>
            </div>
          ))}
          {containers.length === 0 && (
            <p className="section-description">
              Sem métricas disponíveis para exibição.
            </p>
          )}
        </div>
      </Panel>

      <div className="notice">
        <Network size={18} aria-hidden="true" />
        <p>
          As taxas de rede são estimadas pela diferença entre amostras. Os
          totais de rede são acumulados pelo Docker. O histórico é temporário e
          não modifica os containers.
        </p>
      </div>
    </div>
  );
}
