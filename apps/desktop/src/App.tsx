import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import {
  ArrowRight,
  CheckCircle2,
  FolderPlus,
  Info,
  LoaderCircle,
  LockKeyhole,
  ShieldCheck,
  X,
  AlertTriangle,
} from "lucide-react";
import type { Doctor, Page, Scan, SessionProject } from "./types";
import { displayPath, rememberProject } from "./model";
import { Layout, navigation } from "./components/Layout";
import { Badge, EmptyState, Panel } from "./components/ui";
import { Overview } from "./pages/Overview";
import { Projects } from "./pages/Projects";
import { Environment } from "./pages/Environment";
import { Containers } from "./pages/Containers";
import { Monitoring } from "./pages/Monitoring";
import { Console } from "./pages/Console";
import { Assistant } from "./pages/Assistant";

const headings: Record<Page, { title: string; description: string }> = {
  overview: {
    title: "Seu workspace, em foco.",
    description:
      "Conheça seus projetos. Entenda seu ambiente. Prepare o próximo passo.",
  },
  projects: {
    title: "Projetos",
    description:
      "Do diretório às evidências: uma visão clara da estrutura do seu código.",
  },
  setup: {
    title: "Assistente Docker",
    description:
      "Responda algumas perguntas e o AutoDock prepara o Docker do seu projeto. Você revisa tudo antes de gravar.",
  },
  environment: {
    title: "Ambiente",
    description:
      "O que o projeto declara e o que o sistema confirma, sem suposições.",
  },
  containers: {
    title: "Containers",
    description:
      "Um espaço para gerenciar seus ambientes, quando você autorizar.",
  },
  monitoring: {
    title: "Monitoramento",
    description: "Acompanhe CPU, memória e tráfego de rede dos containers Docker.",
  },
  console: {
    title: "Console",
    description: "Execute consultas Docker autorizadas e acompanhe os resultados.",
  },
  settings: {
    title: "Configurações",
    description: "Transparência e controle fazem parte do seu workspace.",
  },
};

