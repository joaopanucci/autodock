// Gera as imagens do README com a interface real e dados de exemplo (sem Docker).
// Uso: cargo build -p autodock-cli && npm --prefix apps/desktop run dev &
//      node scripts/screenshots.mjs
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
const out = path.join(root, "docs/screenshots", theme === "dark" ? "dark" : "", lang === "en" ? "en" : "");
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

const browser = await chromium.launch({ channel: process.env.AUTODOCK_BROWSER_CHANNEL ?? (process.platform === "win32" ? "msedge" : "chrome") });
const context = await browser.newContext({ viewport: { width: 1360, height: 860 }, deviceScaleFactor: 2, colorScheme: theme, locale: lang === "en" ? "en-US" : "pt-BR" });
const page = await context.newPage();
await page.addInitScript(({ theme, lang }) => {
  localStorage.setItem("autodock.theme", theme);
  localStorage.setItem("autodock.language", lang);
}, { theme, lang });
await page.addInitScript(({ scan, doctor, containers, files, project, ports, previewNotes, expo }) => {
  window.__TAURI_INTERNALS__ = {
    transformCallback: () => 1,
    invoke: async (command) => {
      switch (command) {
        case "plugin:dialog|open": return project;
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
}, { scan, doctor, containers, files, project, ports, previewNotes, expo });

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

await browser.close();
for (const dir of [sample, firstSample, expoDir]) fs.rmSync(dir, { recursive: true, force: true });
