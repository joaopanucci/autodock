import { useEffect, useRef, useState } from "react";
import { call } from "./ipc";
import { open } from "@tauri-apps/plugin-dialog";
import {
  CheckCircle2,
  FolderPlus,
  Info,
  LoaderCircle,
  ShieldCheck,
  X,
  AlertTriangle,
} from "lucide-react";
import type { Doctor, Page, Scan, SessionProject } from "./types";
import { displayPath, rememberProject } from "./model";
import { Layout, navigation } from "./components/Layout";
import { ThemeSwitch } from "./components/ThemeSwitch";
import { Badge, Panel } from "./components/ui";
import { Overview } from "./pages/Overview";
import { Projects } from "./pages/Projects";
import { Environment } from "./pages/Environment";
import { Containers } from "./pages/Containers";
import { Monitoring } from "./pages/Monitoring";
import { Console } from "./pages/Console";
import { Assistant } from "./pages/Assistant";
import { Contribute } from "./pages/Contribute";
import { Learn } from "./pages/Learn";
import { LANGUAGES, useI18n, type Lang, type TranslationKey } from "./i18n";

const headings: Record<
  Page,
  { title: TranslationKey; description: TranslationKey }
> = {
  overview: { title: "page.overview.title", description: "page.overview.description" },
  projects: { title: "page.projects.title", description: "page.projects.description" },
  setup: { title: "page.setup.title", description: "page.setup.description" },
  environment: { title: "page.environment.title", description: "page.environment.description" },
  containers: { title: "page.containers.title", description: "page.containers.description" },
  monitoring: { title: "page.monitoring.title", description: "page.monitoring.description" },
  console: { title: "page.console.title", description: "page.console.description" },
  learn: { title: "page.learn.title", description: "page.learn.description" },
  contribute: { title: "page.contribute.title", description: "page.contribute.description" },
  settings: { title: "page.settings.title", description: "page.settings.description" },
};

export default function App() {
  const { t, lang, setLang, locale } = useI18n();
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
    // Messages from one page must not show up on another.
    setMessage("");
    setError("");
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
        title: t("app.dialog.selectProject"),
      });
      if (selected) {
        setPath(selected);
        setScan(null);
        setMessage(t("app.message.folderSelected"));
      }
    });
  }
  function analyze() {
    if (!path.trim()) return;
    void run("scan", async () => {
      setScan(null);
      const result = await call<Scan>("scan_project", { path: path.trim() });
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
          ? t("app.message.scanWarnings")
          : t("app.message.scanDone"),
      );
    });
  }
  function diagnose() {
    setPage("environment");
    void run("doctor", async () => {
      const report = await call<Doctor>("doctor_environment");
      setDoctor(report);
      setCheckedAt(new Date().toISOString());
      setMessage(t("app.message.doctorDone"));
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
            {t("app.eyebrow")} <span>/</span>{" "}
            {t(nav.labelKey).toLocaleUpperCase(locale)}
          </p>
          <h1>{t(headings[page].title)}</h1>
          <p>{t(headings[page].description)}</p>
        </div>
        {page === "overview" && (
          <button
            className="button primary"
            onClick={selectProject}
            disabled={!!busy}
          >
            <FolderPlus size={17} aria-hidden="true" />
            {t("app.importProject")}
          </button>
        )}
      </div>
      <div className="feedback-area" aria-live="polite" aria-atomic="true">
        {busy && (
          <div className="feedback loading" role="status">
            <LoaderCircle className="spin" size={18} aria-hidden="true" />
            <span>
              {busy === "scan"
                ? t("app.busy.scan")
                : busy === "doctor"
                  ? t("app.busy.doctor")
                  : t("app.busy.select")}
            </span>
          </div>
        )}
        {!busy && message && (
          <div className="feedback success" role="status">
            <CheckCircle2 size={18} aria-hidden="true" />
            <span>{message}</span>
            <button
              className="icon-button"
              aria-label={t("app.closeMessage")}
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
            <strong>{t("app.error.title")}</strong>
            <p>{error}</p>
            <small>
              {t("app.error.hint")}
            </small>
          </div>
          <button
            className="icon-button"
            aria-label={t("app.closeError")}
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
            prepareDocker={() => setPage("setup")}
          />
        )}
        <div hidden={page !== "setup"}>
          <Assistant path={path} setPath={setPath} />
        </div>
        {page === "learn" && <Learn navigate={setPage} />}
        {page === "contribute" && <Contribute />}
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
        {page === "settings" && (
          <>
            <Panel title={t("settings.language.title")} eyebrow={t("settings.language.eyebrow")}>
              <p className="section-description">{t("settings.language.body")}</p>
              <label className="field-label" htmlFor="language-select">
                {t("settings.language.label")}
              </label>
              <select
                id="language-select"
                className="language-select"
                value={lang}
                onChange={(event) => setLang(event.target.value as Lang)}
              >
                {LANGUAGES.map((item) => (
                  <option key={item.id} value={item.id}>
                    {item.label}
                  </option>
                ))}
              </select>
            </Panel>
            <div className="requirements-grid">
              <Panel title={t("settings.appearance.title")} eyebrow={t("settings.appearance.eyebrow")}>
                <div className="setting-row">
                  <div>
                    <strong>{t("settings.theme.title")}</strong>
                    <p>{t("settings.theme.body")}</p>
                  </div>
                  <ThemeSwitch />
                </div>
                <div className="setting-row">
                  <div>
                    <strong>{t("settings.motion.title")}</strong>
                    <p>{t("settings.motion.body")}</p>
                  </div>
                  <Badge>{t("settings.motion.badge")}</Badge>
                </div>
                <p className="card-note">{t("settings.appearance.note")}</p>
              </Panel>
              <Panel title={t("settings.session.title")} eyebrow={t("settings.session.eyebrow")}>
                <ShieldCheck className="accent-icon" size={26} aria-hidden="true" />
                <p className="section-description">{t("settings.session.body")}</p>
                <Badge>{t("settings.session.badge")}</Badge>
              </Panel>
            </div>
            <Panel title={t("settings.permissions.title")} eyebrow={t("settings.permissions.eyebrow")}>
              <div className="notice">
                <Info size={20} aria-hidden="true" />
                <p>{t("settings.permissions.notice")}</p>
              </div>
              <ol className="permission-steps">
                <li>
                  <span>01</span>
                  <div>
                    <strong>{t("settings.step1.title")}</strong>
                    <p>{t("settings.step1.body")}</p>
                  </div>
                </li>
                <li>
                  <span>02</span>
                  <div>
                    <strong>{t("settings.step2.title")}</strong>
                    <p>{t("settings.step2.body")}</p>
                  </div>
                </li>
                <li>
                  <span>03</span>
                  <div>
                    <strong>{t("settings.step3.title")}</strong>
                    <p>{t("settings.step3.body")}</p>
                  </div>
                </li>
              </ol>
              <p className="card-note">{t("settings.permissions.note")}</p>
            </Panel>
          </>
        )}
      </div>
    </Layout>
  );
}
