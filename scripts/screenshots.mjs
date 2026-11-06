// Gera as imagens do README com a interface real e dados de exemplo (sem Docker).
// Uso: cargo build -p autodock-cli && npm --prefix apps/desktop run dev &
//      node scripts/screenshots.mjs
import { createRequire } from "node:module";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

const require = createRequire(new URL("../apps/desktop/package.json", import.meta.url));
const { chromium } = require("@playwright/test");
const root = new URL("..", import.meta.url).pathname;
const lang = process.env.AUTODOCK_LANG === "en" ? "en" : "pt";
const out = path.join(root, "docs/screenshots", lang === "en" ? "en" : "");
fs.mkdirSync(out, { recursive: true });
// UI labels the script clicks, per language.
const L = {
  pt: { projects: "Projetos", assistant: "Assistente Docker", selectFolder: "Selecionar pasta", analyze: "Analisar projeto", modules: "módulos identificados", found: /Encontramos/, next: /Continuar/, review: /Revisar/, reviewTitle: "Revise antes de criar", view: "ver", create: /Criar \d+ arquivo/, openIn: "Abrir o projeto em:", containers: "Containers", environment: "Ambiente", contribute: "Contribuir", refresh: /Carregar|Atualizar|Listar/, check: /Verificar|Atualizar/ },
  en: { projects: "Projects", assistant: "Docker assistant", selectFolder: "Select folder", analyze: "Analyze project", modules: "modules found", found: /Found:/, next: /Continue/, review: /Review/, reviewTitle: "Review before creating", view: "view", create: /Create \d+ file/, openIn: "Open the project in:", containers: "Containers", environment: "Environment", contribute: "Contribute", refresh: /Load|Refresh|List/, check: /Check|Refresh/ },
}[lang];
const base = process.env.AUTODOCK_URL ?? "http://127.0.0.1:1420";
const cli = path.join(root, "target/debug/autodock");

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
    { id: "a1b2c3d4e5f6", name: "loja-online-app-1", image: "loja-online-app", state: "running", status: "Up 2 hours", ports: "0.0.0.0:3000->3000/tcp", project: "loja-online", service: "app", health: null },
    { id: "b2c3d4e5f6a1", name: "loja-online-postgres-1", image: "postgres:16-alpine", state: "running", status: "Up 2 hours (healthy)", ports: "127.0.0.1:5432->5432/tcp", project: "loja-online", service: "postgres", health: "healthy" },
    { id: "c3d4e5f6a1b2", name: "loja-online-redis-1", image: "redis:7-alpine", state: "running", status: "Up 2 hours (healthy)", ports: "127.0.0.1:6379->6379/tcp", project: "loja-online", service: "redis", health: "healthy" },
  ],
};

const browser = await chromium.launch({ channel: process.env.AUTODOCK_BROWSER_CHANNEL ?? "chrome" });
const context = await browser.newContext({ viewport: { width: 1360, height: 860 }, deviceScaleFactor: 2, colorScheme: "dark", locale: lang === "en" ? "en-US" : "pt-BR" });
const page = await context.newPage();
await page.addInitScript(({ scan, doctor, containers, files, project, ports, previewNotes }) => {
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
        case "detect_ides": return [{ id: "vscode", label: "VS Code" }, { id: "cursor", label: "Cursor" }, { id: "intellij", label: "IntelliJ IDEA" }];
        default: throw new Error(`IPC ${command}`);
      }
    },
  };
}, { scan, doctor, containers, files, project, ports, previewNotes });

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

await browser.close();
for (const dir of [sample, firstSample]) fs.rmSync(dir, { recursive: true, force: true });
