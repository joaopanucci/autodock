// Gera as imagens do README com a interface real e dados de exemplo (sem Docker).
// Uso: cargo build -p autodock-cli && npm --prefix apps/desktop run dev &
//      node scripts/screenshots.mjs
//
// Opções (variáveis de ambiente):
//   AUTODOCK_LANG=en|pt        idioma da interface (padrão pt)
//   AUTODOCK_THEME=dark|light  tema (padrão light)
//   AUTODOCK_OUT=<pasta>       onde gravar (padrão docs/screenshots[/dark][/en])
//   AUTODOCK_VIEWPORT=1440x810 e AUTODOCK_SCALE=1.3333  tamanho da janela e escala (padrão 1360x860 e 2;
//                              o exemplo dá imagens de 1920x1080, boas para vídeo)
//   AUTODOCK_JAVA_DIR=<pasta>  liga as telas de Java, do editor e do terminal (14 a 19). A pasta tem
//                              in/<projeto> (um projeto Maven legado), out/<projeto>-java25 (a cópia
//                              atualizada) e os JSON inspect.json, plan.json e report.json de
//                              `autodock --lang <idioma> java inspect|plan|upgrade <projeto> --into out --json`
//                              (o upgrade precisa de Docker). Os dados do backend são esses de verdade;
//                              só o que a tela mostraria sobre Docker, containers e a saída do terminal é de exemplo.
import { createRequire } from "node:module";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const require = createRequire(new URL("../apps/desktop/package.json", import.meta.url));
const { chromium } = require("@playwright/test");
const root = fileURLToPath(new URL("..", import.meta.url));
const lang = process.env.AUTODOCK_LANG === "en" ? "en" : "pt";
// AUTODOCK_THEME=dark writes to docs/screenshots/dark; the default is the light theme.
const theme = process.env.AUTODOCK_THEME === "dark" ? "dark" : "light";
const out = process.env.AUTODOCK_OUT ?? path.join(root, "docs/screenshots", theme === "dark" ? "dark" : "", lang === "en" ? "en" : "");
const [viewWidth, viewHeight] = (process.env.AUTODOCK_VIEWPORT ?? "1360x860").split("x").map(Number);
const viewScale = Number(process.env.AUTODOCK_SCALE ?? 2);
fs.mkdirSync(out, { recursive: true });
// UI labels the script clicks, per language.
const L = {
  pt: { projects: "Projetos", assistant: "Assistente Docker", selectFolder: "Selecionar pasta", analyze: "Analisar projeto", modules: "módulos identificados", found: /Encontramos/, next: /Continuar/, review: /Revisar/, reviewTitle: "Revise antes de criar", view: "ver", create: /Criar \d+ arquivo/, openIn: "Abrir o projeto em:", containers: "Containers", environment: "Ambiente", contribute: "Contribuir", expo: "Expo", seeUpgrade: "Ver upgrade", understand: /Entendi que o AutoDock vai rodar/, upgradeTo: "Atualizar para o SDK 58", upgraded: /Atualizado do SDK 57/, refresh: /Carregar|Atualizar|Listar/, check: /Verificar|Atualizar/ },
  en: { projects: "Projects", assistant: "Docker assistant", selectFolder: "Select folder", analyze: "Analyze project", modules: "modules found", found: /Found:/, next: /Continue/, review: /Review/, reviewTitle: "Review before creating", view: "view", create: /Create \d+ file/, openIn: "Open the project in:", containers: "Containers", environment: "Environment", contribute: "Contribute", expo: "Expo", seeUpgrade: "See upgrade", understand: /I understand AutoDock will run these commands in Docker/, upgradeTo: "Upgrade to SDK 58", upgraded: /Upgraded from SDK 57/, refresh: /Load|Refresh|List/, check: /Check|Refresh/ },
}[lang];
const base = process.env.AUTODOCK_URL ?? "http://127.0.0.1:1420";
const cli = path.join(root, "target/debug", process.platform === "win32" ? "autodock.exe" : "autodock");

