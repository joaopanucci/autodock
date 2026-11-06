import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  AlertTriangle,
  Copy,
  FileText,
  LoaderCircle,
  RefreshCw,
  ShieldCheck,
  Trash2,
} from "lucide-react";
import { Badge, EmptyState, Panel } from "../components/ui";
import { logsCopy, logText } from "../logs-format";
import type {
  DockerContainer,
  DockerContainersReport,
  LogsReport,
} from "../types";

export function ContainerLogs() {
  const [containers, setContainers] = useState<DockerContainer[] | null>(null);
  const [id, setId] = useState("");
  const [tail, setTail] = useState<100 | 300 | 500>(100);
  const [result, setResult] = useState<LogsReport | null>(null);
  const [busy, setBusy] = useState<"list" | "logs" | null>(null);
  const [error, setError] = useState("");
  const [copied, setCopied] = useState(false);
  const inFlight = useRef(false);
  const mounted = useRef(true);
  const generation = useRef(0);

  useEffect(() => {
    mounted.current = true;

    return () => {
      mounted.current = false;
      generation.current += 1;
    };
  }, []);

  const selected = containers?.find((c) => c.id === id);
  function clearResult() {
    setResult(null);
    setCopied(false);
    setError("");
  }
  async function run(
    kind: "list" | "logs",
    task: (isCurrent: () => boolean) => Promise<void>,
  ) {
    if (inFlight.current || !mounted.current) return;

    inFlight.current = true;
    const token = ++generation.current;
    const isCurrent = () =>
      mounted.current && generation.current === token;

    setBusy(kind);
    setError("");
    setCopied(false);

    try {
      await task(isCurrent);
    } catch (cause) {
      if (isCurrent()) {
        setError(cause instanceof Error ? cause.message : String(cause));
      }
    } finally {
      inFlight.current = false;
      if (isCurrent()) {
        setBusy(null);
      }
    }
  }
  function refreshContainers() {
    void run("list", async (isCurrent) => {
      const report = await invoke<DockerContainersReport>(
        "list_docker_containers",
      );
      if (!isCurrent()) return;
      setContainers(report.containers);
      setId((current) =>
        report.containers.some((c) => c.id === current)
          ? current
          : (report.containers[0]?.id ?? ""),
      );
      setResult(null);
    });
  }
  function query() {
    if (!selected) return;
    void run("logs", async (isCurrent) => {
      setResult(null);
      const report = await invoke<LogsReport>("query_container_logs", {
        request: { containerId: selected.id, tail },
      });
      if (!isCurrent()) return;
      if (
        report.schemaVersion !== 1 ||
        report.containerId !== selected.id ||
        report.tail !== tail
      )
        throw new Error("Resposta de logs não corresponde à consulta.");
      setResult(report);
    });
  }
  async function copy() {
    if (!result) return;
    try {
      await navigator.clipboard.writeText(logsCopy(result));
      if (mounted.current) setCopied(true);
    } catch {
      if (!mounted.current) return;
      setError(
        "Não foi possível copiar. Verifique a permissão da área de transferência.",
      );
    }
  }
  return (
    <div className="container-logs">
      <Panel
        title="Logs de containers"
        eyebrow="CONSULTA PONTUAL · SOMENTE LEITURA"
        action={
          <button
            className="button secondary"
            disabled={!!busy}
            onClick={refreshContainers}
          >
            <RefreshCw size={16} aria-hidden="true" />
            {containers ? "Atualizar lista" : "Carregar containers"}
          </button>
        }
      >
        <p className="section-description">
          Selecione um container da lista e consulte os registros existentes.
          Nenhum comando é executado dentro do container.
        </p>
        {containers === null ? (
          <EmptyState
            icon={FileText}
            title="Escolha um container para começar"
            description="Carregue a lista do Docker. Os logs só serão consultados quando você solicitar."
          />
        ) : !containers.length ? (
          <p role="status">
            Nenhum container encontrado. Atualize a lista para verificar
            novamente.
          </p>
        ) : (
          <>
            <div className="logs-controls">
              <div className="logs-field">
                <label htmlFor="logs-container">Container</label>
                <select
                  id="logs-container"
                  value={id}
                  disabled={!!busy}
                  onChange={(e) => {
                    setId(e.target.value);
                    clearResult();
                  }}
                >
                  {containers.map((c) => (
                    <option value={c.id} key={c.id}>
                      {c.name} · {c.state || "estado desconhecido"} ·{" "}
                      {c.id.slice(0, 12)}
                    </option>
                  ))}
                </select>
              </div>
              <div className="logs-field">
                <label htmlFor="logs-tail">Últimas linhas</label>
                <select
                  id="logs-tail"
                  value={tail}
                  disabled={!!busy}
                  onChange={(e) => {
                    setTail(Number(e.target.value) as 100 | 300 | 500);
                    clearResult();
                  }}
                >
                  {[100, 300, 500].map((n) => (
                    <option value={n} key={n}>
                      {n} linhas
                    </option>
                  ))}
                </select>
              </div>
              <button
                className="button primary"
                disabled={!!busy || !selected}
                onClick={query}
              >
                {busy === "logs" ? (
                  <LoaderCircle size={16} className="spin" aria-hidden="true" />
                ) : (
                  <RefreshCw size={16} aria-hidden="true" />
                )}
                {result ? "Atualizar logs" : "Consultar logs"}
              </button>
            </div>
            {selected && (
              <div className="logs-selection">
                <strong>{selected.name}</strong>
                <Badge
                  tone={selected.state === "running" ? "green" : "neutral"}
                >
                  {selected.state || "Estado desconhecido"}
                </Badge>
                <code>{selected.id}</code>
                <small>
                  Estado observado na última listagem; o container pode ter
                  mudado desde então.
                </small>
              </div>
            )}
          </>
        )}
        <p className="card-note">
          Limites: 15 segundos, mais até 2 segundos de limpeza; 128 KiB e até a
          quantidade escolhida de linhas por fluxo. Sem acompanhamento contínuo.
        </p>
      </Panel>
      <div role="status" aria-live="polite">
        {busy ? (
          <p>
            {busy === "list" ? "Carregando containers…" : "Consultando logs…"}
          </p>
        ) : copied ? (
          <p>Resultado copiado.</p>
        ) : null}
      </div>
      {error && (
        <div className="feedback error" role="alert">
          <AlertTriangle size={18} aria-hidden="true" />
          <p>{error}</p>
        </div>
      )}
      {result && (
        <Panel
          title="Resultado da consulta"
          eyebrow="SOMENTE NA MEMÓRIA DESTA JANELA"
          action={
            <div className="logs-actions">
              <button className="button secondary" onClick={() => void copy()}>
                <Copy size={15} aria-hidden="true" />
                {copied ? "Copiado" : "Copiar resultado"}
              </button>
              <button className="button secondary" onClick={clearResult}>
                <Trash2 size={15} aria-hidden="true" />
                Limpar logs
              </button>
            </div>
          }
        >
          <div className="console-result-meta">
            <Badge tone={result.status === "available" ? "green" : "amber"}>
              {result.status === "available"
                ? "Consulta concluída"
                : result.status === "timeout"
                  ? "Tempo esgotado"
                  : result.status === "missing"
                    ? "Docker não encontrado"
                    : result.truncated
                      ? "Saída truncada"
                      : "Erro de execução"}
            </Badge>
            <span>{result.durationMs} ms</span>
            <span>Código de saída: {result.exitCode ?? "—"}</span>
            <span>Últimas {result.tail} linhas solicitadas</span>
          </div>
          {(result.detail || result.truncated) && (
            <div className="notice warning" role="status">
              <AlertTriangle size={17} aria-hidden="true" />
              <p>{result.detail || "Saída truncada pelo limite de captura."}</p>
            </div>
          )}
          {result.status !== "available" && (
            <p className="card-note">
              A consulta não foi concluída integralmente. O conteúdo exibido
              pode ser parcial; atualize a lista ou tente novamente.
            </p>
          )}
          <div className="logs-streams">
            <section aria-label="Saída padrão dos logs">
              <h3>stdout</h3>
              <pre tabIndex={0}>
                {logText(result.stdout) || "(sem saída padrão)"}
              </pre>
            </section>
            <section aria-label="Saída de erro dos logs">
              <h3>stderr</h3>
              <pre tabIndex={0}>
                {logText(result.stderr) || "(sem saída de erro)"}
              </pre>
            </section>
          </div>
          {!result.stdout &&
            !result.stderr &&
            result.status === "available" && (
              <p role="status">O Docker não retornou logs nesta consulta.</p>
            )}
          <p className="card-note">
            Controles de terminal são omitidos. Em containers TTY, Docker pode
            reunir os fluxos em stdout; a ordem relativa entre os dois fluxos
            não é reconstruída.
          </p>
        </Panel>
      )}
      <div className="notice">
        <ShieldCheck size={18} aria-hidden="true" />
        <p>
          Os logs não são gravados em arquivos, histórico do Console ou
          relatórios. Podem conter informações sensíveis: revise antes de
          copiar. Fechar a janela encerra a sessão.
        </p>
      </div>
    </div>
  );
}
