import { useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  AlertTriangle,
  CheckCircle2,
  Clock3,
  Copy,
  LoaderCircle,
  Play,
  ShieldCheck,
  TerminalSquare,
  Trash2,
} from "lucide-react";
import { Badge, Panel } from "../components/ui";
import { parseConsoleContainers } from "../console-format";
import type { ConsoleOperation, ConsoleReport } from "../types";
import { ContainerLogs } from "./ContainerLogs";

type Entry = {
  id: number;
  report: ConsoleReport;
  executedAt: string;
};

const operations: {
  id: ConsoleOperation;
  title: string;
  command: string;
  description: string;
}[] = [
  {
    id: "containers",
    title: "Listar containers",
    command: "docker ps -a --no-trunc --format {{json .}}",
    description: "Consulte todos os containers do Docker Engine.",
  },
  {
    id: "engine_info",
    title: "Docker Engine",
    command: "docker info",
    description: "Consulte as informações do ambiente Docker.",
  },
  {
    id: "docker_version",
    title: "Versão",
    command: "docker version",
    description: "Consulte as versões do cliente e do servidor.",
  },
];

function statusLabel(status: ConsoleReport["status"]) {
  switch (status) {
    case "available":
      return "Concluído";
    case "missing":
      return "Docker indisponível";
    case "timeout":
      return "Tempo esgotado";
    default:
      return "Falha na execução";
  }
}