// Docker files generated for real by AutoDock. Two sample projects share one port
// registry, so the second one shows the automatic port adjustment.
process.env.AUTODOCK_HOME = fs.mkdtempSync(path.join(os.tmpdir(), "autodock-home-"));
const services = ["postgres", "redis", "adminer"].flatMap((name) => ["--service", name]);
const makeSample = (name) => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), `${name}-`));
  fs.writeFileSync(path.join(dir, "package.json"), JSON.stringify({ name, scripts: { dev: "node --watch index.js", start: "node index.js" } }));
  execFileSync(cli, ["--lang", lang, "generate", dir, "--stack", "nodejs", "--port", "3000", "--start-script", "start", ...services, "--write"], { stdio: "ignore", env: process.env });
  return dir;
};
const firstSample = makeSample("primeiro-projeto");
const sample = makeSample("loja-online");
const names = ["Dockerfile", "docker-compose.yml", ".dockerignore", ".env.docker", ".env.docker.example"];
const descriptions = lang === "en"
  ? {
      Dockerfile: "How the application image is built.",
      "docker-compose.yml": "Runs the application and the chosen services. Contains no passwords.",
      ".dockerignore": "Keeps unneeded or sensitive files out of the image.",
      ".env.docker": "Strong 32-character passwords generated now. Do not commit this file.",
      ".env.docker.example": "Variable template without secrets, for whoever clones the project.",
    }
  : {
      Dockerfile: "Como a imagem da aplicação é construída.",
      "docker-compose.yml": "Orquestra a aplicação e os serviços escolhidos. Não contém senhas.",
      ".dockerignore": "Evita copiar arquivos desnecessários ou sensíveis para a imagem.",
      ".env.docker": "Senhas fortes de 32 caracteres geradas agora. Não versione este arquivo.",
      ".env.docker.example": "Modelo das variáveis, sem segredos, para quem clonar o projeto.",
    };
const previewNotes = lang === "en"
  ? ["Nothing has been written yet. Review the files before confirming.", "Strong 32-character passwords were just generated and live in .env.docker (restricted permissions). Do not commit that file."]
  : ["Nada foi gravado ainda. Revise os arquivos antes de confirmar.", "Senhas fortes de 32 caracteres foram geradas agora e ficam em .env.docker (permissão restrita). Não versione esse arquivo."];
const files = names
  .filter((name) => fs.existsSync(path.join(sample, name)))
  .map((name) => ({
    name,
    description: descriptions[name],
    content: fs.readFileSync(path.join(sample, name), "utf8"),
    sensitive: name === ".env.docker",
    exists: false,
  }));
// Real host ports taken from the generated compose file.
const compose = fs.readFileSync(path.join(sample, "docker-compose.yml"), "utf8");
const preferred = { app: 3000, postgres: 5432, redis: 6379, adminer: 8088 };
const byContainer = { 3000: "app", 5432: "postgres", 6379: "redis", 8080: "adminer" };
const ports = [...compose.matchAll(/- "(?:127\.0\.0\.1:)?(\d+):(\d+)"/g)].map(([, host, container]) => {
  const name = byContainer[container];
  return { name, containerPort: Number(container), hostPort: Number(host), preferredPort: preferred[name] };
});

