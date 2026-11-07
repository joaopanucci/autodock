import { useEffect, useMemo, useRef, useState } from "react";
import { call } from "../ipc";
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
  Layers,
  LoaderCircle,
  Sparkles,
  SquareArrowOutUpRight,
} from "lucide-react";
import type {
  ApplyReport,
  AppsReport,
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
import { TechLogo, hasLogo } from "../components/TechLogo";
import { displayPath } from "../model";
import { STACKS, SERVICES } from "../stacks";
import { MultiApp } from "./MultiApp";
import { useI18n, type TranslationKey } from "../i18n";


/** Stacks that cannot start a new project from here (existing projects only). */
const EXISTING_ONLY: StackId[] = ["rails"];

const NODE_FRAMEWORK_TITLES = { nextjs: "Next.js", nestjs: "NestJS" } as const;


const STEP_KEYS: TranslationKey[] = [
  "wizard.step.project",
  "wizard.step.technology",
  "wizard.step.services",
  "wizard.step.review",
];

type Starter = "clean" | FrameworkId;
type StarterOption = { id: Starter; title: string | TranslationKey; translateTitle?: boolean; hint: TranslationKey };
const CLEAN = "wizard.starter.clean" as const;
const STARTERS: Record<StackId, StarterOption[]> = {
  nodejs: [
    { id: "clean", title: CLEAN, translateTitle: true, hint: "wizard.starter.node.clean" },
    { id: "vite-react", title: "Vite + React", hint: "wizard.starter.vite" },
    { id: "vite-vue", title: "Vite + Vue", hint: "wizard.starter.vite" },
    { id: "nextjs", title: "Next.js", hint: "wizard.starter.next" },
    { id: "nestjs", title: "NestJS", hint: "wizard.starter.nest" },
  ],
  "spring-boot": [
    { id: "clean", title: CLEAN, translateTitle: true, hint: "wizard.starter.spring.clean" },
  ],
  python: [
    { id: "clean", title: CLEAN, translateTitle: true, hint: "wizard.starter.python.clean" },
    { id: "django", title: "Django", hint: "wizard.starter.django" },
  ],
  laravel: [{ id: "laravel", title: "Laravel", hint: "wizard.starter.laravel" }],
  go: [{ id: "clean", title: CLEAN, translateTitle: true, hint: "wizard.starter.go.clean" }],
  dotnet: [{ id: "clean", title: CLEAN, translateTitle: true, hint: "wizard.starter.dotnet.clean" }],
  rails: [{ id: "clean", title: CLEAN, translateTitle: true, hint: "wizard.starter.node.clean" }],
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
  const { t, tn } = useI18n();
  const [step, setStep] = useState(0);
  const detectedPath = useRef<string | null>(null);
  const [appsReport, setAppsReport] = useState<AppsReport | null>(null);
  const [multi, setMulti] = useState(false);
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
          : starter.startsWith("vite") || starter === "nextjs"
            ? "dev"
            : starter === "nestjs"
              ? "start:dev"
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
    detectedPath.current = value;
    setBusy("detect");
    setError("");
    try {
      // Several apps in one folder (monorepo, microservices): offer to set up all of them.
      const [found, apps] = await Promise.all([
        call<StackSuggestion[]>("detect_stacks", { path: value }),
        call<AppsReport>("detect_apps", { path: value }).catch(() => null),
      ]);
      setSuggestions(found);
      setAppsReport(apps);
      setNewProject(false);
      if (found.length) {
        setEmptyFolder(false);
        chooseStack(found[0].stack, found);
      } else {
        setStack(null);
        setEmptyFolder(await call<boolean>("inspect_folder", { path: value }));
      }
    } catch (e) {
      setSuggestions([]);
      setAppsReport(null);
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
    // NestJS dependencies fail to install with the npm that ships with Node 22.
    setVersion(hint?.nodeFramework === "nestjs" ? "24" : info.versions[0]);
    setPlan(null);
    setResult(null);
    if (hint?.packageManager) setPackageManager(hint.packageManager);
    if (id === "nodejs") {
      const scripts = hint?.scripts ?? [];
      const preferred =
        hint?.nodeFramework === "nextjs" ? "dev" : hint?.nodeFramework === "nestjs" ? "start:dev" : "start";
      setStartScript(
        scripts.includes(preferred)
          ? preferred
          : scripts.includes("start")
            ? "start"
            : scripts.includes("dev")
              ? "dev"
              : scripts[0] ?? "start",
      );
    }
    if (hint?.pythonFramework) setFramework(hint.pythonFramework);
  }

  async function selectFolder() {
    try {
      const picked = await open({ directory: true, multiple: false, title: t("app.dialog.selectProject") });
      if (typeof picked === "string") {
        setPath(picked);
        setAppsReport(null);
        setMulti(false);
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
      setError(t("wizard.error.port"));
      return;
    }
    setBusy("preview");
    setError("");
    try {
      if (isFramework && !scaffold?.success) {
        setScaffoldPlan(
          await call<ScaffoldPlan>("scaffold_plan", {
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
      const generated = await call<GeneratePlan>("preview_docker_setup", { request: body });
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
      const report = await call<ScaffoldReport>("scaffold_run", {
        request: {
          path: body.path,
          framework: starter,
          projectName,
          runtimeVersion: stack === "laravel" ? undefined : version,
        },
      });
      setScaffold(report);
      if (report.success) {
        const generated = await call<GeneratePlan>("preview_docker_setup", { request: body });
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
      await call("open_in_ide", { id, path });
      setOpened(id);
    } catch (e) {
      setError(message(e));
    }
  }

  async function reveal() {
    setError("");
    try {
      await call("reveal_folder", { path });
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
      const report = await call<ApplyReport>("apply_docker_setup", {
        // The seed and ports come from the preview, so the written files are identical.
        request: { ...body, seed: plan?.seed, ports: plan?.ports },
        files: selected,
      });
      setResult(report);
      if (report.written.length) {
        setOpened("");
        setIdes(await call<IdeInfo[]>("detect_ides").catch(() => []));
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
    // A path that was already detected (e.g. picked in the dialog) is not detected twice:
    // the second run could undo what the user did meanwhile.
    if (!value || stack || step !== 0 || detectedPath.current === value) return;
    const timer = window.setTimeout(() => void detect(value), 500);
    return () => window.clearTimeout(timer);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [path, stack, step]);

  const grouped = useMemo(() => {
    const groups = new Map<TranslationKey, typeof SERVICES>();
    for (const service of SERVICES) {
      groups.set(service.group, [...(groups.get(service.group) ?? []), service]);
    }
    return [...groups.entries()];
  }, []);
  const stepLabel = (n: number) => t("wizard.stepOf", { n });

  const canContinue =
    step === 0 ? !!path.trim() && !!stack : step === 1
        ? !!stack && !!port && (!newProject || validName(projectName))
        : true;
  const activeFile = plan?.files.find((f) => f.name === active);
  const command = "docker compose up --build";

  if (multi && appsReport) {
    return <MultiApp path={path} report={appsReport} onBack={() => setMulti(false)} />;
  }

  return (
    <div className="wizard">
      <ol className="wizard-steps" aria-label={t("wizard.steps.aria")}>
        {STEP_KEYS.map((labelKey, index) => (
          <li
            key={labelKey}
            className={index === step ? "current" : index < step ? "done" : ""}
            aria-current={index === step ? "step" : undefined}
          >
            <span>{index < step ? <Check size={14} aria-hidden="true" /> : index + 1}</span>
            {t(labelKey)}
          </li>
        ))}
      </ol>

      {error && (
        <div className="feedback error" role="alert">
          <AlertTriangle size={19} aria-hidden="true" />
          <div>
            <strong>{t("wizard.error.title")}</strong>
            <p>{error}</p>
          </div>
        </div>
      )}

      {step === 0 && (
        <Panel title={t("wizard.s1.title")} eyebrow={stepLabel(1)}>
          <p className="section-description">
            {t("wizard.s1.body")}
          </p>
          <div className="path-control">
            <FolderOpen size={18} aria-hidden="true" />
            <input
              aria-label={t("wizard.s1.pathAria")}
              value={path}
              onChange={(e) => {
                setPath(e.target.value);
                setStack(null);
                setSuggestions([]);
                setAppsReport(null);
                setMulti(false);
                setEmptyFolder(false);
                setNewProject(false);
                setScaffold(null);
                setScaffoldPlan(null);
                setPlan(null);
              }}
              placeholder={t("projects.path.placeholder")}
            />
            <button type="button" className="button secondary" onClick={() => void selectFolder()} disabled={!!busy}>
              {t("projects.selectFolder")}
            </button>
          </div>
          {busy === "detect" && (
            <p className="field-hint">
              <LoaderCircle className="spin" size={13} aria-hidden="true" /> {t("wizard.s1.detecting")}
            </p>
          )}
          {!busy && path.trim() && !suggestions.length && !emptyFolder && !error && (
            <p className="field-hint">
              {t("wizard.s1.none")}
            </p>
          )}
          {!busy && emptyFolder && (
            <div className="empty-folder" role="status">
              <Sparkles size={22} aria-hidden="true" />
              <div>
                <strong>{t("wizard.s1.empty.title")}</strong>
                <p>
                  {t("wizard.s1.empty.body")}
                </p>
              </div>
              <button type="button" className="button primary" onClick={startNewProject}>
                {t("wizard.s1.empty.button")}
              </button>
            </div>
          )}
          {appsReport && appsReport.apps.length >= 2 && !newProject && (
            <div className="multi-banner" role="status">
              <Layers size={22} aria-hidden="true" />
              <div>
                <strong>{tn("multi.banner.title", appsReport.apps.length)}</strong>
                <p>{t("multi.banner.body")}</p>
                <small>{t("multi.banner.single")}</small>
              </div>
              <button type="button" className="button primary" onClick={() => setMulti(true)}>
                {t("multi.banner.button")}
              </button>
            </div>
          )}
          {suggestions.length > 0 && (
            <p className="wizard-detected">
              <CheckCircle2 size={16} aria-hidden="true" />{" "}
              {t("wizard.s1.found", {
                list: suggestions
                  .map(
                    (s) =>
                      `${STACKS.find((x) => x.id === s.stack)?.title}${s.nodeFramework ? ` · ${NODE_FRAMEWORK_TITLES[s.nodeFramework]}` : ""} (${s.evidence})`,
                  )
                  .join(", "),
              })}
            </p>
          )}
        </Panel>
      )}

      {step === 1 && (
        <Panel
          title={newProject ? t("wizard.s2.titleNew") : t("wizard.s2.titleExisting")}
          eyebrow={stepLabel(2)}
        >
          <div className="choice-grid" role="radiogroup" aria-label={t("wizard.s2.techAria")}>
            {STACKS.filter((s) => !(newProject && EXISTING_ONLY.includes(s.id))).map((s) => (
              <button
                key={s.id}
                type="button"
                role="radio"
                aria-checked={stack === s.id}
                className={`choice with-logo ${stack === s.id ? "selected" : ""}`}
                onClick={() => chooseStack(s.id)}
              >
                <TechLogo id={s.id} />
                <strong>{s.title}</strong>
                <span>{t(s.hint)}</span>
                {detected(s.id) && <Badge tone="green">{t("wizard.s2.detected")}</Badge>}
              </button>
            ))}
          </div>
          {newProject && meta && (
            <>
              <div className="question-grid">
                <label className="question">
                  <span>{t("wizard.s2.name.label")}</span>
                  <input
                    value={projectName}
                    onChange={(e) => setProjectName(e.target.value.toLowerCase())}
                    aria-invalid={!validName(projectName)}
                  />
                  <small>
                    {validName(projectName)
                      ? t("wizard.s2.name.ok")
                      : t("wizard.s2.name.bad")}
                  </small>
                </label>
              </div>
              <p className="field-label">{t("wizard.s2.how")}</p>
              <div className="choice-grid" role="radiogroup" aria-label={t("wizard.s2.howAria")}>
                {STARTERS[stack!].map((option) => (
                  <button
                    key={option.id}
                    type="button"
                    role="radio"
                    aria-checked={starter === option.id}
                    className={`choice with-logo ${starter === option.id ? "selected" : ""}`}
                    onClick={() => {
                      setStarter(option.id);
                      setVersion(option.id === "nestjs" ? "24" : meta.versions[0]);
                      setPort(option.id.startsWith("vite") ? "5173" : String(meta.port));
                      setPlan(null);
                      setScaffoldPlan(null);
                    }}
                  >
                    <TechLogo id={option.id === "clean" ? stack! : option.id} />
                    <strong>{option.translateTitle ? t(option.title as TranslationKey) : option.title}</strong>
                    <span>{t(option.hint)}</span>
                    {option.id !== "clean" && <Badge tone="blue">{t("wizard.s2.official")}</Badge>}
                  </button>
                ))}
              </div>
            </>
          )}
          {meta && (
            <div className="question-grid">
              <label className="question">
                <span>{t("wizard.s2.port.label")}</span>
                <input
                  inputMode="numeric"
                  value={port}
                  onChange={(e) => {
                    setPort(e.target.value.replace(/\D/g, ""));
                    setPlan(null);
                  }}
                />
                <small>{t("wizard.s2.port.hint")}</small>
              </label>
              <label className="question">
                <span>{t(meta.versionLabel)}</span>
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
                    <span>{t("wizard.s2.pm")}</span>
                    <select value={packageManager} onChange={(e) => setPackageManager(e.target.value as PackageManager)}>
                      <option>npm</option>
                      <option>pnpm</option>
                      <option>yarn</option>
                    </select>
                  </label>
                  {!newProject && (
                  <label className="question">
                    <span>{t("wizard.s2.script.label")}</span>
                    {nodeScripts.length ? (
                      <select value={startScript} onChange={(e) => setStartScript(e.target.value)}>
                        {nodeScripts.map((s) => (
                          <option key={s}>{s}</option>
                        ))}
                      </select>
                    ) : (
                      <input value={startScript} onChange={(e) => setStartScript(e.target.value)} />
                    )}
                    <small>{t("wizard.s2.script.hint", { pm: packageManager, script: startScript || "start" })}</small>
                  </label>
                  )}
                </>
              )}
              {stack === "python" && starter === "clean" && (
                <>
                  <label className="question">
                    <span>{t("wizard.s2.framework")}</span>
                    <select value={framework} onChange={(e) => setFramework(e.target.value as PythonFramework)}>
                      <option value="fastapi">FastAPI</option>
                      {!newProject && <option value="django">Django</option>}
                      <option value="flask">Flask</option>
                    </select>
                  </label>
                  {framework !== "django" && !newProject && (
                    <label className="question">
                      <span>{t("wizard.s2.module.label")}</span>
                      <input
                        value={module}
                        onChange={(e) => setModule(e.target.value)}
                        placeholder={framework === "fastapi" ? "main:app" : "app"}
                      />
                      <small>{t("wizard.s2.module.hint")}</small>
                    </label>
                  )}
                </>
              )}
            </div>
          )}
        </Panel>
      )}

      {step === 2 && (
        <Panel title={t("wizard.s3.title")} eyebrow={stepLabel(3)}>
          <p className="section-description">
            {t("wizard.s3.body")}
          </p>
          {grouped.map(([group, items]) => (
            <fieldset className="service-group" key={group}>
              <legend>{t(group)}</legend>
              <div className="choice-grid">
                {items.map((s) => (
                  <label key={s.id} className={`choice ${hasLogo(s.id) ? "with-logo" : ""} ${services.includes(s.id) ? "selected" : ""}`}>
                    <input
                      type="checkbox"
                      checked={services.includes(s.id)}
                      onChange={() => toggleService(s.id)}
                    />
                    <TechLogo id={s.id} />
                    <strong>{s.title}</strong>
                    <span>{t(s.hint)}</span>
                  </label>
                ))}
              </div>
            </fieldset>
          ))}
        </Panel>
      )}

      {step === 3 && isFramework && !plan && scaffoldPlan && (
        <Panel title={t("wizard.install.title", { title: scaffoldPlan.title })} eyebrow={stepLabel(4)}>
          <p className="section-description">
            {t("wizard.install.body")}
          </p>
          <pre className="code-preview" aria-label={t("wizard.install.cmdAria")}>
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
                {t("wizard.install.running", {
                  time: `${Math.floor(elapsed / 60)}:${String(elapsed % 60).padStart(2, "0")}`,
                })}
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
        <Panel title={newProject ? t("wizard.review.titleNew") : t("wizard.review.title")} eyebrow={stepLabel(4)}>
          {scaffold?.success && (
            <div className="feedback success" role="status">
              <CheckCircle2 size={18} aria-hidden="true" />
              <span>
                {t("wizard.install.done", {
                  title: scaffoldPlan?.title ?? t("wizard.install.official"),
                  seconds: Math.max(1, Math.round(scaffold.durationMs / 1000)),
                })}
              </span>
            </div>
          )}
          <p className="section-description">
            {t("wizard.review.before")}{" "}
            <span className="mono">{displayPath(plan.project)}</span>. {t("wizard.review.after")}
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
                  {t("wizard.review.view")}
                </button>
                <small>{file.description}</small>
                <span className="file-badges">
                  {file.sensitive && <Badge tone="red">{t("wizard.badge.secrets")}</Badge>}
                  {file.exists ? <Badge tone="amber">{t("wizard.badge.exists")}</Badge> : <Badge tone="blue">{t("wizard.badge.new")}</Badge>}
                </span>
              </li>
            ))}
          </ul>
          {!!plan.ports?.length && (
            <div className="port-box">
              <p className="field-label">{t("wizard.ports.title")}</p>
              <ul className="port-list">
                {plan.ports.map((item) => (
                  <li key={item.name} className={item.hostPort !== item.preferredPort ? "changed" : ""}>
                    <span>{item.name === "app" ? t("wizard.ports.app") : item.name}</span>
                    <code>
                      localhost:{item.hostPort} → {item.containerPort}
                    </code>
                    {item.hostPort !== item.preferredPort && (
                      <small>{t("wizard.ports.changed", { port: item.preferredPort })}</small>
                    )}
                  </li>
                ))}
              </ul>
            </div>
          )}
          {activeFile && (
            <pre className="code-preview" aria-label={t("wizard.contentAria", { name: activeFile.name })}>
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
                    ? t("wizard.result.created", { count: result.written.length })
                    : t("wizard.result.none")}
                </strong>
                {result.skipped.map((s) => (
                  <p key={s.name}>
                    {s.name}: {s.reason}
                  </p>
                ))}
                {result.written.length > 0 && (
                  <>
                    <p className="open-title">{t("wizard.open.title")}</p>
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
                        {t("wizard.open.folder")}
                        {opened === "folder" && <Check size={14} aria-hidden="true" />}
                      </button>
                    </div>
                    {!ides.length && (
                      <p className="field-hint">{t("wizard.open.noEditors")}</p>
                    )}
                    <p>{t("wizard.open.run")}</p>
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
                        <Copy size={13} aria-hidden="true" /> {copied ? t("logs.copied") : t("console.copy")}
                      </button>
                    </p>
                    {plan?.ports?.find((item) => item.name === "app") && (
                      <p>
                        {t("wizard.open.visit")}{" "}
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
          <ArrowLeft size={16} aria-hidden="true" /> {t("wizard.back")}
        </button>
        {step < 2 && (
          <button
            type="button"
            className="button primary"
            disabled={!canContinue || !!busy}
            onClick={() => setStep((s) => s + 1)}
          >
            {t("wizard.continue")} <ArrowRight size={16} aria-hidden="true" />
          </button>
        )}
        {step === 2 && (
          <button type="button" className="button primary" disabled={!!busy} onClick={() => void review()}>
            {busy === "preview" ? <LoaderCircle className="spin" size={16} aria-hidden="true" /> : null}
            {t("wizard.review")} <ArrowRight size={16} aria-hidden="true" />
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
            {scaffold && !scaffold.success ? t("wizard.install.retry") : t("wizard.install.go")}
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
            {t("wizard.create", { count: selected.length })}
          </button>
        )}
      </div>
    </div>
  );
}
