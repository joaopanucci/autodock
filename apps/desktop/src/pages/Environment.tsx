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
  checkLabels,
  checkSummary,
  compatibility,
  recommendation,
  toolNames,
  projectName,
} from "../model";
import { Badge, EmptyState, Panel } from "../components/ui";

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
  const comparisons = scan ? compatibility(scan, doctor) : [];
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
          {check
            ? check.name === "docker-engine" && check.status === "available"
              ? "Engine acessível"
              : checkLabels[check.status]
            : "Não verificado"}
        </Badge>
      </div>
    );
  }
  return (
    <>
      <Panel
        title="Diagnóstico do ambiente"
        eyebrow="INSTALAÇÕES E DISPONIBILIDADE"
        action={
          <button
            className="button secondary"
            onClick={diagnose}
            disabled={busy}
          >
            <RefreshCw size={16} aria-hidden="true" />
            {doctor ? "Atualizar diagnóstico" : "Verificar ambiente"}
          </button>
        }
      >
        <p className="section-description">
          Verificações locais de ferramentas e acesso ao Docker Engine. Nenhuma
          instalação ou alteração é executada.
        </p>
        {doctor ? (
          <>
            <div className="diagnostic-meta">
              <Badge tone="blue">{doctor.os}</Badge>
              <span>
                Última verificação:{" "}
                {checkedAt
                  ? new Date(checkedAt).toLocaleTimeString("pt-BR")
                  : "horário não registrado"}
              </span>
              <span>
                {doctor.checks.filter((c) => c.status === "available").length}{" "}
                de {doctor.checks.length} checks responderam
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
                    {check.name === "docker-engine" &&
                    check.status === "available"
                      ? "Engine acessível"
                      : checkLabels[check.status]}
                  </Badge>
                  <p className="tool-version mono">{checkSummary(check)}</p>
                  <p className="recommendation">{recommendation(check)}</p>
                  <details className="details">
                    <summary>Evidência da verificação</summary>
                    <dl>
                      <dt>Executável</dt>
                      <dd>{check.executable ?? "Não resolvido"}</dd>
                      <dt>Código de saída</dt>
                      <dd>{check.exitCode ?? "Não disponível"}</dd>
                      <dt>Duração</dt>
                      <dd>
                        {check.durationMs === undefined
                          ? "Não informada"
                          : `${check.durationMs} ms`}
                      </dd>
                    </dl>
                    {check.stdout && (
                      <>
                        <h4>Saída padrão</h4>
                        <pre>{check.stdout}</pre>
                      </>
                    )}
                    {check.stderr && (
                      <>
                        <h4>Saída de erro</h4>
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
            title="Seu ambiente ainda não foi verificado"
            description="Execute o diagnóstico para consultar versões e ferramentas. Cada verificação tem um prazo limitado; falhas e ausência são apresentadas separadamente."
          />
        )}
      </Panel>
      <div className="requirements-grid">
        <Panel
          title="Execução local"
          eyebrow={scan ? projectName(scan.project) : "REQUISITOS DO PROJETO"}
        >
          {scan && localTools.length ? (
            <>
              <p className="section-description">
                Ferramentas base associadas às tecnologias detectadas. Versões e
                dependências adicionais precisam de evidência no projeto.
              </p>
              {localTools.map(requirement)}
              <p className="card-note">
                Java e Maven não são sondados pelo doctor atual. Scripts de
                inicialização não são executados. Gerenciadores de pacotes
                alternativos não são inferidos.
              </p>
            </>
          ) : (
            <EmptyState
              icon={SlidersHorizontal}
              title="Requisitos não verificados"
              description="Analise um projeto para relacionar as tecnologias detectadas às ferramentas locais."
            />
          )}
        </Panel>
        <Panel title="Execução com Docker" eyebrow="PRÉ-REQUISITOS GERAIS">
          <p className="section-description">
            A disponibilidade das ferramentas não garante que o projeto esteja
            pronto para executar em containers.
          </p>
          {["docker", "compose", "docker-engine"].map(requirement)}
          <p className="card-note">
            {scan
              ? `${(scan.findings ?? []).filter((f) => f.kind === "container_config").length} arquivos Docker/Compose encontrados. Validade e requisitos internos não verificados.`
              : "Configuração Docker do projeto ainda não verificada."}
          </p>
        </Panel>
      </div>
      <Panel title="Compatibilidade de versões" eyebrow="DECLARAÇÃO × SISTEMA">
        {!comparisons.length ? (
          <EmptyState
            icon={ShieldCheck}
            title="Nenhuma comparação disponível"
            description="É necessário um requisito de versão declarado e uma versão instalada verificável. Ausência de resultados não confirma compatibilidade."
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
                      ? "Incompatibilidade confirmada"
                      : c.status === "matches"
                        ? "Versão exata correspondente"
                        : "Não verificado"}
                  </Badge>
                </div>
                <p>
                  <code>{c.declared}</code>
                  <ArrowRight size={14} aria-hidden="true" />
                  <code>{c.installed ?? "Instalação não verificada"}</code>
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
