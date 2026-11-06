import { test, expect } from "@playwright/test";

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    const state = window as any;
    state.calls = [];
    state.logStatus = "available";
    state.logOutput = "test stdout\n";
    state.logError = "test stderr\n";
    state.logTruncated = false;
    state.consoleBadJson = false;
    state.consoleTruncated = false;
    state.emptyList = false;
    Object.defineProperty(navigator, "clipboard", {
      value: {
        writeText: async (text: string) => {
          state.copiedText = text;
        },
      },
    });
    state.__TAURI_INTERNALS__ = {
      invoke: async (command: string, args: any) => {
        state.calls.push({ command, args });
        await new Promise((resolve) => setTimeout(resolve, 40));
        if (command === "list_docker_containers")
          return {
            schemaVersion: 1,
            containers: state.emptyList
              ? []
              : [
                  {
                    id: "a".repeat(64),
                    name: "test-web",
                    image: "test",
                    state: "running",
                    status: "Up",
                    ports: "",
                    project: null,
                    service: null,
                    health: null,
                  },
                  {
                    id: "b".repeat(64),
                    name: "test-worker",
                    image: "test",
                    state: "exited",
                    status: "Exited",
                    ports: "",
                    project: null,
                    service: null,
                    health: null,
                  },
                ],
          };
        if (command === "collect_container_metrics")
          return {
            schemaVersion: 1,
            collectedAtUnixMs: Date.now(),
            containers: [],
          };
        if (command === "query_container_logs")
          return {
            schemaVersion: 1,
            ...args.request,
            status: state.logStatus,
            exitCode: state.logStatus === "available" ? 0 : null,
            stdout: state.logOutput,
            stderr: state.logError,
            truncated: state.logTruncated,
            detail: state.logTruncated ? "Saída truncada pelo limite." : "",
            durationMs: 12,
          };
        if (command === "execute_console_operation")
          return {
            schemaVersion: 1,
            operation: args.operation,
            label: args.operation,
            command: "docker " + args.operation,
            status: state.consoleTruncated ? "execution_error" : "available",
            exitCode: 0,
            stdout: state.consoleBadJson
              ? "{broken"
              : args.operation === "containers"
                ? JSON.stringify({
                    ID: "a".repeat(64),
                    Names: "test-web",
                    Image: "test",
                    State: "running",
                    Status: "Up",
                    Ports: "",
                  })
                : "test version",
            stderr: "",
            detail: state.consoleTruncated ? "Saida truncada" : "",
            truncated: state.consoleTruncated,
            durationMs: 12,
          };
        throw Error("Unexpected test IPC");
      },
    };
  });
  await page.goto("/");
  await page
    .getByRole("navigation")
    .getByRole("button", { name: "Console", exact: true })
    .click();
});
async function logs(page: any) {
  await page
    .getByRole("button", { name: "Logs de containers", exact: true })
    .click();
  await page
    .getByRole("button", { name: "Carregar containers", exact: true })
    .click();
  await expect(page.getByLabel("Container", { exact: true })).toBeVisible();
}
test("preserves three operations, formatted/raw views and history capped at twenty across navigation", async ({
  page,
}) => {
  await expect(page.locator(".console-operation")).toHaveCount(3);
  await page
    .getByRole("button", { name: "Executar consulta", exact: true })
    .click();
  await expect(page.getByRole("table")).toBeVisible();
  await page.getByRole("button", { name: "Bruta", exact: true }).click();
  await expect(page.locator(".console-terminal")).toContainText("test-web");
  await page.getByRole("button", { name: "Formatada", exact: true }).click();
  for (let i = 0; i < 20; i++) {
    await page
      .getByRole("button", { name: "Executar consulta", exact: true })
      .click();
    await expect(
      page.getByRole("button", { name: "Executar consulta", exact: true }),
    ).toBeEnabled();
  }
  await expect(page.locator(".console-history-item")).toHaveCount(20);
  const before = await page.evaluate(() => (window as any).calls.length);
  await page
    .getByRole("navigation")
    .getByRole("button", { name: "Projetos 0" })
    .click();
  await page
    .getByRole("navigation")
    .getByRole("button", { name: "Console", exact: true })
    .click();
  await expect(page.locator(".console-history-item")).toHaveCount(20);
  expect(await page.evaluate(() => (window as any).calls.length)).toBe(before);
});
test("invalid and truncated JSON never appears as a complete formatted table", async ({
  page,
}) => {
  for (const field of ["consoleBadJson", "consoleTruncated"]) {
    await page.evaluate((field) => {
      (window as any)[field] = true;
    }, field);
    await page
      .getByRole("button", { name: "Executar consulta", exact: true })
      .click();
    await expect(
      page.getByText(/Visualização formatada indisponível/),
    ).toBeVisible();
    await expect(
      page.getByRole("button", { name: "Formatada", exact: true }),
    ).toBeDisabled();
    await expect(page.getByRole("table")).toHaveCount(0);
  }
});
test("logs use selected IDs and allowed limits, split streams, copy and clear without Console history", async ({
  page,
}) => {
  await logs(page);
  for (const tail of ["100", "300", "500"]) {
    await page.getByLabel("Últimas linhas").selectOption(tail);
    await page
      .getByRole("button", { name: "Consultar logs", exact: true })
      .click();
    await expect(
      page.getByRole("region", { name: "Saída padrão dos logs" }),
    ).toContainText("test stdout");
    await expect(
      page.getByRole("region", { name: "Saída de erro dos logs" }),
    ).toContainText("test stderr");
  }
  const calls = await page.evaluate(() =>
    (window as any).calls.filter(
      (c: any) => c.command === "query_container_logs",
    ),
  );
  expect(calls.map((c: any) => c.args.request.tail)).toEqual([100, 300, 500]);
  expect(calls[0].args.request.containerId).toBe("a".repeat(64));
  await page
    .getByRole("button", { name: "Copiar resultado", exact: true })
    .click();
  expect(await page.evaluate(() => (window as any).copiedText)).toContain(
    "[stderr]\ntest stderr",
  );
  await page
    .getByLabel("Container", { exact: true })
    .selectOption("b".repeat(64));
  await expect(
    page.getByRole("region", { name: "Saída padrão dos logs" }),
  ).toHaveCount(0);
  await page
    .getByRole("button", { name: "Consultar logs", exact: true })
    .click();
  await expect(
    page.getByRole("button", { name: "Limpar logs", exact: true }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Limpar logs", exact: true }).click();
  await page
    .getByRole("button", { name: "Consultas Docker", exact: true })
    .click();
  await expect(page.getByText("Nenhuma consulta executada")).toBeVisible();
});
test("empty, timeout, execution error and truncation remain explicit", async ({
  page,
}) => {
  await logs(page);
  await page.evaluate(() => {
    (window as any).logOutput = "";
    (window as any).logError = "";
  });
  await page
    .getByRole("button", { name: "Consultar logs", exact: true })
    .click();
  await expect(
    page.getByText("O Docker não retornou logs nesta consulta."),
  ).toBeVisible();
  for (const status of ["timeout", "execution_error", "missing"]) {
    await page.evaluate((status) => {
      (window as any).logStatus = status;
    }, status);
    await page
      .getByRole("button", { name: "Atualizar logs", exact: true })
      .click();
    await expect(
      page.getByText(/A consulta não foi concluída integralmente/),
    ).toBeVisible();
  }
  await page.evaluate(() => {
    (window as any).logStatus = "execution_error";
    (window as any).logTruncated = true;
  });
  await page
    .getByRole("button", { name: "Atualizar logs", exact: true })
    .click();
  await expect(page.getByText("Saída truncada pelo limite.")).toBeVisible();
});
test("navigation does not repeat logs and container listing is deduplicated under StrictMode", async ({
  page,
}) => {
  await logs(page);
  await page
    .getByRole("button", { name: "Consultar logs", exact: true })
    .click();
  await expect(
    page.getByText("Consulta concluída", { exact: true }),
  ).toBeVisible();
  await page
    .getByRole("navigation")
    .getByRole("button", { name: "Visão geral" })
    .click();
  await page
    .getByRole("navigation")
    .getByRole("button", { name: "Console", exact: true })
    .click();
  expect(
    await page.evaluate(
      () =>
        (window as any).calls.filter(
          (c: any) => c.command === "query_container_logs",
        ).length,
    ),
  ).toBe(1);
  await page
    .getByRole("navigation")
    .getByRole("button", { name: "Containers", exact: true })
    .click();
  await expect(page.locator('strong:visible').filter({hasText:/^test-web$/})).toBeVisible();
  expect(
    await page.evaluate(
      () =>
        (window as any).calls.filter(
          (c: any) => c.command === "list_docker_containers",
        ).length,
    ),
  ).toBe(2);
});
test("logs controls fit narrow layout and do not auto-fetch after canceling context", async ({
  page,
}) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await logs(page);
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= window.innerWidth,
    ),
  ).toBeTruthy();
  expect(
    await page.evaluate(
      () =>
        (window as any).calls.filter(
          (c: any) => c.command === "query_container_logs",
        ).length,
    ),
  ).toBe(0);
});
