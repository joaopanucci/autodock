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
const out = path.join(root, "docs/screenshots");
const base = process.env.AUTODOCK_URL ?? "http://127.0.0.1:1420";
const cli = path.join(root, "target/debug/autodock");

// Arquivos Docker reais, gerados pelo próprio AutoDock para um projeto de exemplo.
const sample = fs.mkdtempSync(path.join(os.tmpdir(), "loja-online-"));
fs.writeFileSync(path.join(sample, "package.json"), JSON.stringify({ name: "loja-online", scripts: { dev: "node --watch index.js", start: "node index.js" } }));
execFileSync(cli, ["generate", sample, "--stack", "nodejs", "--port", "3000", "--start-script", "start", "--service", "postgres", "--service", "redis", "--service", "adminer", "--write"], { stdio: "ignore" });
const files = ["Dockerfile", "docker-compose.yml", ".dockerignore", ".env.docker.example"]
  .filter((name) => fs.existsSync(path.join(sample, name)))
  .map((name) => ({
    name,
    description: {
      Dockerfile: "Como a imagem da aplicação é construída.",
      "docker-compose.yml": "Orquestra a aplicação e os serviços escolhidos.",
      ".dockerignore": "Evita copiar arquivos desnecessários ou sensíveis para a imagem.",
      ".env.docker.example": "Exemplo de variáveis para senhas de desenvolvimento.",
    }[name],
    content: fs.readFileSync(path.join(sample, name), "utf8"),
    exists: false,
  }));

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
const context = await browser.newContext({ viewport: { width: 1360, height: 860 }, deviceScaleFactor: 2, colorScheme: "dark", locale: "pt-BR" });
const page = await context.newPage();
await page.addInitScript(({ scan, doctor, containers, files, project }) => {
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
        case "preview_docker_setup": return { schemaVersion: 1, project, files, notes: ["Nada foi gravado ainda. Revise os arquivos antes de confirmar.", "Os serviços auxiliares publicam portas apenas em 127.0.0.1."] };
        case "apply_docker_setup": return { written: files.map((f) => f.name), skipped: [] };
        case "detect_ides": return [{ id: "vscode", label: "VS Code" }, { id: "cursor", label: "Cursor" }, { id: "intellij", label: "IntelliJ IDEA" }];
        default: throw new Error(`IPC ${command}`);
      }
    },
  };
}, { scan, doctor, containers, files, project });

const shot = async (name) => {
  await page.waitForTimeout(250);
  await page.screenshot({ path: path.join(out, `${name}.png`) });
  console.log("ok", name);
};
const nav = (label) => page.getByRole("navigation").getByRole("button", { name: label }).first().click();

await page.goto(base);
await shot("01-visao-geral");

// Projetos: análise
await nav("Projetos");
await page.getByRole("button", { name: "Selecionar pasta" }).click();
await page.getByRole("button", { name: "Analisar projeto" }).click();
await page.getByText("módulos identificados").first().waitFor();
await shot("02-analise");

// Assistente
await nav("Assistente Docker");
await page.getByRole("button", { name: "Selecionar pasta" }).last().click();
await page.getByText(/Encontramos/).waitFor();
await shot("03-assistente-projeto");
await page.getByRole("button", { name: /Continuar/ }).click();
await shot("04-assistente-tecnologia");
await page.getByRole("button", { name: /Continuar/ }).click();
for (const label of ["PostgreSQL", "Redis", "Adminer"]) await page.locator(".choice", { hasText: label }).click();
await shot("05-assistente-servicos");
await page.getByRole("button", { name: /Revisar/ }).click();
await page.getByText("Revise antes de criar").waitFor();
await page.locator(".file-list li", { hasText: "docker-compose.yml" }).getByRole("button", { name: "ver" }).click();
await shot("06-assistente-revisao");
await page.getByRole("button", { name: /Criar \d+ arquivo/ }).click();
await page.getByText("Abrir o projeto em:").waitFor();
await page.locator(".wizard").scrollIntoViewIfNeeded();
await shot("07-assistente-pronto");

// Containers, ambiente, contribuir
await nav("Containers");
await page.getByRole("button", { name: /Carregar|Atualizar|Listar/ }).first().click().catch(() => {});
await shot("08-containers");
await nav("Ambiente");
await page.getByRole("button", { name: /Verificar|Atualizar/ }).first().click().catch(() => {});
await shot("09-ambiente");
await nav("Contribuir");
await shot("10-contribuir");

await browser.close();
fs.rmSync(sample, { recursive: true, force: true });