export function Console() {
  const [area, setArea] = useState<"queries" | "logs">("queries");
  const [operation, setOperation] = useState<ConsoleOperation>("containers");
  const [history, setHistory] = useState<Entry[]>([]);
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [copied, setCopied] = useState(false);
  const [view, setView] = useState<"formatted" | "raw">("formatted");
  const nextId = useRef(0);
  const inFlight = useRef(false);

  const selected = history.find((item) => item.id === selectedId) ?? null;
  const current = operations.find((item) => item.id === operation)!;

  async function execute() {
    if (inFlight.current) return;

    inFlight.current = true;
    setBusy(true);
    setError("");
    setCopied(false);

    try {
      const report = await invoke<ConsoleReport>("execute_console_operation", {
        operation,
      });

      const entry: Entry = {
        id: ++nextId.current,
        report,
        executedAt: new Date().toLocaleTimeString("pt-BR"),
      };

      setHistory((previous) => [entry, ...previous].slice(0, 20));
      setSelectedId(entry.id);
      setView("formatted");
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      inFlight.current = false;
      setBusy(false);
    }
  }

  async function copyOutput() {
    if (!selected) return;

    const report = selected.report;
    const output = [
      `$ ${report.command}`,
      report.stdout,
      report.stderr ? `[stderr]\n${report.stderr}` : "",
      report.detail ? `[detalhes]\n${report.detail}` : "",
    ]
      .filter(Boolean)
      .join("\n\n");

    try {
      await navigator.clipboard.writeText(output);
      setCopied(true);
    } catch {
      setError("Não foi possível copiar a saída.");
    }
  }

  const formatted =
    selected?.report.operation === "containers" &&
    selected.report.status === "available" &&
    !selected.report.truncated &&
    !/truncad/i.test(selected.report.detail)
      ? parseConsoleContainers(selected.report.stdout)
      : null;

  return (
    <div className="console-page">
      <div
        className="console-view-switch"
        role="group"
        aria-label="Área do Console"
      >
        <button
          type="button"
          aria-pressed={area === "queries"}
          className={area === "queries" ? "selected" : ""}
          onClick={() => setArea("queries")}
        >
          Consultas Docker
        </button>
        <button
          type="button"
          aria-pressed={area === "logs"}
          className={area === "logs" ? "selected" : ""}
          onClick={() => setArea("logs")}
        >
          Logs de containers
        </button>
      </div>
      <div hidden={area !== "logs"}>
        <ContainerLogs />
      </div>
      <div hidden={area !== "queries"}>
        <div className="console-intro">
          <div>
            <p className="eyebrow">DOCKER ENGINE LOCAL</p>
            <p className="section-description">
              Consultas controladas, sem alterações em containers.
            </p>
          </div>
          <Badge tone="green">
            <ShieldCheck size={13} aria-hidden="true" />
            Somente leitura
          </Badge>
        </div>

        <Panel title="Selecionar operação" eyebrow="COMANDOS PERMITIDOS">
          <div className="console-operations">
            {operations.map((item) => (
              <button
                key={item.id}
                type="button"
                className={`console-operation ${
                  operation === item.id ? "selected" : ""
                }`}
                onClick={() => setOperation(item.id)}
                aria-pressed={operation === item.id}
                disabled={busy}
              >
                <strong>{item.title}</strong>
                <code>{item.command}</code>
                <span>{item.description}</span>
              </button>
            ))}
          </div>

          <div className="console-action">
            <div>
              <span>Comando selecionado</span>
              <code>$ {current.command}</code>
            </div>
            <button
              type="button"
              className="button primary"
              onClick={() => void execute()}
              disabled={busy}
            >
              {busy ? (
                <LoaderCircle className="spin" size={17} />
              ) : (
                <Play size={17} />
              )}
              {busy ? "Executando..." : "Executar consulta"}
            </button>
          </div>
        </Panel>

        {error && (
          <div className="feedback error" role="alert">
            <AlertTriangle size={19} aria-hidden="true" />
            <div>
              <strong>Não foi possível executar a consulta</strong>
              <p>{error}</p>
            </div>
          </div>
        )}
        {busy && (
          <p role="status" aria-live="polite">
            Executando consulta Docker…
          </p>
        )}

        <div className="console-results">
          <Panel
            title="Saída da operação"
            eyebrow="RESULTADO"
            action={
              selected ? (
                <button
                  type="button"
                  className="button secondary"
                  onClick={() => void copyOutput()}
                >
                  <Copy size={15} aria-hidden="true" />
                  {copied ? "Copiado" : "Copiar"}
                </button>
              ) : undefined
            }
          >
            {selected ? (
              <>
                <div className="console-result-meta">
                  <Badge
                    tone={
                      selected.report.status === "available" ? "green" : "amber"
                    }
                  >
                    {selected.report.status === "available" ? (
                      <CheckCircle2 size={13} />
                    ) : (
                      <AlertTriangle size={13} />
                    )}
                    {statusLabel(selected.report.status)}
                  </Badge>
                  <span>
                    <Clock3 size={14} aria-hidden="true" />
                    {selected.report.durationMs} ms
                  </span>
                  <span>{selected.executedAt}</span>
                  <span>Exit code: {selected.report.exitCode ?? "—"}</span>
                </div>
                {selected.report.operation === "containers" &&
                  formatted === null && (
                    <p className="notice warning" role="status">
                      Visualização formatada indisponível: consulta sem sucesso,
                      saída truncada ou JSON inválido. Consulte a saída bruta.
                    </p>
                  )}
                {
                  <div
                    className="console-view-switch"
                    role="group"
                    aria-label="Visualização do resultado"
                  >
                    <button
                      type="button"
                      className={view === "formatted" ? "selected" : ""}
                      aria-pressed={view === "formatted"}
                      disabled={formatted === null}
                      onClick={() => setView("formatted")}
                    >
                      Formatada
                    </button>
                    <button
                      type="button"
                      className={view === "raw" ? "selected" : ""}
                      aria-pressed={view === "raw"}
                      onClick={() => setView("raw")}
                    >
                      Bruta
                    </button>
                  </div>
                }
                {formatted && view === "formatted" ? (
                  <>
                    <div className="console-table-scroll">
                      <table className="console-table">
                        <thead>
                          <tr>
                            <th>Container</th>
                            <th>Imagem</th>
                            <th>Estado</th>
                            <th>Portas</th>
                          </tr>
                        </thead>
                        <tbody>
                          {formatted.length === 0 ? (
                            <tr>
                              <td colSpan={4}>Nenhum container encontrado.</td>
                            </tr>
                          ) : (
                            formatted.map((container) => (
                              <tr key={container.id}>
                                <td title={container.id}>{container.name}</td>
                                <td>{container.image}</td>
                                <td>{container.status || container.state}</td>
                                <td>{container.ports || "—"}</td>
                              </tr>
                            ))
                          )}
                        </tbody>
                      </table>
                    </div>
                    {selected.report.stderr && (
                      <div className="notice warning">
                        <p>stderr: {selected.report.stderr}</p>
                      </div>
                    )}
                    {selected.report.detail && (
                      <p className="card-note">{selected.report.detail}</p>
                    )}
                  </>
                ) : (
                  <div className="console-terminal">
                    <div className="console-terminal-heading">
                      <TerminalSquare size={16} aria-hidden="true" />
                      Saída Docker
                    </div>
                    <pre>
                      <span className="console-command">
                        $ {selected.report.command}
                      </span>
                      {"\n\n"}
                      {selected.report.stdout || "(sem saída padrão)"}
                      {selected.report.stderr && (
                        <>
                          {"\n\n[stderr]\n"}
                          {selected.report.stderr}
                        </>
                      )}
                      {selected.report.detail && (
                        <>
                          {"\n\n[detalhes]\n"}
                          {selected.report.detail}
                        </>
                      )}
                    </pre>
                  </div>
                )}
              </>
            ) : (
              <div className="console-empty">
                <TerminalSquare size={28} aria-hidden="true" />
                <strong>Nenhuma consulta executada</strong>
                <p>Selecione uma operação e clique em Executar consulta.</p>
              </div>
            )}
          </Panel>

          <Panel
            title="Histórico"
            eyebrow="SESSÃO ATUAL"
            action={
              history.length > 0 ? (
                <button
                  type="button"
                  className="button secondary"
                  onClick={() => {
                    setHistory([]);
                    setSelectedId(null);
                    setCopied(false);
                  }}
                  disabled={busy}
                >
                  <Trash2 size={15} aria-hidden="true" />
                  Limpar
                </button>
              ) : undefined
            }
          >
            {history.length === 0 ? (
              <p className="section-description">
                As consultas realizadas aparecerão aqui.
              </p>
            ) : (
              <div className="console-history">
                {history.map((entry) => (
                  <button
                    key={entry.id}
                    type="button"
                    className={`console-history-item ${
                      selectedId === entry.id ? "selected" : ""
                    }`}
                    onClick={() => {
                      setSelectedId(entry.id);
                      setCopied(false);
                    }}
                  >
                    <strong>{entry.report.label}</strong>
                    <small>{entry.executedAt}</small>
                    <span>{statusLabel(entry.report.status)}</span>
                  </button>
                ))}
              </div>
            )}
          </Panel>
        </div>

        <div className="notice">
          <ShieldCheck size={18} aria-hidden="true" />
          <p>
            O Console executa apenas operações predefinidas pelo Rust. O
            histórico permanece na memória desta janela, limitado a 20
            consultas. Revise os resultados antes de copiá-los: informações do
            Docker podem conter dados sensíveis.
          </p>
        </div>
      </div>
    </div>
  );
}
