import { test, expect } from "@playwright/test";

// Test-only IPC fixture. Production never loads demo projects or fabricated metrics.
const scan = {
  schemaVersion: 1,
  project: "C:\\work\\sample-project",
  components: [
    { path: "web", technology: "nodejs", evidence: ["web/package.json"] },
    { path: "api", technology: "maven", evidence: ["api/pom.xml"] },
    {
      path: "api",
      technology: "spring-boot",
      evidence: [
        "api/pom.xml: org.springframework.boot:spring-boot-starter-parent",
      ],
    },
  ],
  warnings: ["Manifesto ignorado no teste de análise parcial."],
  findings: [
    {
      module: "web",
      kind: "runtime",
      name: "node",
      value: "20.0.0",
      source: "web/package.json → engines.node",
      status: "declared",
    },
    {
      module: "web",
      kind: "framework",
      name: "react",
      value: "^19",
      source: "web/package.json → dependencies.react",
      status: "declared",
    },
    {
      module: "api",
      kind: "container_config",
      name: "Dockerfile",
      value: null,
      source: "api/Dockerfile",
      status: "detected",
    },
  ],
};
const diagnostic = {
  schemaVersion: 1,
  os: "windows",
  notes: ["Consulta somente leitura."],
  checks: [
    {
      name: "node",
      status: "available",
      stdout: "v22.0.0",
      stderr: "",
      detail: "",
      executable: "C:\\node.exe",
      exitCode: 0,
      durationMs: 12,
    },
    {
      name: "git",
      status: "missing",
      stdout: "",
      stderr: "",
      detail: "Não encontrado no PATH.",
    },
    {
      name: "docker",
      status: "available",
      stdout: "Docker version 29.9.2",
      stderr: "",
      detail: "",
    },
    {
      name: "compose",
      status: "timeout",
      stdout: "",
      stderr: "",
      detail: "Prazo excedido.",
    },
    {
      name: "docker-engine",
      status: "execution_error",
      stdout: "",
      stderr: "connection refused",
      detail: "",
    },
  ],
};
test.beforeEach(async ({ page }) => {
  await page.addInitScript(
    ({ scan, diagnostic }) => {
      const state = window as unknown as {
        __TAURI_INTERNALS__: object;
        testFailScan: boolean;
        testCancelPicker: boolean;
        testCalls: string[];
      };
      state.testCalls = [];
      state.__TAURI_INTERNALS__ = {
        invoke: async (command: string) => {
          state.testCalls.push(command);
          if (command === "plugin:dialog|open")
            return state.testCancelPicker ? null : "C:\\work\\sample-project";
          await new Promise((resolve) => setTimeout(resolve, 180));
          if (command === "scan_project") {
            if (state.testFailScan) throw "Diretório não encontrado.";
            return scan;
          }
          if (command === "doctor_environment") return diagnostic;
          if (command === "detect_stacks") return [];
          if (command === "inspect_folder") return true;
          if (command === "preview_docker_setup")
            return {
              schemaVersion: 1,
              project: "C:\\work\\sample-project",
              notes: ["Nada foi gravado ainda."],
              files: [
                { name: "index.js", description: "Servidor.", content: "x", exists: false },
                { name: "Dockerfile", description: "Imagem.", content: "FROM node", exists: false },
              ],
            };
          if (command === "apply_docker_setup")
            return { written: ["index.js", "Dockerfile"], skipped: [] };
          if (command === "detect_ides") return [{ id: "vscode", label: "VS Code" }];
          if (command === "open_in_ide") return null;
          throw new Error(`Unexpected IPC ${command}`);
        },
      };
    },
    { scan, diagnostic },
  );
  await page.goto("/");
});

