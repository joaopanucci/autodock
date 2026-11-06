import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import {
  AlertTriangle,
  ArrowLeft,
  ArrowRight,
  Check,
  CheckCircle2,
  Copy,
  FolderOpen,
  FolderSearch,
  LoaderCircle,
  Sparkles,
  SquareArrowOutUpRight,
} from "lucide-react";
import type {
  ApplyReport,
  GeneratePlan,
  GenerateRequest,
  IdeInfo,
  FrameworkId,
  ScaffoldPlan,
  ScaffoldReport,
  PackageManager,
  PythonFramework,
  ServiceId,
  StackId,
  StackSuggestion,
} from "../types";
import { Badge, Panel } from "../components/ui";
import { displayPath } from "../model";

const STACKS: {
  id: StackId;
  title: string;
  hint: string;
  port: number;
  versions: string[];
  versionLabel: string;
}[] = [
  {
    id: "nodejs",
    title: "Node.js",
    hint: "Express, Next.js, Vite, NestJS…",
    port: 3000,
    versions: ["22", "24", "20"],
    versionLabel: "Versão do Node.js",
  },
  {
    id: "spring-boot",
    title: "Java / Spring Boot",
    hint: "Projetos Maven com Spring Boot.",
    port: 8080,
    versions: ["21", "17"],
    versionLabel: "Versão do Java",
  },
  {
    id: "python",
    title: "Python",
    hint: "FastAPI, Django ou Flask.",
    port: 8000,
    versions: ["3.12", "3.13", "3.11"],
    versionLabel: "Versão do Python",
  },
  {
    id: "laravel",
    title: "PHP / Laravel",
    hint: "Aplicações Laravel com Composer.",
    port: 8000,
    versions: ["8.4", "8.5", "8.3", "8.2"],
    versionLabel: "Versão do PHP",
  },
];

const SERVICES: { id: ServiceId; title: string; hint: string; group: string }[] =
  [
    { id: "postgres", title: "PostgreSQL", hint: "Banco relacional", group: "Bancos de dados" },
    { id: "mysql", title: "MySQL", hint: "Banco relacional", group: "Bancos de dados" },
    { id: "mongodb", title: "MongoDB", hint: "Banco de documentos", group: "Bancos de dados" },
    { id: "redis", title: "Redis", hint: "Cache e sessões", group: "Cache e filas" },
    { id: "rabbitmq", title: "RabbitMQ", hint: "Fila de mensagens", group: "Cache e filas" },
    { id: "adminer", title: "Adminer", hint: "Interface web para o banco (porta 8088)", group: "Ferramentas de desenvolvimento" },
    { id: "mailhog", title: "MailHog", hint: "Captura e-mails de teste (porta 8025)", group: "Ferramentas de desenvolvimento" },
  ];

const STEPS = ["Projeto", "Tecnologia", "Serviços", "Revisar e criar"];

type Starter = "clean" | FrameworkId;
const STARTERS: Record<StackId, { id: Starter; title: string; hint: string }[]> = {
  nodejs: [
    { id: "clean", title: "Projeto limpo", hint: "Servidor Express mínimo, só o essencial." },
    { id: "vite-react", title: "Vite + React", hint: "Instalador oficial (create-vite), com TypeScript." },
    { id: "vite-vue", title: "Vite + Vue", hint: "Instalador oficial (create-vite), com TypeScript." },
  ],
  "spring-boot": [
    { id: "clean", title: "Projeto limpo", hint: "Spring Boot com Maven e dois endpoints de exemplo." },
  ],
  python: [
    { id: "clean", title: "Projeto limpo", hint: "FastAPI ou Flask mínimo." },
    { id: "django", title: "Django", hint: "Instalador oficial (django-admin startproject)." },
  ],
  laravel: [
    { id: "laravel", title: "Laravel", hint: "Instalador oficial (composer create-project)." },
  ],
};

function suggestName(path: string) {
  const last = path.replace(/[\\/]+$/, "").split(/[\\/]/).pop() ?? "";
  let name = last.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "");
  if (!name || /^[0-9]/.test(name)) name = `app-${name}`.replace(/-$/, "");
  return name.slice(0, 40).replace(/-$/, "");
}
const validName = (value: string) => /^[a-z](?:[a-z0-9]|-(?=[a-z0-9])){0,39}$/.test(value);