const project = "/Users/maria/projetos/loja-online";
const scan = {
  schemaVersion: 1,
  project,
  components: [
    { path: "web", technology: "nodejs", evidence: ["web/package.json"] },
    { path: "api", technology: "maven", evidence: ["api/pom.xml"] },
    { path: "api", technology: "spring-boot", evidence: ["api/pom.xml: org.springframework.boot:spring-boot-starter-parent"] },
  ],
  warnings: [],
  findings: [
    { module: "web", kind: "runtime", name: "node", value: "22.0.0", source: "web/package.json → engines.node", status: "declared" },
    { module: "web", kind: "framework", name: "react", value: "^19", source: "web/package.json → dependencies.react", status: "declared" },
    { module: "api", kind: "runtime", name: "java", value: "21", source: "api/pom.xml → java.version", status: "declared" },
    { module: "api", kind: "container_config", name: "Dockerfile", value: null, source: "api/Dockerfile", status: "detected" },
  ],
};
const check = (name, stdout) => ({ name, status: "available", stdout, stderr: "", detail: "", durationMs: 40, exitCode: 0 });
const doctor = {
  schemaVersion: 1,
  os: "macos",
  notes: ["Consulta somente leitura."],
  checks: [check("node", "v22.11.0"), check("git", "git version 2.47.0"), check("docker", "Docker version 27.3.1"), check("compose", "Docker Compose version v2.29.7"), check("docker-engine", "27.3.1")],
};
const containers = {
  schemaVersion: 1,
  containers: [
    { id: "a1b2c3d4e5f6", name: "loja-online", image: "loja-online-app", state: "running", status: "Up 2 hours", ports: "0.0.0.0:3000->3000/tcp", project: "loja-online", service: "app", health: null },
    { id: "b2c3d4e5f6a1", name: "loja-online-postgres", image: "postgres:16-alpine", state: "running", status: "Up 2 hours (healthy)", ports: "127.0.0.1:5432->5432/tcp", project: "loja-online", service: "postgres", health: "healthy" },
    { id: "c3d4e5f6a1b2", name: "loja-online-redis", image: "redis:7-alpine", state: "running", status: "Up 2 hours (healthy)", ports: "127.0.0.1:6379->6379/tcp", project: "loja-online", service: "redis", health: "healthy" },
  ],
};

// Expo: a tiny real SDK 57 project, so the inspection and the upgrade plan come from the CLI.
const expoDir = fs.mkdtempSync(path.join(os.tmpdir(), "meu-app-"));
fs.writeFileSync(path.join(expoDir, "package.json"), JSON.stringify({ name: "meu-app", main: "expo-router/entry", dependencies: { expo: "~57.0.32", "expo-router": "~57.0.30", react: "19.2.3", "react-native": "0.86.3" } }, null, 2));
fs.writeFileSync(path.join(expoDir, "package-lock.json"), "{}");
fs.writeFileSync(path.join(expoDir, ".gitignore"), "/android\n/ios\n");
for (const folder of ["android", "ios"]) {
  fs.mkdirSync(path.join(expoDir, folder));
  fs.writeFileSync(path.join(expoDir, folder, ".keep"), "");
}
const expoProjectPath = "/Users/maria/projetos/meu-app";
const expoReal = fs.realpathSync.native(expoDir);
const prettify = (value) => JSON.parse(JSON.stringify(value).split(JSON.stringify(expoReal).slice(1, -1)).join(expoProjectPath));
const cliJson = (...args) => JSON.parse(execFileSync(cli, ["--lang", lang, ...args], { env: process.env }).toString());
const expoProject = prettify(cliJson("expo", "inspect", expoDir, "--json"));
const expoPlan = prettify(cliJson("expo", "plan", expoDir, "--json"));
const expoLatest = { checkedAt: Math.floor(Date.now() / 1000) - 600, latestVersion: "58.0.23", latestSdk: 58, availableSdks: [56, 57, 58] };
const expoState = (project, update, latest = expoLatest) => ({
  schemaVersion: 1,
  checkOnStart: false,
  latest,
  projects: project ? [{ path: expoProjectPath, name: "meu-app", project, update }] : [],
  alerts: update && !update.dismissed ? 1 : 0,
  checkError: null,
});
const expo = {
  empty: expoState(null, null, null),
  before: expoState(expoProject, { currentSdk: 57, latestSdk: 58, nextSdk: 58, stepsBehind: 1, dismissed: false }),
  after: expoState({ ...expoProject, sdk: 58, declaredVersion: "^58.0.23" }, null),
  plan: expoPlan,
  backups: [{ id: "1794028502-57-58", fromSdk: 57, toSdk: 58, createdAt: Math.floor(Date.now() / 1000), status: "done", files: ["package.json", "package-lock.json"], modifiedSinceUpgrade: false }],
  report: {
    schemaVersion: 1, succeeded: true, restored: false, fromSdk: 57, toSdk: 58,
    changes: [
      ["expo", "~57.0.32", "^58.0.23"], ["expo-router", "~57.0.30", "~58.0.30"], ["react", "19.2.3", "19.3.0"],
      ["react-dom", "19.2.3", "19.3.0"], ["react-native", "0.86.3", "0.88.1"], ["react-native-reanimated", "4.5.1", "4.7.1"],
      ["react-native-screens", "~4.26.0", "~4.30.1"], ["react-native-safe-area-context", "~5.7.0", "~5.11.1"],
    ].map(([name, from, to]) => ({ name, section: "dependencies", from, to })),
    doctor: "Running 20 checks on your project...\n20/20 checks passed. No issues detected!",
    doctorPassed: true, usedLegacyPeerDeps: false, output: "", detail: "", durationMs: 41000, backupId: "1794028502-57-58",
    nextSteps: lang === "en"
      ? ["Run npm install in the project to update the node_modules on your computer.", "Delete the android folder (it is generated) so it is recreated on the next build.", "Delete the ios folder (it is generated) so it is recreated on the next build.", "Read the SDK 58 release notes and test the app."]
      : ["Rode npm install no projeto para atualizar a node_modules do seu computador.", "Apague a pasta android (é gerada) para ela ser recriada no próximo build.", "Apague a pasta ios (é gerada) para ela ser recriada no próximo build.", "Leia as notas da versão do SDK 58 e teste o app."],
  },
};