export default function App() {
  const [page, setPage] = useState<Page>("overview");
  const [path, setPath] = useState("");
  const [scan, setScan] = useState<Scan | null>(null);
  const [projects, setProjects] = useState<SessionProject[]>([]);
  const [doctor, setDoctor] = useState<Doctor | null>(null);
  const [checkedAt, setCheckedAt] = useState<string | null>(null);
  const [busy, setBusy] = useState<"scan" | "doctor" | "select" | null>(null);
  const [error, setError] = useState("");
  const [message, setMessage] = useState("");
  const working = useRef(false);
  const previousPage = useRef(page);
  useEffect(() => {
    if (previousPage.current === page) return;
    previousPage.current = page;
    document.getElementById("content")?.focus({ preventScroll: true });
    window.scrollTo(0, 0);
  }, [page]);
  async function run(
    kind: NonNullable<typeof busy>,
    task: () => Promise<void>,
  ) {
    if (working.current) return;
    working.current = true;
    setBusy(kind);
    setError("");
    setMessage("");
    try {
      await task();
    } catch (e) {
      setError(
        typeof e === "string" ? e : e instanceof Error ? e.message : String(e),
      );
    } finally {
      setBusy(null);
      working.current = false;
    }
  }
  function selectProject() {
    setPage("projects");
    void run("select", async () => {
      const selected = await open({
        directory: true,
        multiple: false,
        title: "Selecionar projeto",
      });
      if (selected) {
        setPath(selected);
        setScan(null);
        setMessage(
          "Pasta selecionada. Inicie a análise para conhecer o projeto.",
        );
      }
    });
  }
  function analyze() {
    if (!path.trim()) return;
    void run("scan", async () => {
      setScan(null);
      const result = await invoke<Scan>("scan_project", { path: path.trim() });
      setScan(result);
      setProjects((previous) =>
        rememberProject(
          previous,
          { scan: result, analyzedAt: new Date().toISOString() },
          doctor?.os,
        ),
      );
      setMessage(
        result.warnings.length
          ? "Análise concluída com avisos. Consulte as limitações abaixo."
          : "Análise concluída. As evidências estão prontas para consulta.",
      );
    });
  }
  function diagnose() {
    setPage("environment");
    void run("doctor", async () => {
      const report = await invoke<Doctor>("doctor_environment");
      setDoctor(report);
      setCheckedAt(new Date().toISOString());
      setMessage(
        "Diagnóstico concluído. Consulte os resultados de cada ferramenta.",
      );
    });
  }
  function inspect(project: SessionProject) {
    setScan(project.scan);
    setPath(displayPath(project.scan.project));
    setPage("projects");
    setError("");
    setMessage("");
  }
  const nav = navigation.find((item) => item.id === page)!;
  return (
    <Layout page={page} navigate={setPage} projects={projects.length}>
      <div className="page-heading">
        <div>
          <p className="eyebrow">
            WORKSPACE LOCAL <span>/</span>{" "}
            {nav.label.toLocaleUpperCase("pt-BR")}
          </p>
          <h1>{headings[page].title}</h1>
          <p>{headings[page].description}</p>
        </div>
        {page === "overview" && (
          <button
            className="button primary"
            onClick={selectProject}
            disabled={!!busy}
          >
            <FolderPlus size={17} aria-hidden="true" />
            Importar projeto
          </button>
        )}
      </div>
      <div className="feedback-area" aria-live="polite" aria-atomic="true">
        {busy && (
          <div className="feedback loading" role="status">
            <LoaderCircle className="spin" size={18} aria-hidden="true" />
            <span>
              {busy === "scan"
                ? "Analisando manifests e diretórios…"
                : busy === "doctor"
                  ? "Verificando ferramentas… Cada check pode levar até 8 segundos, além da limpeza de processos."
                  : "Aguardando seleção de pasta…"}
            </span>
          </div>
        )}
        {!busy && message && (
          <div className="feedback success" role="status">
            <CheckCircle2 size={18} aria-hidden="true" />
            <span>{message}</span>
            <button
              className="icon-button"
              aria-label="Fechar mensagem"
              onClick={() => setMessage("")}
            >
              <X size={16} />
            </button>
          </div>
        )}
      </div>
      {error && (
        <div className="feedback error" role="alert">
          <AlertTriangle size={19} aria-hidden="true" />
          <div>
            <strong>Não foi possível concluir</strong>
            <p>{error}</p>
            <small>
              Confira o caminho ou o estado da ferramenta e tente novamente.
            </small>
          </div>
          <button
            className="icon-button"
            aria-label="Fechar erro"
            onClick={() => setError("")}
          >
            <X size={16} />
          </button>
        </div>
      )}
      <div className="page-body" aria-busy={!!busy}>
        {page === "overview" && (
          <Overview
            projects={projects}
            doctor={doctor}
            navigate={setPage}
            importProject={selectProject}
            inspect={inspect}
            diagnose={diagnose}
            busy={!!busy}
          />
        )}
        {page === "projects" && (
          <Projects
            path={path}
            setPath={(value) => {
              setPath(value);
              setScan(null);
              setError("");
              setMessage("");
            }}
            scan={scan}
            projects={projects}
            busy={!!busy}
            select={selectProject}
            analyze={analyze}
            inspect={inspect}
          />
        )}
        <div hidden={page !== "setup"}>
          <Assistant path={path} setPath={setPath} />
        </div>
        {page === "environment" && (
          <Environment
            doctor={doctor}
            scan={scan}
            diagnose={diagnose}
            busy={!!busy}
            checkedAt={checkedAt}
          />
        )}
        {page === "containers" && <Containers />}
        {page === "monitoring" && <Monitoring />}
        <div hidden={page !== "console"}><Console /></div>
        {nav.planned && page !== "containers" && page !== "monitoring" && page !== "console" && (
          <Panel
            title={nav.label}
            action={<Badge tone="amber">Em desenvolvimento</Badge>}
          >
            <EmptyState
              icon={nav.icon}
              title="O próximo capítulo do seu workspace"
              description="O console exibirá resultados de operações autorizadas. A execução de comandos e os logs de containers ainda não estão disponíveis."
              action={
                <button
                  className="button secondary"
                  onClick={() => setPage("projects")}
                >
                  Explorar projetos <ArrowRight size={16} aria-hidden="true" />
                </button>
              }
            />
            <div className="notice">
              <LockKeyhole size={18} aria-hidden="true" />
              <p>
                Futuras operações apresentarão os comandos e arquivos afetados
                antes de pedir sua autorização.
              </p>
            </div>
          </Panel>
        )}
        {page === "settings" && (
          <>
            <div className="requirements-grid">
              <Panel title="Aparência" eyebrow="PREFERÊNCIAS">
                <div className="setting-row">
                  <div>
                    <strong>Tema escuro</strong>
                    <p>Grafite e azul, seguindo a identidade AutoDock.</p>
                  </div>
                  <Badge tone="blue">Padrão atual</Badge>
                </div>
                <div className="setting-row">
                  <div>
                    <strong>Redução de movimento</strong>
                    <p>Segue automaticamente a preferência do sistema.</p>
                  </div>
                  <Badge>Automático</Badge>
                </div>
                <p className="card-note">
                  Personalização de tema e outras preferências estão em
                  desenvolvimento.
                </p>
              </Panel>
              <Panel title="Dados da sessão" eyebrow="PRIVACIDADE">
                <ShieldCheck
                  className="accent-icon"
                  size={26}
                  aria-hidden="true"
                />
                <p className="section-description">
                  Projetos analisados e diagnósticos ficam somente na memória
                  desta janela. Fechar o aplicativo encerra a sessão.
                </p>
                <Badge>Sem persistência de preferências</Badge>
              </Panel>
            </div>
            <Panel
              title="Permissões e operações"
              eyebrow="VOCÊ AUTORIZA CADA ALTERAÇÃO"
            >
              <div className="notice">
                <Info size={20} aria-hidden="true" />
                <p>
                  Hoje, o AutoDock analisa projetos e consulta ferramentas em
                  modo somente leitura. Nenhuma instalação automática está
                  disponível.
                </p>
              </div>
              <ol className="permission-steps">
                <li>
                  <span>01</span>
                  <div>
                    <strong>Revisão antes da execução</strong>
                    <p>
                      Apresentar a ação, os comandos previstos e os arquivos
                      afetados.
                    </p>
                  </div>
                </li>
                <li>
                  <span>02</span>
                  <div>
                    <strong>Autorização explícita</strong>
                    <p>
                      Solicitar consentimento antes de efeitos colaterais ou
                      privilégios administrativos.
                    </p>
                  </div>
                </li>
                <li>
                  <span>03</span>
                  <div>
                    <strong>Proteção e registro</strong>
                    <p>
                      Criar backup antes de sobrescrever e registrar o
                      resultado, sem credenciais nos logs.
                    </p>
                  </div>
                </li>
              </ol>
              <p className="card-note">
                Diretriz para operações futuras. Nenhum executor de alterações
                foi implementado nesta etapa.
              </p>
            </Panel>
          </>
        )}
      </div>
    </Layout>
  );
}
