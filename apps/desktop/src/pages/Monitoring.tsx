import { useCallback, useEffect, useId, useRef, useState } from "react";
import { call } from "../ipc";
import {
  Activity,
  Cpu,
  HardDrive,
  Network,
  RefreshCw,
  AlertCircle,
} from "lucide-react";
import { Badge, Panel } from "../components/ui";
import { useI18n } from "../i18n";
import type { TranslationKey } from "../i18n/core";
import type { ContainerMetrics, MonitoringReport } from "../types";

type Sample = {
  time: number;
  cpu: number | null;
  memory: number | null;
  rx: number | null;
  tx: number | null;
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

function formatBytes(value: number | null, unavailable: string): string {
  if (value === null || !Number.isFinite(value) || value < 0) {
    return unavailable;
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

function formatPercent(value: number | null, unavailable: string): string {
  return value === null || !Number.isFinite(value)
    ? unavailable
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

type Field = "cpu" | "memory" | "rx" | "tx";

const FIELD_KEY: Record<Field, TranslationKey> = {
  cpu: "monitor.metric.cpu",
  memory: "monitor.metric.memory",
  rx: "monitor.metric.rx",
  tx: "monitor.metric.tx",
};

// Round a byte rate up to a "nice" axis maximum (1, 2, 5 x 10^n).
function niceMax(value: number): number {
  if (!(value > 0)) return 1024;
  const exponent = Math.floor(Math.log10(value));
  const base = 10 ** exponent;
  const fraction = value / base;
  const nice = fraction <= 1 ? 1 : fraction <= 2 ? 2 : fraction <= 5 ? 5 : 10;
  return nice * base;
}

function Trend({
  samples,
  field,
  color,
}: {
  samples: Sample[];
  field: Field;
  color: string;
}) {
  const { t, locale } = useI18n();
  const gradientId = useId().replace(/:/g, "");
  const [hover, setHover] = useState<number | null>(null);
  const isBytes = field === "rx" || field === "tx";

  const valid = samples.filter(
    (sample) =>
      sample[field] !== null &&
      Number.isFinite(sample[field]) &&
      Number.isFinite(sample.time),
  );

  if (valid.length < 2) {
    return (
      <div className="ad-monitor-empty">
        {t("monitor.waitingSamples")}
      </div>
    );
  }

  const width = 600;
  const height = 210;
  const left = isBytes ? 62 : 48;
  const right = 585;
  const top = 12;
  const bottom = 160;

  const firstTime = samples[0].time;
  const lastTime = samples[samples.length - 1].time;
  const duration = Math.max(1, lastTime - firstTime);

  const values = valid.map((sample) => sample[field] as number);
  const peak = Math.max(...values);
  const average = values.reduce((sum, value) => sum + value, 0) / values.length;
  const lowest = Math.min(...values);
  const latest = values[values.length - 1];

  // Docker CPU can exceed 100% on multi-core containers.
  const maximum =
    field === "cpu"
      ? Math.max(100, Math.ceil(peak / 25) * 25)
      : field === "memory"
        ? 100
        : niceMax(peak);

  const format = (value: number) =>
    isBytes
      ? `${formatBytes(value, t("monitor.unavailable"))}/s`
      : formatPercent(value, t("monitor.unavailable"));

  const axis = (value: number) =>
    isBytes
      ? formatBytes(value, t("monitor.unavailable"))
      : `${value.toFixed(0)}%`;

  const x = (time: number) =>
    left + ((time - firstTime) / duration) * (right - left);

  const y = (value: number) => bottom - (value / maximum) * (bottom - top);

  const segments: { x: number; y: number }[][] = [];
  let current: { x: number; y: number }[] = [];

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

    current.push({ x: x(sample.time), y: y(value) });
  });

  if (current.length >= 2) segments.push(current);

  const timeLabel = (time: number) =>
    new Date(time).toLocaleTimeString(locale, {
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
    });

  const points = (segment: { x: number; y: number }[]) =>
    segment.map((p) => `${p.x.toFixed(2)},${p.y.toFixed(2)}`).join(" ");

  const area = (segment: { x: number; y: number }[]) =>
    `${points(segment)} ${segment[segment.length - 1].x.toFixed(2)},${bottom} ${segment[0].x.toFixed(2)},${bottom}`;

  const last = valid[valid.length - 1];
  const hovered = hover !== null && valid[hover] ? valid[hover] : null;

  const moveTo = (clientX: number, element: SVGSVGElement) => {
    const rect = element.getBoundingClientRect();
    if (rect.width <= 0) return;
    const svgX = ((clientX - rect.left) / rect.width) * width;
    let best = 0;
    let distance = Infinity;
    valid.forEach((sample, index) => {
      const d = Math.abs(x(sample.time) - svgX);
      if (d < distance) {
        distance = d;
        best = index;
      }
    });
    setHover(best);
  };

  const step = (delta: number) =>
    setHover((index) => {
      const base = index ?? valid.length - 1;
      return Math.min(valid.length - 1, Math.max(0, base + delta));
    });

  const tipLeft = hovered
    ? Math.min(Math.max(((x(hovered.time) - 0) / width) * 100, 12), 88)
    : 0;

  return (
    <div className="ad-monitor-trend">
      <div className="ad-monitor-stats" aria-hidden="true">
        <span>{t("monitor.stat.min")} <b>{format(lowest)}</b></span>
        <span>{t("monitor.stat.avg")} <b>{format(average)}</b></span>
        <span>{t("monitor.stat.max")} <b>{format(peak)}</b></span>
      </div>
      <div className="ad-monitor-plot">
        <svg
          className="ad-monitor-chart"
          viewBox={`0 0 ${width} ${height}`}
          role="img"
          tabIndex={0}
          aria-label={t("monitor.chartAria", {
            metric: t(FIELD_KEY[field]),
            count: valid.length,
          })}
          preserveAspectRatio="xMidYMid meet"
          onMouseMove={(event) => moveTo(event.clientX, event.currentTarget)}
          onMouseLeave={() => setHover(null)}
          onBlur={() => setHover(null)}
          onKeyDown={(event) => {
            if (event.key === "ArrowLeft") {
              event.preventDefault();
              step(-1);
            } else if (event.key === "ArrowRight") {
              event.preventDefault();
              step(1);
            } else if (event.key === "Escape") {
              setHover(null);
            }
          }}
        >
          <defs>
            <linearGradient id={gradientId} x1="0" x2="0" y1="0" y2="1">
              <stop offset="0%" stopColor={color} stopOpacity="0.38" />
              <stop offset="100%" stopColor={color} stopOpacity="0.02" />
            </linearGradient>
          </defs>

          {[0, 0.25, 0.5, 0.75, 1].map((fraction) => {
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
                  strokeOpacity={fraction === 1 ? 0.35 : 0.14}
                  strokeDasharray={fraction === 1 ? undefined : "4 5"}
                />
                <text
                  x={left - 7}
                  y={lineY + 4}
                  textAnchor="end"
                  fill="currentColor"
                  fontSize="11"
                >
                  {axis(value)}
                </text>
              </g>
            );
          })}

          {segments.map((segment, index) => (
            <g key={index}>
              <polygon points={area(segment)} fill={`url(#${gradientId})`} />
              <polyline
                points={points(segment)}
                fill="none"
                stroke={color}
                strokeWidth="2.5"
                strokeLinecap="round"
                strokeLinejoin="round"
                vectorEffect="non-scaling-stroke"
              />
            </g>
          ))}

          <circle
            cx={x(last.time)}
            cy={y(last[field] as number)}
            r="4.5"
            fill={color}
            stroke="var(--panel, #0f1523)"
            strokeWidth="2"
          />

          {hovered && (
            <g>
              <line
                x1={x(hovered.time)}
                x2={x(hovered.time)}
                y1={top}
                y2={bottom}
                stroke="currentColor"
                strokeOpacity="0.45"
              />
              <circle
                cx={x(hovered.time)}
                cy={y(hovered[field] as number)}
                r="5"
                fill={color}
                stroke="#fff"
                strokeWidth="1.5"
              />
            </g>
          )}

          {[0, 0.5, 1].map((fraction) => (
            <text
              key={fraction}
              x={left + fraction * (right - left)}
              y={height - 14}
              textAnchor={fraction === 0 ? "start" : fraction === 1 ? "end" : "middle"}
              fill="currentColor"
              fontSize="11"
            >
              {timeLabel(firstTime + fraction * duration)}
            </text>
          ))}
        </svg>

        {hovered && (
          <div
            className="ad-monitor-tooltip"
            style={{ left: `${tipLeft}%` }}
            role="status"
          >
            <strong>{format(hovered[field] as number)}</strong>
            <small>{timeLabel(hovered.time)}</small>
          </div>
        )}
      </div>
      <p className="ad-monitor-caption">
        {t("monitor.caption", { count: valid.length, now: format(latest) })}
      </p>
    </div>
  );
}

export function Monitoring() {
  const { t, locale } = useI18n();
  const bytes = (value: number | null) => formatBytes(value, t("monitor.unavailable"));
  const percent = (value: number | null) => formatPercent(value, t("monitor.unavailable"));
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
      const result = await call<MonitoringReport>(
        "collect_container_metrics",
      );

      if (!mounted.current) return;

      if (result.schemaVersion !== 1 || !Array.isArray(result.containers)) {
        throw new Error(t("monitor.error.format"));
      }

      const now = result.collectedAtUnixMs;

      if (!Number.isSafeInteger(now) || now <= 0 || now > Date.now() + 60_000) {
        throw new Error(t("monitor.error.timestamp"));
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
              rx: nextRates[metric.id]?.rx ?? null,
              tx: nextRates[metric.id]?.tx ?? null,
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
          <p className="eyebrow">{t("monitor.eyebrow")}</p>
          <p className="section-description">
            {t("monitor.intro")}
          </p>
          <small>
            {updatedAt
              ? t("monitor.lastSample", {
                  time: new Date(updatedAt).toLocaleTimeString(locale),
                })
              : t("monitor.noSample")}
            {loading && ` · ${t("monitor.collectingNow")}`}
            {!loading && error && ` · ${t("monitor.previousData")}`}
            {!loading && !error && lastFailureAt && ` · ${t("monitor.checkCollection")}`}
          </small>
        </div>

        <div className="ad-monitor-actions">
          <label className="ad-monitor-auto">
            <input
              type="checkbox"
              checked={automatic}
              onChange={(event) => setAutomatic(event.target.checked)}
            />
            {t("monitor.autoRefresh")}
          </label>

          <select
            aria-label={t("monitor.intervalLabel")}
            value={intervalSeconds}
            onChange={(event) => setIntervalSeconds(Number(event.target.value))}
            disabled={!automatic}
          >
            <option value={5}>{t("monitor.seconds", { n: 5 })}</option>
            <option value={10}>{t("monitor.seconds", { n: 10 })}</option>
            <option value={30}>{t("monitor.seconds", { n: 30 })}</option>
          </select>

          <button
            className="button primary"
            type="button"
            onClick={() => void refresh()}
            disabled={loading}
          >
            <RefreshCw size={16} aria-hidden="true" />
            {loading ? t("containers.querying") : t("containers.refresh")}
          </button>
        </div>
      </div>

      {error && (
        <div className="ad-monitor-error" role="alert">
          <AlertCircle size={19} aria-hidden="true" />
          <div>
            <strong>{t("monitor.failure.title")}</strong>
            <p>{error}</p>
            {report && (
              <p>{t("monitor.failure.note")}</p>
            )}
          </div>
        </div>
      )}

      <div className="ad-monitor-summary">
        <div className="metric">
          <div className="metric-label">
            <span>{t("monitor.summary.monitored")}</span>
            <Activity size={18} aria-hidden="true" />
          </div>
          <strong>{report ? containers.length : "—"}</strong>
          <p>{t("monitor.summary.runningLast")}</p>
        </div>

        <div className="metric">
          <div className="metric-label">
            <span>{t("monitor.summary.memory")}</span>
            <HardDrive size={18} aria-hidden="true" />
          </div>
          <strong>{report && knownMemory ? bytes(totalMemory) : "—"}</strong>
          <p>{t("monitor.summary.memorySum")}</p>
        </div>

        <div className="metric">
          <div className="metric-label">
            <span>{t("monitor.summary.collection")}</span>
            <Cpu size={18} aria-hidden="true" />
          </div>
          <strong>
            {loading
              ? t("monitor.summary.inProgress")
              : automatic
                ? t("monitor.summary.automatic")
                : t("monitor.summary.manual")}
          </strong>
          <p>
            {automatic
              ? t("monitor.summary.interval", { n: intervalSeconds })
              : t("monitor.summary.onDemand")}
          </p>
        </div>
      </div>

      <Panel
        title={t("monitor.perf.title")}
        eyebrow={t("monitor.perf.eyebrow")}
        action={
          <Badge tone={error ? "amber" : report ? "green" : "neutral"}>
            {error
              ? report
                ? t("monitor.previousData")
                : t("monitor.badge.failure")
              : report
                ? t("logs.status.done")
                : t("monitor.badge.waiting")}
          </Badge>
        }
      >
        {!report && !error && (
          <p className="section-description" role="status">
            {t("monitor.perf.loading")}
          </p>
        )}

        {report && containers.length === 0 && (
          <p className="section-description">
            {t("monitor.perf.none")}
          </p>
        )}

        {containers.length > 0 && (
          <>
            <div className="ad-monitor-picker">
              <label htmlFor="ad-monitor-select">{t("logs.container")}</label>
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
                    <span>{t("monitor.memory")}</span>
                    <strong>{percent(current.memoryPercent)}</strong>
                    <small>
                      {bytes(current.memoryUsageBytes)} /{" "}
                      {bytes(current.memoryLimitBytes)}
                    </small>
                  </div>
                  <div>
                    <span>{t("monitor.receiving")}</span>
                    <strong>
                      {currentRates.rx === null
                        ? t("monitor.waitingForSamples")
                        : `${bytes(currentRates.rx)}/s`}
                    </strong>
                    <small>{t("monitor.total", { value: bytes(current.networkRxBytes) })}</small>
                  </div>
                  <div>
                    <span>{t("monitor.sending")}</span>
                    <strong>
                      {currentRates.tx === null
                        ? t("monitor.waitingForSamples")
                        : `${bytes(currentRates.tx)}/s`}
                    </strong>
                    <small>{t("monitor.total", { value: bytes(current.networkTxBytes) })}</small>
                  </div>
                </div>

                <div className="ad-monitor-graphs">
                  <div>
                    <div className="ad-monitor-graph-heading">
                      <strong>{t("monitor.history.cpu")}</strong>
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
                      <strong>{t("monitor.history.memory")}</strong>
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
                  <div>
                    <div className="ad-monitor-graph-heading">
                      <strong>{t("monitor.history.rx")}</strong>
                      <Badge tone="neutral">
                        {currentRates.rx === null ? t("monitor.unavailable") : `${bytes(currentRates.rx)}/s`}
                      </Badge>
                    </div>
                    <Trend samples={currentHistory} field="rx" color="#e0a85c" />
                  </div>
                  <div>
                    <div className="ad-monitor-graph-heading">
                      <strong>{t("monitor.history.tx")}</strong>
                      <Badge tone="neutral">
                        {currentRates.tx === null ? t("monitor.unavailable") : `${bytes(currentRates.tx)}/s`}
                      </Badge>
                    </div>
                    <Trend samples={currentHistory} field="tx" color="#b38cf0" />
                  </div>
                </div>
              </>
            )}
          </>
        )}
      </Panel>

      <Panel title={t("monitor.all.title")} eyebrow={t("monitor.all.eyebrow")}>
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
                <span>{t("monitor.memory")}</span>
                <strong>{bytes(metric.memoryUsageBytes)}</strong>
              </div>
              <div>
                <span>{t("monitor.networkReceived")}</span>
                <strong>{bytes(metric.networkRxBytes)}</strong>
              </div>
              <div>
                <span>{t("monitor.networkSent")}</span>
                <strong>{bytes(metric.networkTxBytes)}</strong>
              </div>
            </div>
          ))}
          {containers.length === 0 && (
            <p className="section-description">
              {t("monitor.noMetrics")}
            </p>
          )}
        </div>
      </Panel>

      <div className="notice">
        <Network size={18} aria-hidden="true" />
        <p>
          {t("monitor.note")}
        </p>
      </div>
    </div>
  );
}