test("starts empty and Console navigation makes no automatic queries", async ({
  page,
}) => {
  await expect(
    page.getByRole("heading", { name: "Seu workspace, em foco." }),
  ).toBeVisible();
  await expect(page.getByText("Nenhuma análise por enquanto")).toBeVisible();
  expect(await page.evaluate(() => (window as any).testCalls)).toEqual([]);
  await page.screenshot({
    path: "../../.cache/etapa-002-3-overview.png",
    fullPage: true,
  });
  await page
    .getByRole("navigation")
    .getByRole("button", { name: "Console", exact: true })
    .click();
  await expect(page.getByText("Nenhuma consulta executada")).toBeVisible();
  await page
    .getByRole("button", { name: "Logs de containers", exact: true })
    .click();
  await expect(
    page.getByText("Escolha um container para começar"),
  ).toBeVisible();
  expect(await page.evaluate(() => (window as any).testCalls)).toEqual([]);
});
test("selects, scans, groups modules, shows warnings and remembers only one project on rescan", async ({
  page,
}) => {
  await page
    .getByRole("button", { name: "Importar projeto", exact: true })
    .click();
  await expect(page.getByLabel("Caminho do projeto")).toHaveValue(
    "C:\\work\\sample-project",
  );
  await page
    .getByRole("button", { name: "Analisar projeto", exact: true })
    .click();
  await expect(page.getByText("2 módulos identificados")).toBeVisible();
  await expect(
    page.getByRole("heading", { name: "Análise com avisos" }),
  ).toBeVisible();
  await expect(page.getByText("Spring Boot · Detectado")).toBeVisible();
  await expect(page.getByText("web/package.json → engines.node")).toBeVisible();
  await page.screenshot({
    path: "../../.cache/etapa-002-3-projects.png",
    fullPage: true,
  });
  await page
    .getByRole("button", { name: "Analisar projeto", exact: true })
    .click();
  await expect(page.getByText("2 módulos identificados")).toBeVisible();
  await page
    .getByRole("navigation")
    .getByRole("button", { name: "Visão geral" })
    .click();
  await expect(page.locator(".session-list>button")).toHaveCount(1);
});
test("keeps errors explicit, allows retry and diagnoses without claiming unsupported compatibility", async ({
  page,
}) => {
  await page
    .getByRole("navigation")
    .getByRole("button", { name: "Projetos 0" })
    .click();
  await page.getByLabel("Caminho do projeto").fill("C:\\bad");
  await page.evaluate(() => {
    (window as any).testFailScan = true;
  });
  await page
    .getByRole("button", { name: "Analisar projeto", exact: true })
    .click();
  await expect(page.getByRole("alert")).toContainText(
    "Diretório não encontrado.",
  );
  await page.evaluate(() => {
    (window as any).testFailScan = false;
  });
  await page
    .getByRole("button", { name: "Analisar projeto", exact: true })
    .click();
  await expect(page.getByText("2 módulos identificados")).toBeVisible();
  await page
    .getByRole("navigation")
    .getByRole("button", { name: "Ambiente", exact: true })
    .click();
  await expect(
    page.getByText("Não verificado", { exact: true }).first(),
  ).toBeVisible();
  await page
    .getByRole("button", { name: "Verificar ambiente", exact: true })
    .click();
  await expect(page.getByRole("status")).toContainText(
    "Verificando ferramentas",
  );
  await expect(
    page.getByText("Incompatibilidade confirmada", { exact: true }),
  ).toBeVisible();
  await expect(
    page.getByText("Tempo esgotado", { exact: true }).first(),
  ).toBeVisible();
  await expect(
    page.getByText("Erro na verificação", { exact: true }).first(),
  ).toBeVisible();
  await expect(page.getByText(/a causa não foi confirmada/)).toBeVisible();
});
test("canceling picker does not scan and session is not persisted after reload", async ({
  page,
}) => {
  await page.evaluate(() => {
    (window as any).testCancelPicker = true;
  });
  await page
    .getByRole("button", { name: "Importar projeto", exact: true })
    .click();
  await expect(page.getByLabel("Caminho do projeto")).toHaveValue("");
  await expect(
    page.getByRole("button", { name: "Analisar projeto", exact: true }),
  ).toBeDisabled();
  expect(await page.evaluate(() => (window as any).testCalls)).toEqual([
    "plugin:dialog|open",
  ]);
  await page.reload();
  await expect(page.getByText("Nenhuma análise por enquanto")).toBeVisible();
});
test("supports narrow layouts, keyboard navigation and reduced motion", async ({
  page,
}) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.emulateMedia({ reducedMotion: "reduce" });
  await expect(
    page.getByRole("heading", { name: "Seu workspace, em foco." }),
  ).toBeVisible();
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= window.innerWidth,
    ),
  ).toBeTruthy();
  await page.screenshot({
    path: "../../.cache/etapa-002-3-narrow.png",
    fullPage: true,
  });
  await page.keyboard.press("Tab");
  await expect(
    page.getByRole("link", { name: "Pular para o conteúdo" }),
  ).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(page.locator("#content")).toBeFocused();
  await page
    .getByRole("navigation")
    .getByRole("button", { name: "Configurações" })
    .click();
  await expect(
    page.getByRole("heading", { name: "Permissões e operações" }),
  ).toBeVisible();
});

test("contribute tab shows the PIX QR and copy action", async ({
  page,
}) => {
  await page
    .getByRole("navigation")
    .getByRole("button", { name: "Contribuir", exact: true })
    .click();
  await expect(page.getByRole("heading", { name: "Contribuir", level: 1 })).toBeVisible();
  await expect(page.getByText("Contribuição ainda não configurada")).toHaveCount(0);
  await expect(page.getByRole("img", { name: /QR Code PIX/ })).toBeVisible();
  await page.getByRole("button", { name: "R$ 25", exact: true }).click();
  await expect(page.getByRole("img", { name: /valor R\$ 25\.00/ })).toBeVisible();
  await expect(page.getByRole("button", { name: /Copiar PIX copia e cola/ })).toBeVisible();
});

test("docker assistant is reachable and starts at the project step", async ({
  page,
}) => {
  await page
    .getByRole("navigation")
    .getByRole("button", { name: "Assistente Docker", exact: true })
    .click();
  await expect(
    page.getByRole("heading", { name: "Qual projeto você quer colocar no Docker?" }),
  ).toBeVisible();
  await expect(page.getByRole("button", { name: /Continuar/ })).toBeDisabled();
});

test("empty folder offers a new project and then opens it in the detected IDE", async ({
  page,
}) => {
  await page
    .getByRole("navigation")
    .getByRole("button", { name: "Assistente Docker", exact: true })
    .click();
  await page.getByRole("button", { name: "Selecionar pasta" }).last().click();
  await expect(page.getByText("Esta pasta está vazia.")).toBeVisible();
  await page.getByRole("button", { name: "Criar projeto novo" }).click();
  await expect(
    page.getByRole("heading", { name: "Com que tecnologia você quer criar o projeto?" }),
  ).toBeVisible();
  await expect(page.getByRole("radio", { name: /PHP \/ Laravel/ })).toBeDisabled();
  await page.getByRole("button", { name: /Continuar/ }).click();
  await page.getByRole("button", { name: /Gerar preview/ }).click();
  await expect(page.getByText("Revise o projeto novo antes de criar")).toBeVisible();
  await page.getByRole("button", { name: /Criar 2 arquivo/ }).click();
  await page.getByRole("button", { name: "VS Code" }).click();
  expect(await page.evaluate(() => (window as any).testCalls)).toContain("open_in_ide");
});
