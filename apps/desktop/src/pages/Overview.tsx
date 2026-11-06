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
          title="Projetos na sessão"
          value={projects.length}
          detail="Análises realizadas neste workspace"
        />
        <Metric
          icon={Layers3}
          title="Módulos identificados"
          value={modules}
          detail="Agrupados pelo diretório de origem"
        />
        <Metric
          icon={CheckCheck}
          title="Verificações concluídas"
          value={doctor ? `${available}/${doctor.checks.length}` : "—"}
          detail={
            doctor
              ? "Ferramentas que responderam com sucesso"
              : "Ambiente ainda não verificado"
          }
        />
        <Metric
          icon={Boxes}
          title="Arquivos Docker"
          value={projects.reduce(
            (n, p) =>
              n +
              (p.scan.findings ?? []).filter(
                (f) => f.kind === "container_config",
              ).length,
            0,
          )}
          detail="Presença detectada, execução não verificada"
        />
      </div>
      <div className="overview-grid">
        <Panel
          title="Seus projetos começam aqui"
          eyebrow="DO CÓDIGO AO AMBIENTE"
          className="import-panel"
        >
          <p className="lead">
            Entenda a estrutura do seu projeto antes de preparar o ambiente.
          </p>
          <div className="import-zone">
            <div className="folder-art">
              <FolderGit2 size={36} strokeWidth={1.4} aria-hidden="true" />
            </div>
            <h3>Traga seu projeto para o AutoDock</h3>
            <p>
              Selecione uma pasta local para identificar tecnologias,
              <br className="wide-only" /> módulos e requisitos declarados.
            </p>
            <button
              className="button primary"
              disabled={busy}
              onClick={importProject}
            >
              <FolderPlus size={17} aria-hidden="true" />
              Selecionar projeto
            </button>
            <button
              className="button secondary"
              disabled={busy}
              onClick={() => navigate("setup")}
            >
              <WandSparkles size={17} aria-hidden="true" />
              Preparar Docker com o assistente
            </button>
            <span className="micro-copy">
              <ShieldCheck size={13} aria-hidden="true" />
              A análise não altera nada. O assistente só cria arquivos após sua confirmação
            </span>
          </div>
          <div className="flow">
            <span>
              <b>01</b> Selecione
            </span>
            <ArrowRight size={14} />
            <span>
              <b>02</b> Analise
            </span>
            <ArrowRight size={14} />
            <span>
              <b>03</b> Entenda
            </span>
            <ArrowRight size={14} />
            <span>
              <b>04</b> Dockerize
            </span>
          </div>
        </Panel>
        <Panel
          title="Estado do ambiente"
          eyebrow="FERRAMENTAS ESSENCIAIS"
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
                        ? "Não verificado"
                        : check.status === "available"
                          ? "Verificado"
                          : check.status === "missing"
                            ? "Não encontrado"
                            : "Requer atenção"}
                    </span>
                  </div>
                );
              },
            )}
          </div>
          <p className="card-note">
            Essenciais para avaliar o ambiente. Docker não é necessário para
            analisar arquivos.
          </p>
          <button
            className="button secondary full"
            disabled={busy}
            onClick={diagnose}
          >
            <SlidersHorizontal size={16} aria-hidden="true" />
            {doctor ? "Atualizar diagnóstico" : "Verificar ambiente"}
          </button>
        </Panel>
      </div>
      <div className="overview-bottom">
        <Panel
          title="Projetos analisados"
          eyebrow="NESTA SESSÃO"
          action={
            <button
              className="text-button"
              onClick={() => navigate("projects")}
            >
              Ver projetos <ArrowRight size={14} aria-hidden="true" />
            </button>
          }
        >
          {!projects.length ? (
            <EmptyState
              icon={FolderGit2}
              title="Nenhuma análise por enquanto"
              description="Os projetos que você analisar aparecerão aqui. Os dados ficam apenas nesta sessão."
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
                    {groupModules(p.scan).length} módulos
                  </Badge>
                  <ArrowRight size={16} aria-hidden="true" />
                </button>
              ))}
            </div>
          )}
        </Panel>
        <Panel
          title="Controle em cada etapa"
          eyebrow="PERMISSÕES EXPLÍCITAS"
          className="trust-panel"
        >
          <ShieldCheck size={27} aria-hidden="true" />
          <p>
            A análise lê manifests e reúne evidências. O assistente mostra cada
            arquivo antes de gravar. Você decide.
          </p>
          <ul className="check-list">
            <li>
              <CheckCheck size={15} />
              Sem scripts do projeto
            </li>
            <li>
              <CheckCheck size={15} />
              Sem instalações automáticas
            </li>
            <li>
              <CheckCheck size={15} />
              Sem sobrescrever arquivos existentes
            </li>
          </ul>
          <button className="text-button" onClick={() => navigate("settings")}>
            Conhecer as permissões <ArrowRight size={14} aria-hidden="true" />
          </button>
        </Panel>
      </div>
    </>
  );
}