function message(error: unknown) {
  return typeof error === "string"
    ? error
    : error instanceof Error
      ? error.message
      : String(error);
}

export function Assistant({
  path,
  setPath,
}: {
  path: string;
  setPath: (value: string) => void;
}) {
  const [step, setStep] = useState(0);
  const [suggestions, setSuggestions] = useState<StackSuggestion[]>([]);
  const [stack, setStack] = useState<StackId | null>(null);
  const [port, setPort] = useState("3000");
  const [version, setVersion] = useState<string | undefined>();
  const [packageManager, setPackageManager] = useState<PackageManager>("npm");
  const [startScript, setStartScript] = useState("start");
  const [framework, setFramework] = useState<PythonFramework>("fastapi");
  const [module, setModule] = useState("");
  const [services, setServices] = useState<ServiceId[]>([]);
  const [plan, setPlan] = useState<GeneratePlan | null>(null);
  const [selected, setSelected] = useState<string[]>([]);
  const [active, setActive] = useState("");
  const [result, setResult] = useState<ApplyReport | null>(null);
  const [busy, setBusy] = useState<"detect" | "preview" | "apply" | null>(null);
  const [error, setError] = useState("");
  const [copied, setCopied] = useState(false);
  const [emptyFolder, setEmptyFolder] = useState(false);
  const [newProject, setNewProject] = useState(false);
  const [ides, setIdes] = useState<IdeInfo[]>([]);
  const [opened, setOpened] = useState("");
  const [starter, setStarter] = useState<Starter>("clean");
  const [projectName, setProjectName] = useState("");
  const [scaffoldPlan, setScaffoldPlan] = useState<ScaffoldPlan | null>(null);
  const [scaffold, setScaffold] = useState<ScaffoldReport | null>(null);
  const [installing, setInstalling] = useState(false);
  const [elapsed, setElapsed] = useState(0);

  const meta = STACKS.find((s) => s.id === stack);
  const isFramework = newProject && starter !== "clean";
  const detected = (id: StackId) => suggestions.find((s) => s.stack === id);
  const nodeScripts = detected("nodejs")?.scripts ?? [];

  function request(): GenerateRequest | null {
    const number = Number(port);
    if (!stack || !path.trim() || !Number.isInteger(number)) return null;
    return {
      path: path.trim(),
      stack,
      port: number,
      runtimeVersion: version,
      packageManager: stack === "nodejs" ? packageManager : undefined,
      startScript:
        stack !== "nodejs"
          ? undefined
          : starter.startsWith("vite")
            ? "dev"
            : newProject
              ? undefined
              : startScript.trim() || undefined,
      pythonFramework:
        stack === "python" ? (starter === "django" ? "django" : framework) : undefined,
      pythonModule:
        stack === "python" && !newProject && module.trim() ? module.trim() : undefined,
      services,
      // Com instalador oficial, o projeto já existe quando o Docker é gerado.
      newProject: newProject && starter === "clean" ? true : undefined,
      projectName: newProject && starter === "clean" ? projectName : undefined,
    };
  }

  async function detect(value: string) {
    setBusy("detect");
    setError("");
    try {
      const found = await invoke<StackSuggestion[]>("detect_stacks", { path: value });
      setSuggestions(found);
      setNewProject(false);
      if (found.length) {
        setEmptyFolder(false);
        chooseStack(found[0].stack, found);
      } else {
        setStack(null);
        setEmptyFolder(await invoke<boolean>("inspect_folder", { path: value }));
      }
    } catch (e) {
      setSuggestions([]);
      setStack(null);
      setEmptyFolder(false);
      setError(message(e));
    } finally {
      setBusy(null);
    }
  }

  function chooseStack(id: StackId, list: StackSuggestion[] = suggestions) {
    const info = STACKS.find((s) => s.id === id)!;
    const hint = list.find((s) => s.stack === id);
    setStack(id);
    setStarter(STARTERS[id][0].id);
    setScaffoldPlan(null);
    setPort(String(hint?.vite ? 5173 : info.port));
    setVersion(info.versions[0]);
    setPlan(null);
    setResult(null);
    if (hint?.packageManager) setPackageManager(hint.packageManager);
    if (id === "nodejs") {
      const scripts = hint?.scripts ?? [];
      setStartScript(scripts.includes("start") ? "start" : scripts.includes("dev") ? "dev" : scripts[0] ?? "start");
    }
    if (hint?.pythonFramework) setFramework(hint.pythonFramework);
  }

  async function selectFolder() {
    try {
      const picked = await open({ directory: true, multiple: false, title: "Selecionar projeto" });
      if (typeof picked === "string") {
        setPath(picked);
        setEmptyFolder(false);
        setNewProject(false);
        setScaffold(null);
        setScaffoldPlan(null);
        setPlan(null);
        setResult(null);
        await detect(picked);
      }
    } catch (e) {
      setError(message(e));
    }
  }

  function toggleService(id: ServiceId) {
    setPlan(null);
    setServices((current) =>
      current.includes(id) ? current.filter((s) => s !== id) : [...current, id],
    );
  }

  async function review() {
    const body = request();
    if (!body) {
      setError("Revise a porta informada.");
      return;
    }
    setBusy("preview");
    setError("");
    try {
      if (isFramework && !scaffold?.success) {
        setScaffoldPlan(
          await invoke<ScaffoldPlan>("scaffold_plan", {
            request: {
              path: body.path,
              framework: starter,
              projectName,
              runtimeVersion: stack === "laravel" ? undefined : version,
            },
          }),
        );
        setPlan(null);
        setResult(null);
        setStep(3);
        return;
      }
      const generated = await invoke<GeneratePlan>("preview_docker_setup", { request: body });
      setPlan(generated);
      setSelected(generated.files.filter((f) => !f.exists).map((f) => f.name));
      setActive(generated.files[0]?.name ?? "");
      setResult(null);
      setStep(3);
    } catch (e) {
      setError(message(e));
    } finally {
      setBusy(null);
    }
  }

  async function runScaffold() {
    const body = request();
    if (!body || installing) return;
    setInstalling(true);
    setElapsed(0);
    setError("");
    setScaffold(null);
    try {
      const report = await invoke<ScaffoldReport>("scaffold_run", {
        request: {
          path: body.path,
          framework: starter,
          projectName,
          runtimeVersion: stack === "laravel" ? undefined : version,
        },
      });
      setScaffold(report);
      if (report.success) {
        const generated = await invoke<GeneratePlan>("preview_docker_setup", { request: body });
        setPlan(generated);
        setSelected(generated.files.filter((f) => !f.exists).map((f) => f.name));
        setActive(generated.files[0]?.name ?? "");
        setResult(null);
      }
    } catch (e) {
      setError(message(e));
    } finally {
      setInstalling(false);
    }
  }

  useEffect(() => {
    if (!installing) return;
    const timer = window.setInterval(() => setElapsed((value) => value + 1), 1000);
    return () => window.clearInterval(timer);
  }, [installing]);

  function startNewProject() {
    setProjectName(suggestName(path));
    setScaffold(null);
    setNewProject(true);
    setPlan(null);
    setResult(null);
    chooseStack("nodejs", []);
    setStep(1);
  }

  async function openIn(id: string) {
    setError("");
    try {
      await invoke("open_in_ide", { id, path });
      setOpened(id);
    } catch (e) {
      setError(message(e));
    }
  }

  async function reveal() {
    setError("");
    try {
      await invoke("reveal_folder", { path });
      setOpened("folder");
    } catch (e) {
      setError(message(e));
    }
  }

  async function apply() {
    const body = request();
    if (!body || !selected.length) return;
    setBusy("apply");
    setError("");
    try {
      const report = await invoke<ApplyReport>("apply_docker_setup", {
        // The seed and ports come from the preview, so the written files are identical.
        request: { ...body, seed: plan?.seed, ports: plan?.ports },
        files: selected,
      });
      setResult(report);
      if (report.written.length) {
        setOpened("");
        setIdes(await invoke<IdeInfo[]>("detect_ides").catch(() => []));
      }
    } catch (e) {
      setError(message(e));
    } finally {
      setBusy(null);
    }
  }

  useEffect(() => {
    setCopied(false);
  }, [result]);

  // O caminho pode chegar de fora (ex.: botão na tela de Projetos) ou ser digitado.
  useEffect(() => {
    const value = path.trim();
    if (!value || stack || step !== 0) return;
    const timer = window.setTimeout(() => void detect(value), 500);
    return () => window.clearTimeout(timer);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [path, stack, step]);

  const grouped = useMemo(() => {
    const groups = new Map<string, typeof SERVICES>();
    for (const service of SERVICES) {
      groups.set(service.group, [...(groups.get(service.group) ?? []), service]);
    }
    return [...groups.entries()];
  }, []);

  const canContinue =
    step === 0 ? !!path.trim() && !!stack : step === 1
        ? !!stack && !!port && (!newProject || validName(projectName))
        : true;
  const activeFile = plan?.files.find((f) => f.name === active);
  const command = "docker compose up --build";

  return (
    <div className="wizard">
      <ol className="wizard-steps" aria-label="Etapas do assistente">
        {STEPS.map((label, index) => (
          <li
            key={label}
            className={index === step ? "current" : index < step ? "done" : ""}
            aria-current={index === step ? "step" : undefined}
          >
            <span>{index < step ? <Check size={14} aria-hidden="true" /> : index + 1}</span>
            {label}
          </li>
        ))}
      </ol>

      {error && (
        <div className="feedback error" role="alert">
          <AlertTriangle size={19} aria-hidden="true" />
          <div>
            <strong>Não foi possível continuar</strong>
            <p>{error}</p>
          </div>
        </div>
      )}

      {step === 0 && (
        <Panel title="Qual projeto você quer colocar no Docker?" eyebrow="PASSO 1 DE 4">
          <p className="section-description">
            Escolha a pasta do projeto. O AutoDock lê apenas os arquivos de configuração para
            sugerir a tecnologia, sem executar nada do seu código.
          </p>
          <div className="path-control">
            <FolderOpen size={18} aria-hidden="true" />
            <input
              aria-label="Pasta do projeto para o Docker"
              value={path}
              onChange={(e) => {
                setPath(e.target.value);
                setStack(null);
                setSuggestions([]);
                setEmptyFolder(false);
                setNewProject(false);
                setScaffold(null);
                setScaffoldPlan(null);
                setPlan(null);
              }}
              placeholder="Selecione uma pasta ou informe o caminho absoluto"
            />
            <button type="button" className="button secondary" onClick={() => void selectFolder()} disabled={!!busy}>
              Selecionar pasta
            </button>
          </div>
          {busy === "detect" && (
            <p className="field-hint">
              <LoaderCircle className="spin" size={13} aria-hidden="true" /> Procurando a tecnologia do projeto…
            </p>
          )}
          {!busy && path.trim() && !suggestions.length && !emptyFolder && !error && (
            <p className="field-hint">
              Nenhuma tecnologia reconhecida automaticamente. Você ainda pode escolher uma no próximo passo.
            </p>
          )}
          {!busy && emptyFolder && (
            <div className="empty-folder" role="status">
              <Sparkles size={22} aria-hidden="true" />
              <div>
                <strong>Esta pasta está vazia.</strong>
                <p>
                  Quer que o AutoDock crie um projeto novo aqui? Ele monta um projeto inicial
                  mínimo, já pronto para rodar no Docker.
                </p>
              </div>
              <button type="button" className="button primary" onClick={startNewProject}>
                Criar projeto novo
              </button>
            </div>
          )}
          {suggestions.length > 0 && (
            <p className="wizard-detected">
              <CheckCircle2 size={16} aria-hidden="true" /> Encontramos:{" "}
              {suggestions
                .map((s) => `${STACKS.find((x) => x.id === s.stack)?.title} (${s.evidence})`)
                .join(", ")}
            </p>
          )}
        </Panel>
      )}

      {step === 1 && (
        <Panel
          title={newProject ? "Com que tecnologia você quer criar o projeto?" : "Como o projeto é feito?"}
          eyebrow="PASSO 2 DE 4"
        >
          <div className="choice-grid" role="radiogroup" aria-label="Tecnologia">
            {STACKS.map((s) => (
              <button
                key={s.id}
                type="button"
                role="radio"
                aria-checked={stack === s.id}
                className={`choice ${stack === s.id ? "selected" : ""}`}
                onClick={() => chooseStack(s.id)}
              >
                <strong>{s.title}</strong>
                <span>{s.hint}</span>
                {detected(s.id) && <Badge tone="green">Detectado</Badge>}
              </button>
            ))}
          </div>
          {newProject && meta && (
            <>
              <div className="question-grid">
                <label className="question">
                  <span>Nome do projeto</span>
                  <input
                    value={projectName}
                    onChange={(e) => setProjectName(e.target.value.toLowerCase())}
                    aria-invalid={!validName(projectName)}
                  />
                  <small>
                    {validName(projectName)
                      ? "Usado nos arquivos do projeto. Os arquivos são criados na pasta escolhida."
                      : "Use letras minúsculas, números e hífen, começando por uma letra."}
                  </small>
                </label>
              </div>
              <p className="field-label">Como começar?</p>
              <div className="choice-grid" role="radiogroup" aria-label="Como começar">
                {STARTERS[stack!].map((option) => (
                  <button
                    key={option.id}
                    type="button"
                    role="radio"
                    aria-checked={starter === option.id}
                    className={`choice ${starter === option.id ? "selected" : ""}`}
                    onClick={() => {
                      setStarter(option.id);
                      setPort(option.id.startsWith("vite") ? "5173" : String(meta.port));
                      setPlan(null);
                      setScaffoldPlan(null);
                    }}
                  >
                    <strong>{option.title}</strong>
                    <span>{option.hint}</span>
                    {option.id !== "clean" && <Badge tone="blue">Instalador oficial</Badge>}
                  </button>
                ))}
              </div>
            </>
          )}
          {meta && (
            <div className="question-grid">
              <label className="question">
                <span>Porta da aplicação</span>
                <input
                  inputMode="numeric"
                  value={port}
                  onChange={(e) => {
                    setPort(e.target.value.replace(/\D/g, ""));
                    setPlan(null);
                  }}
                />
                <small>Porta preferida. Se estiver ocupada ou reservada por outro projeto, o AutoDock escolhe a próxima livre.</small>
              </label>
              <label className="question">
                <span>{meta.versionLabel}</span>
                <select
                  value={version}
                  onChange={(e) => {
                    setVersion(e.target.value);
                    setPlan(null);
                  }}
                >
                  {meta.versions.map((v) => (
                    <option key={v}>{v}</option>
                  ))}
                </select>
              </label>
              {stack === "nodejs" && (
                <>
                  <label className="question">
                    <span>Gerenciador de pacotes</span>
                    <select value={packageManager} onChange={(e) => setPackageManager(e.target.value as PackageManager)}>
                      <option>npm</option>
                      <option>pnpm</option>
                      <option>yarn</option>
                    </select>
                  </label>
                  {!newProject && (
                  <label className="question">
                    <span>Script que inicia o app</span>
                    {nodeScripts.length ? (
                      <select value={startScript} onChange={(e) => setStartScript(e.target.value)}>
                        {nodeScripts.map((s) => (
                          <option key={s}>{s}</option>
                        ))}
                      </select>
                    ) : (
                      <input value={startScript} onChange={(e) => setStartScript(e.target.value)} />
                    )}
                    <small>Será executado como “{packageManager} run {startScript || "start"}”.</small>
                  </label>
                  )}
                </>
              )}
              {stack === "python" && starter === "clean" && (
                <>
                  <label className="question">
                    <span>Framework</span>
                    <select value={framework} onChange={(e) => setFramework(e.target.value as PythonFramework)}>
                      <option value="fastapi">FastAPI</option>
                      {!newProject && <option value="django">Django</option>}
                      <option value="flask">Flask</option>
                    </select>
                  </label>
                  {framework !== "django" && !newProject && (
                    <label className="question">
                      <span>Módulo da aplicação</span>
                      <input
                        value={module}
                        onChange={(e) => setModule(e.target.value)}
                        placeholder={framework === "fastapi" ? "main:app" : "app"}
                      />
                      <small>Deixe em branco para usar o padrão.</small>
                    </label>
                  )}
                </>
              )}
            </div>
          )}
        </Panel>
      )}

      {step === 2 && (
        <Panel title="De que mais o projeto precisa?" eyebrow="PASSO 3 DE 4">
          <p className="section-description">
            Cada item vira um container ao lado da sua aplicação, já configurado e conectado. Pode pular se não precisar.
          </p>
          {grouped.map(([group, items]) => (
            <fieldset className="service-group" key={group}>
              <legend>{group}</legend>
              <div className="choice-grid">
                {items.map((s) => (
                  <label key={s.id} className={`choice ${services.includes(s.id) ? "selected" : ""}`}>
                    <input
                      type="checkbox"
                      checked={services.includes(s.id)}
                      onChange={() => toggleService(s.id)}
                    />
                    <strong>{s.title}</strong>
                    <span>{s.hint}</span>
                  </label>
                ))}
              </div>
            </fieldset>
          ))}
        </Panel>
      )}

      {step === 3 && isFramework && !plan && scaffoldPlan && (
        <Panel title={`Instalar ${scaffoldPlan.title}`} eyebrow="PASSO 4 DE 4">
          <p className="section-description">
            O AutoDock vai rodar o instalador oficial dentro de um container Docker, sem instalar
            nada no seu computador. Este é o comando exato:
          </p>
          <pre className="code-preview" aria-label="Comando que será executado">
            <code>{scaffoldPlan.command}</code>
          </pre>
          <div className="notice">
            <ul>
              {scaffoldPlan.notes.map((n) => (
                <li key={n}>{n}</li>
              ))}
            </ul>
          </div>
          {installing && (
            <div className="feedback loading" role="status">
              <LoaderCircle className="spin" size={18} aria-hidden="true" />
              <span>
                Instalando… {Math.floor(elapsed / 60)}:{String(elapsed % 60).padStart(2, "0")}. Na
                primeira vez pode levar alguns minutos (baixa imagens e dependências).
              </span>
            </div>
          )}
          {scaffold && !scaffold.success && (
            <div className="feedback error" role="alert">
              <AlertTriangle size={19} aria-hidden="true" />
              <div>
                <strong>{scaffold.detail}</strong>
                <pre className="code-preview">
                  <code>{scaffold.output}</code>
                </pre>
              </div>
            </div>
          )}
        </Panel>
      )}

      {step === 3 && plan && (
        <Panel title={newProject ? "Revise o projeto novo antes de criar" : "Revise antes de criar"} eyebrow="PASSO 4 DE 4">
          {scaffold?.success && (
            <div className="feedback success" role="status">
              <CheckCircle2 size={18} aria-hidden="true" />
              <span>
                Projeto criado com {scaffoldPlan?.title ?? "o instalador oficial"} em{" "}
                {Math.max(1, Math.round(scaffold.durationMs / 1000))} s. Falta gerar os arquivos
                Docker.
              </span>
            </div>
          )}
          <p className="section-description">
            Nada foi gravado ainda. Estes arquivos serão criados em{" "}
            <span className="mono">{displayPath(plan.project)}</span>. Arquivos que já existem nunca são sobrescritos.
          </p>
          <ul className="file-list">
            {plan.files.map((file) => (
              <li key={file.name}>
                <label>
                  <input
                    type="checkbox"
                    disabled={file.exists || !!result}
                    checked={selected.includes(file.name)}
                    onChange={() =>
                      setSelected((c) => (c.includes(file.name) ? c.filter((n) => n !== file.name) : [...c, file.name]))
                    }
                  />
                  <span className="mono">{file.name}</span>
                </label>
                <button type="button" className="link-button" onClick={() => setActive(file.name)}>
                  ver
                </button>
                <small>{file.description}</small>
                <span className="file-badges">
                  {file.sensitive && <Badge tone="red">Contém senhas</Badge>}
                  {file.exists ? <Badge tone="amber">Já existe, será mantido</Badge> : <Badge tone="blue">Novo</Badge>}
                </span>
              </li>
            ))}
          </ul>
          {!!plan.ports?.length && (
            <div className="port-box">
              <p className="field-label">Portas no seu computador</p>
              <ul className="port-list">
                {plan.ports.map((item) => (
                  <li key={item.name} className={item.hostPort !== item.preferredPort ? "changed" : ""}>
                    <span>{item.name === "app" ? "Aplicação" : item.name}</span>
                    <code>
                      localhost:{item.hostPort} → {item.containerPort}
                    </code>
                    {item.hostPort !== item.preferredPort && (
                      <small>a {item.preferredPort} estava ocupada ou reservada</small>
                    )}
                  </li>
                ))}
              </ul>
            </div>
          )}
          {activeFile && (
            <pre className="code-preview" aria-label={`Conteúdo de ${activeFile.name}`}>
              <code>{activeFile.content}</code>
            </pre>
          )}
          {plan.notes.length > 0 && (
            <div className="notice">
              <ul>
                {plan.notes.map((n) => (
                  <li key={n}>{n}</li>
                ))}
              </ul>
            </div>
          )}
          {result && (
            <div className="feedback success" role="status">
              <CheckCircle2 size={18} aria-hidden="true" />
              <div>
                <strong>
                  {result.written.length
                    ? `${result.written.length} arquivo(s) criado(s)`
                    : "Nenhum arquivo foi criado"}
                </strong>
                {result.skipped.map((s) => (
                  <p key={s.name}>
                    {s.name}: {s.reason}
                  </p>
                ))}
                {result.written.length > 0 && (
                  <>
                    <p className="open-title">Abrir o projeto em:</p>
                    <div className="open-actions">
                      {ides.map((ide) => (
                        <button
                          key={ide.id}
                          type="button"
                          className="button secondary"
                          onClick={() => void openIn(ide.id)}
                        >
                          <SquareArrowOutUpRight size={15} aria-hidden="true" />
                          {ide.label}
                          {opened === ide.id && <Check size={14} aria-hidden="true" />}
                        </button>
                      ))}
                      <button type="button" className="button secondary" onClick={() => void reveal()}>
                        <FolderSearch size={15} aria-hidden="true" />
                        Mostrar na pasta
                        {opened === "folder" && <Check size={14} aria-hidden="true" />}
                      </button>
                    </div>
                    {!ides.length && (
                      <p className="field-hint">Nenhum editor conhecido foi encontrado neste computador.</p>
                    )}
                    <p>Para subir o ambiente, rode na pasta do projeto:</p>
                    <p className="command">
                      <code>{command}</code>
                      <button
                        type="button"
                        className="link-button"
                        onClick={() =>
                          void navigator.clipboard
                            ?.writeText(command)
                            .then(() => setCopied(true))
                            .catch(() => undefined)
                        }
                      >
                        <Copy size={13} aria-hidden="true" /> {copied ? "Copiado" : "Copiar"}
                      </button>
                    </p>
                    {plan?.ports?.find((item) => item.name === "app") && (
                      <p>
                        Depois, acesse{" "}
                        <code>http://localhost:{plan.ports.find((item) => item.name === "app")!.hostPort}</code>
                      </p>
                    )}
                  </>
                )}
              </div>
            </div>
          )}
        </Panel>
      )}

      <div className="wizard-actions">
        <button
          type="button"
          className="button secondary"
          disabled={step === 0 || !!busy || installing}
          onClick={() => setStep((s) => s - 1)}
        >
          <ArrowLeft size={16} aria-hidden="true" /> Voltar
        </button>
        {step < 2 && (
          <button
            type="button"
            className="button primary"
            disabled={!canContinue || !!busy}
            onClick={() => setStep((s) => s + 1)}
          >
            Continuar <ArrowRight size={16} aria-hidden="true" />
          </button>
        )}
        {step === 2 && (
          <button type="button" className="button primary" disabled={!!busy} onClick={() => void review()}>
            {busy === "preview" ? <LoaderCircle className="spin" size={16} aria-hidden="true" /> : null}
            Revisar <ArrowRight size={16} aria-hidden="true" />
          </button>
        )}
        {step === 3 && isFramework && !plan && (
          <button
            type="button"
            className="button primary"
            disabled={installing}
            onClick={() => void runScaffold()}
          >
            {installing ? <LoaderCircle className="spin" size={16} aria-hidden="true" /> : null}
            {scaffold && !scaffold.success ? "Tentar de novo" : "Instalar e continuar"}
          </button>
        )}
        {step === 3 && plan && !result && (
          <button
            type="button"
            className="button primary"
            disabled={!selected.length || !!busy}
            onClick={() => void apply()}
          >
            {busy === "apply" ? <LoaderCircle className="spin" size={16} aria-hidden="true" /> : null}
            Criar {selected.length} arquivo(s)
          </button>
        )}
      </div>
    </div>
  );
}
