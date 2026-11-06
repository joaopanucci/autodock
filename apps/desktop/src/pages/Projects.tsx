import {
  FileCode2,
  FolderGit2,
  FolderOpen,
  ScanLine,
  Layers3,
  Info,
  AlertTriangle,
  WandSparkles,
} from "lucide-react";
import type { Scan, SessionProject } from "../types";
import {
  displayPath,
  groupModules,
  projectName,
  technologyNames,
} from "../model";
import { Badge, EmptyState, Panel } from "../components/ui";

export function Projects({
  path,
  setPath,
  scan,
  projects,
  busy,
  select,
  analyze,
  inspect,
  prepareDocker,
}: {
  path: string;
  setPath: (value: string) => void;
  scan: Scan | null;
  projects: SessionProject[];
  busy: boolean;
  select: () => void;
  analyze: () => void;
  inspect: (project: SessionProject) => void;
  prepareDocker: () => void;
}) {
  const modules = scan ? groupModules(scan) : [];
  return (
    <>
      <Panel title="Importar um projeto" eyebrow="DIRETÓRIO LOCAL">
        <form
          onSubmit={(event) => {
            event.preventDefault();
            if (path.trim() && !busy) analyze();
          }}
        >
          <label className="field-label" htmlFor="project-path">
            Caminho do projeto
          </label>
          <div className="path-control">
            <FolderOpen size={18} aria-hidden="true" />
            <input
              id="project-path"
              value={path}
              disabled={busy}
              onChange={(event) => setPath(event.target.value)}
              placeholder="Selecione uma pasta ou informe o caminho absoluto"
            />
            <button
              type="button"
              className="button secondary"
              disabled={busy}
              onClick={select}
            >
              Selecionar pasta
            </button>
            <button
              type="submit"
              className="button primary"
              disabled={busy || !path.trim()}
            >
              <ScanLine size={16} aria-hidden="true" />
              Analisar projeto
            </button>
          </div>
          <p className="field-hint">
            Análise somente leitura. Diretórios de dependências, cache e backups
            locais são ignorados.
          </p>
        </form>
      </Panel>
      {projects.length > 0 && (
        <div className="project-tabs" aria-label="Projetos desta sessão">
          {projects.map((p) => (
            <button
              key={p.scan.project}
              className={scan?.project === p.scan.project ? "selected" : ""}
              onClick={() => inspect(p)}
              disabled={busy}
            >
              <FolderGit2 size={15} aria-hidden="true" />
              {projectName(p.scan.project)}
            </button>
          ))}
        </div>
      )}
      {!scan ? (
        <Panel title="Estrutura do projeto">
          <EmptyState
            icon={Layers3}
            title="O próximo passo é analisar"
            description="Escolha um diretório e inicie a análise. As tecnologias serão agrupadas por módulo, com suas evidências de origem."
          />
        </Panel>
      ) : (
        <>
          <div className="results-heading">
            <div>
              <p className="eyebrow">ANÁLISE CONCLUÍDA</p>
              <h2>{projectName(scan.project)}</h2>
              <p className="mono path-text">{displayPath(scan.project)}</p>
            </div>
            <button className="button primary" onClick={prepareDocker} disabled={busy}>
              <WandSparkles size={16} aria-hidden="true" />
              Preparar Docker deste projeto
            </button>
            <Badge tone={scan.warnings.length ? "amber" : "green"}>
              {modules.length}{" "}
              {modules.length === 1
                ? "módulo identificado"
                : "módulos identificados"}
            </Badge>
          </div>
          {!!scan.warnings.length && (
            <section className="notice warning" aria-label="Avisos da análise">
              <AlertTriangle size={20} aria-hidden="true" />
              <div>
                <h3>Análise com avisos</h3>
                <ul>
                  {scan.warnings.map((w, i) => (
                    <li key={i}>{w}</li>
                  ))}
                </ul>
              </div>
            </section>
          )}
          {!modules.length && (
            <Panel title="Resultado">
              <EmptyState
                icon={FileCode2}
                title="Nenhum módulo reconhecido"
                description="A análise terminou, mas não encontrou os manifests suportados. Isso não significa que o diretório não contenha um projeto."
              />
            </Panel>
          )}
          <div className="module-grid">
            {modules.map((module) => {
              const findings =
                scan.findings?.filter((f) => f.module === module.path) ?? [];
              const selected = findings.filter((f) => f.kind !== "dependency");
              const dependencies = findings.filter(
                (f) => f.kind === "dependency",
              );
              return (
                <Panel
                  key={module.path}
                  title={module.path === "." ? "Módulo raiz" : module.path}
                  eyebrow="MÓDULO"
                  action={<Layers3 size={18} aria-hidden="true" />}
                >
                  <div className="technology-list">
                    {module.technologies.map((t) => (
                      <Badge key={t} tone="blue">
                        {technologyNames[t] ?? t} · Detectado
                      </Badge>
                    ))}
                    {!module.technologies.length && (
                      <Badge>Configuração encontrada</Badge>
                    )}
                  </div>
                  <div className="evidence-list">
                    {module.evidence.map((e) => (
                      <p key={e}>
                        <FileCode2 size={14} aria-hidden="true" />
                        <code>{e}</code>
                      </p>
                    ))}
                  </div>
                  {selected.length > 0 && (
                    <div className="finding-list">
                      {selected.map((f, i) => (
                        <div className="finding" key={i}>
                          <div>
                            <strong>{f.name}</strong>
                            <Badge
                              tone={
                                f.status === "declared" ? "blue" : "neutral"
                              }
                            >
                              {f.status === "declared"
                                ? "Declarado pelo projeto"
                                : "Detectado"}
                            </Badge>
                          </div>
                          {f.value && <code>{f.value}</code>}
                          <small>{f.source}</small>
                          {f.kind === "container_config" && (
                            <small>
                              Presença do arquivo; conteúdo e execução não
                              verificados.
                            </small>
                          )}
                        </div>
                      ))}
                    </div>
                  )}
                  {!!dependencies.length && (
                    <details className="details">
                      <summary>
                        Dependências declaradas ({dependencies.length})
                      </summary>
                      <div className="finding-list">
                        {dependencies.map((f, i) => (
                          <div className="finding" key={i}>
                            <strong>{f.name}</strong>
                            <code>
                              {f.value ?? "Versão não declarada diretamente"}
                            </code>
                            <small>{f.source}</small>
                          </div>
                        ))}
                      </div>
                    </details>
                  )}
                </Panel>
              );
            })}
          </div>
          <section className="notice">
            <Info size={18} aria-hidden="true" />
            <div>
              <h3>O que esta análise confirma</h3>
              <p>
                As evidências vêm dos arquivos locais. Versões declaradas não
                comprovam instalação. Herança Maven, ranges de versão e
                configuração de serviços Docker não são resolvidos nesta etapa.
              </p>
            </div>
          </section>
        </>
      )}
    </>
  );
}