// Words of the interface in this language, so the script clicks what a person would read.
const dictSource = fs.readFileSync(path.join(root, "apps/desktop/src/i18n", `${lang}.ts`), "utf8");
const dict = Object.fromEntries(
  [...dictSource.matchAll(/^\s*"([^"]+)":\s*"((?:[^"\\]|\\.)*)",?\s*$/gm)].map(([, key, value]) => [key, JSON.parse(`"${value}"`)]),
);
const T = (key, vars = {}) => (dict[key] ?? key).replace(/\{(\w+)\}/g, (_, name) => String(vars[name] ?? `{${name}}`));
const startOf = (text, size = 24) => new RegExp(text.slice(0, size).replace(/[.*+?^${}()|[\]\\]/g, "\\$&"));

// Java: real inspection, plan and report of a legacy project, and the real files of the project and
// of its upgraded copy, shown under a made-up folder so no path of this computer appears.
let tour = null;
if (process.env.AUTODOCK_JAVA_DIR) {
  const javaDir = process.env.AUTODOCK_JAVA_DIR;
  const shown = "/Users/maria/projetos";
  const [name] = fs.readdirSync(path.join(javaDir, "in"));
  const inDir = path.join(javaDir, "in", name);
  const outDir = path.join(javaDir, "out", `${name}-java25`);
  const slashes = (text) => text.replaceAll("\\", "/").replace("//?/", "");
  const escape = (text) => text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const swaps = [
    [outDir, `${shown}/${name}-java25`],
    [inDir, `${shown}/${name}`],
  ].flatMap(([from, to]) => [[slashes(path.resolve(from)), to], [slashes(fs.realpathSync.native(from)), to]]);
  const hide = (text) => swaps.reduce((acc, [from, to]) => acc.replace(new RegExp(escape(from), "gi"), to), slashes(text));
  const deep = (value) =>
    typeof value === "string" ? hide(value) : Array.isArray(value) ? value.map(deep) : value && typeof value === "object" ? Object.fromEntries(Object.entries(value).map(([k, v]) => [k, deep(v)])) : value;
  const readJson = (file) => deep(JSON.parse(fs.readFileSync(path.join(javaDir, file), "utf8").replace(/^\uFEFF/, "")));
  const readTree = (dir, prefix = "") =>
    fs.readdirSync(dir, { withFileTypes: true }).reduce((files, entry) => {
      if (["target", ".git", "node_modules"].includes(entry.name)) return files;
      const relative = prefix ? `${prefix}/${entry.name}` : entry.name;
      if (entry.isDirectory()) return { ...files, ...readTree(path.join(dir, entry.name), relative) };
      const bytes = fs.readFileSync(path.join(dir, entry.name));
      return bytes.length > 200_000 || bytes.includes(0) ? files : { ...files, [relative]: bytes.toString("utf8") };
    }, {});
  tour = {
    name,
    project: `${shown}/${name}`,
    copy: `${shown}/${name}-java25`,
    java: { project: readJson("inspect.json"), plan: readJson("plan.json"), report: readJson("report.json") },
    fs: { [`${shown}/${name}`]: readTree(inDir), [`${shown}/${name}-java25`]: readTree(outDir) },
  };
}

const browser = await chromium.launch({ channel: process.env.AUTODOCK_BROWSER_CHANNEL ?? (process.platform === "win32" ? "msedge" : "chrome") });
const context = await browser.newContext({ viewport: { width: viewWidth, height: viewHeight }, deviceScaleFactor: viewScale, colorScheme: theme, locale: lang === "en" ? "en-US" : "pt-BR" });
const page = await context.newPage();
await page.addInitScript(({ theme, lang }) => {
  localStorage.setItem("autodock.theme", theme);
  localStorage.setItem("autodock.language", lang);
}, { theme, lang });
await page.addInitScript(({ scan, doctor, containers, files, project, ports, previewNotes, expo, tour }) => {
  const callbacks = {};
  let nextCallback = 1;
  let ticks = 0;
  const written = {};
  const fileList = (root) => ({ ...(tour?.fs[root] ?? {}), ...(written[root] ?? {}) });
  window.__TAURI_INTERNALS__ = {
    transformCallback: (callback) => {
      const id = nextCallback++;
      callbacks[id] = callback;
      return id;
    },
    unregisterCallback: (id) => delete callbacks[id],
    invoke: async (command, args) => {
      switch (command) {
        case "plugin:dialog|open": return window.__dialogPath ?? project;
        case "update_get_prefs": return { schemaVersion: 1, checkOnStart: true, lastCheck: Math.floor(Date.now() / 1000) - 3600, skippedVersion: null };
        case "update_auto_check":
        case "update_check_now": return { checked: true, offer: null, error: null };
        case "update_set_check_on_start": return { schemaVersion: 1, checkOnStart: args.enabled, lastCheck: null, skippedVersion: null };
        case "collect_container_metrics": {
          ticks += 1;
          const wave = (seed, base, amplitude) => Math.max(1, Math.round(base + amplitude * Math.sin(ticks / 2 + seed) + (ticks * seed) % 5));
          const row = (id, name, seed, cpu, megabytes, limit) => ({
            id, name, cpuPercent: wave(seed, cpu, cpu / 2), memoryUsageBytes: (megabytes + ticks * seed) * 1e6, memoryLimitBytes: limit * 1e6,
            memoryPercent: ((megabytes + ticks * seed) / limit) * 100, networkRxBytes: ticks * ticks * 30_000 * seed, networkTxBytes: ticks * ticks * 12_000 * seed,
          });
          return {
            schemaVersion: 1, collectedAtUnixMs: Date.now() - 120_000 + ticks * 5000,
            containers: [row("a1b2c3d4e5f6", "loja-online", 1, 22, 380, 1024), row("b2c3d4e5f6a1", "loja-online-postgres", 2, 9, 210, 1024), row("c3d4e5f6a1b2", "loja-online-redis", 3, 3, 18, 256)],
          };
        }
        case "execute_console_operation": {
          const lines = [
            { ID: "a1b2c3d4e5f6", Names: "loja-online", Image: "loja-online-app", State: "running", Status: "Up 2 hours", Ports: "0.0.0.0:3000->3000/tcp" },
            { ID: "b2c3d4e5f6a1", Names: "loja-online-postgres", Image: "postgres:16-alpine", State: "running", Status: "Up 2 hours (healthy)", Ports: "127.0.0.1:5432->5432/tcp" },
            { ID: "c3d4e5f6a1b2", Names: "loja-online-redis", Image: "redis:7-alpine", State: "running", Status: "Up 2 hours (healthy)", Ports: "127.0.0.1:6379->6379/tcp" },
          ];
          return {
            schemaVersion: 1, operation: args.operation, label: args.operation, command: "docker ps --all --format json", status: "available", exitCode: 0,
            stdout: args.operation === "containers" ? lines.map((line) => JSON.stringify(line)).join("\n") : "Docker version 27.3.1, build ce12230", stderr: "", detail: "", truncated: false, durationMs: 84,
          };
        }
        case "java_inspect": return tour.java.project;
        case "java_upgrade_plan": return tour.java.plan;
        case "java_upgrade_run": return tour.java.report;
        case "editor_open_root": return { path: args.path, name: args.path.split("/").pop() };
        case "editor_list": {
          const prefix = args.path ? `${args.path}/` : "";
          const entries = new Map();
          for (const [relative, text] of Object.entries(fileList(args.root))) {
            if (!relative.startsWith(prefix)) continue;
            const [first, ...more] = relative.slice(prefix.length).split("/");
            entries.set(first, more.length ? { name: first, kind: "dir", size: 0 } : { name: first, kind: "file", size: text.length });
          }
          const sorted = [...entries.values()].sort((a, b) => Number(a.kind !== "dir") - Number(b.kind !== "dir") || a.name.localeCompare(b.name));
          return { entries: sorted, truncated: false };
        }
        case "editor_read": {
          const text = fileList(args.root)[args.path];
          if (text === undefined) throw `${args.path}: not found`;
          return { path: args.path, text, size: text.length, modifiedMs: 1000, bom: false, sensitive: false };
        }
        case "editor_write": {
          written[args.root] = { ...(written[args.root] ?? {}), [args.path]: args.text };
          return { modifiedMs: 2000, size: args.text.length };
        }
        case "editor_search": {
          const needle = args.caseSensitive ? args.query : args.query.toLowerCase();
          const matches = [];
          let filesWithMatches = 0;
          const all = Object.entries(fileList(args.root)).sort(([a], [b]) => a.localeCompare(b));
          for (const [relative, text] of all) {
            let found = false;
            text.split("\n").forEach((line, index) => {
              const at = (args.caseSensitive ? line : line.toLowerCase()).indexOf(needle);
              if (at < 0) return;
              found = true;
              const preview = line.trim();
              matches.push({ path: relative, line: index + 1, column: at + 1, preview, previewStart: at - (line.length - line.trimStart().length), length: needle.length });
            });
            if (found) filesWithMatches += 1;
          }
          return { matches, filesScanned: all.length, filesWithMatches, filesSkipped: 0, truncated: false };
        }
        case "terminal_images": return [
          { id: "maven-jdk25", label: "Java (Maven + JDK 25)", image: "maven:3.9-eclipse-temurin-25" },
          { id: "node22", label: "Node.js 22", image: "node:22-bookworm-slim" },
          { id: "python313", label: "Python 3.13", image: "python:3.13-slim" },
          { id: "debian", label: "Debian", image: "debian:bookworm-slim" },
        ];
        case "terminal_default_image": return "maven-jdk25";
        case "terminal_start": {
          let index = 0;
          window.__terminal = (message) => callbacks[args.onEvent.id]({ index: index++, message });
          return 1;
        }
        case "terminal_write":
        case "terminal_resize":
        case "terminal_stop": return null;
        case "scan_project": return scan;
        case "doctor_environment": return doctor;
        case "list_docker_containers": return containers;
        case "detect_stacks": return [{ stack: "nodejs", evidence: "package.json", scripts: ["dev", "start"], packageManager: "npm", pythonFramework: null, vite: false }];
        case "inspect_folder": return false;
        case "preview_docker_setup": return { schemaVersion: 1, project, files, seed: "0".repeat(64), ports, notes: previewNotes };
        case "apply_docker_setup": return { written: files.map((f) => f.name), skipped: [] };
        case "expo_watch_state":
        case "expo_auto_check":
          return sessionStorage.getItem("expo-demo") ? (window.__upgraded ? expo.after : expo.before) : expo.empty;
        case "expo_upgrade_plan": return expo.plan;
        case "expo_upgrade_run": window.__upgraded = true; return expo.report;
        case "expo_backups": return window.__upgraded ? expo.backups : [];
        case "expo_inspect": return null;
        case "detect_ides": return [{ id: "vscode", label: "VS Code" }, { id: "cursor", label: "Cursor" }, { id: "intellij", label: "IntelliJ IDEA" }];
        default: throw new Error(`IPC ${command}`);
      }
    },
  };
}, { scan, doctor, containers, files, project, ports, previewNotes, expo, tour });

const shot = async (name) => {
  await page.waitForTimeout(250);
  await page.screenshot({ path: path.join(out, `${name}.png`) });
  console.log("ok", name);
};
const nav = (label) => page.getByRole("navigation").getByRole("button", { name: label }).first().click();

await page.goto(base);
await shot("01-visao-geral");

// Projetos: análise
await nav(L.projects);
await page.getByRole("button", { name: L.selectFolder }).click();
await page.getByRole("button", { name: L.analyze }).click();
await page.getByText(L.modules).first().waitFor();
await shot("02-analise");

// Visão geral com um projeto analisado e o ambiente verificado
await nav(T("nav.overview"));
await page.getByRole("button", { name: T("env.verify") }).first().click();
await page.getByText("v22.11.0").first().waitFor().catch(() => {});
await nav(T("nav.overview"));
await page.waitForTimeout(400);
await shot("24-visao-geral-projetos");

// Assistente
await nav(L.assistant);
await page.getByRole("button", { name: L.selectFolder }).last().click();
await page.getByText(L.found).waitFor();
await shot("03-assistente-projeto");
await page.getByRole("button", { name: L.next }).click();
await shot("04-assistente-tecnologia");
await page.getByRole("button", { name: L.next }).click();
for (const label of ["PostgreSQL", "Redis", "Adminer"]) await page.locator(".choice", { hasText: label }).click();
await shot("05-assistente-servicos");
await page.getByRole("button", { name: L.review }).click();
await page.getByText(L.reviewTitle).waitFor();
await page.locator(".file-list li", { hasText: "docker-compose.yml" }).getByRole("button", { name: L.view }).click();
await shot("06-assistente-revisao");
await page.getByRole("button", { name: L.create }).click();
await page.getByText(L.openIn).waitFor();
await page.locator(".wizard").scrollIntoViewIfNeeded();
await shot("07-assistente-pronto");

// Containers, ambiente, contribuir
await nav(L.containers);
await page.getByRole("button", { name: L.refresh }).first().click().catch(() => {});
await shot("08-containers");
await nav(L.environment);
await page.getByRole("button", { name: L.check }).first().click().catch(() => {});
await shot("09-ambiente");
await nav(L.contribute);
await shot("10-contribuir");

// Expo: aviso de SDK novo, plano de atualização e relatório
await page.evaluate(() => sessionStorage.setItem("expo-demo", "1"));
await page.reload();
await page.getByTestId("expo-alert").waitFor();
await shot("11-expo-aviso");
await page.getByTestId("expo-alert").getByRole("button", { name: L.seeUpgrade }).click();
await page.getByLabel(L.understand).waitFor();
await shot("12-expo-atualizacao");
await page.getByLabel(L.understand).check();
await page.locator(".multi-run-actions").getByRole("button", { name: L.upgradeTo }).click();
await page.getByText(L.upgraded).waitFor();
await page.evaluate(() => document.querySelectorAll(".expo-report details").forEach((d) => { d.open = true; }));
await page.locator(".expo-report").scrollIntoViewIfNeeded();
await shot("13-expo-relatorio");

// Monitoramento, console, aprender e configurações
await nav(T("nav.monitoring"));
const refreshMetrics = page.getByRole("button", { name: T("containers.refresh") }).first();
for (let i = 0; i < 14; i++) {
  await refreshMetrics.click();
  await page.waitForTimeout(120);
}
await shot("20-monitoramento");
await page.locator("svg.ad-monitor-chart").nth(2).evaluate((chart) => chart.scrollIntoView({ block: "center" }));
await shot("25-monitoramento-graficos");
await nav(T("nav.console"));
await page.getByRole("button", { name: T("console.run") }).click();
await page.locator(".console-table").first().waitFor();
await shot("21-console");
await page.locator(".console-table").first().evaluate((table) => table.scrollIntoView({ block: "center" }));
await shot("26-console-resultado");
await nav(T("nav.learn"));
await shot("22-aprender");
await nav(T("nav.settings"));
await shot("23-configuracoes");

// Java e editor, com os dados de um projeto legado de verdade
if (tour) {
  await page.evaluate((folder) => { window.__dialogPath = folder; }, tour.project);
  await nav(T("nav.java"));
  await page.getByRole("button", { name: T("java.pick.button") }).first().click();
  await page.getByRole("button", { name: T("java.project.plan") }).click();
  const understand = page.getByLabel(startOf(T("java.upgrade.confirm")));
  await understand.waitFor();
  await page.locator(".expo-steps").first().evaluate((list) => list.scrollIntoView({ block: "center" }));
  await shot("14-java-plano");
  await understand.check();
  await page.locator(".multi-run-actions").getByRole("button", { name: T("java.upgrade.start") }).click();
  await page.getByRole("button", { name: T("java.report.review") }).waitFor();
  // o relatório, do começo, e depois a parte de baixo (testes, revisão e próximos passos)
  await page.locator("section.panel").filter({ hasText: T("java.report.destination") }).last().evaluate((panel) => panel.scrollIntoView({ block: "start" }));
  await shot("15-java-relatorio");
  await page.getByRole("button", { name: T("java.report.review") }).evaluate((button) => button.scrollIntoView({ block: "center" }));
  await shot("27-java-relatorio-proximos-passos");

  // revisar o upgrade no editor: lado a lado
  await page.getByRole("button", { name: T("java.report.review") }).click();
  await page.locator(".editor-changes").waitFor();
  await page.locator(".editor-changes").getByRole("button", { name: "pom.xml" }).click();
  await page.getByTestId("diff-editor").waitFor();
  await page.waitForTimeout(1500);
  await shot("16-editor-revisao");

  // um arquivo da cópia, no editor
  await page.getByRole("button", { name: T("editor.tab.close", { name: "pom.xml" }) }).click();
  await page.getByRole("button", { name: T("editor.review.stop") }).click();
  await page.getByRole("tab", { name: T("editor.view.files") }).click();
  const open = async (...parts) => {
    for (const part of parts.slice(0, -1)) await page.getByRole("treeitem", { name: part }).first().click();
    await page.getByRole("treeitem", { name: parts.at(-1) }).first().click();
  };
  await open("src", "main", "java", "com", "example", "shop", "ProductController.java");
  await page.waitForTimeout(1200);
  await shot("17-editor-codigo");

  // buscar no projeto inteiro
  await page.getByRole("tab", { name: T("editor.view.search") }).click();
  await page.getByLabel(T("editor.search.label")).fill("jakarta");
  await page.getByRole("button", { name: T("editor.search.button"), exact: true }).click();
  await page.locator(".editor-hit").first().waitFor();
  await page.locator('.editor-hit[aria-label*="ProductController"]').first().click();
  await page.waitForTimeout(1200);
  await shot("18-editor-busca");

  // o terminal, que roda num container com a pasta montada
  await page.getByRole("button", { name: T("terminal.toggle"), exact: true }).click();
  await page.locator(".terminal-screen .xterm").waitFor();
  await page.waitForFunction(() => typeof window.__terminal === "function");
  const prompt = "\x1b[01;32mroot@9f3c2a1b7d40\x1b[00m:\x1b[01;34m/work\x1b[00m# ";
  const tests = tour.java.report.tests?.run ?? 2;
  const lines = [
    `${prompt}java -version`,
    'openjdk version "25" 2025-09-16 LTS',
    "OpenJDK Runtime Environment Temurin-25+36 (build 25+36-LTS)",
    `${prompt}mvn -q test`,
    "\x1b[1;34m[INFO]\x1b[m  T E S T S",
    `\x1b[1;34m[INFO]\x1b[m Tests run: ${tests}, Failures: 0, Errors: 0, Skipped: 0`,
    "\x1b[1;34m[INFO]\x1b[m \x1b[1;32mBUILD SUCCESS\x1b[m",
    `${prompt}`,
  ];
  await page.evaluate((text) => window.__terminal({ event: "data", data: btoa(text) }), lines.join("\r\n"));
  await page.waitForTimeout(600);
  await shot("19-editor-terminal");
}

await browser.close();
for (const dir of [sample, firstSample, expoDir]) fs.rmSync(dir, { recursive: true, force: true });
