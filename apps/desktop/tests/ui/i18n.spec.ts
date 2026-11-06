import { test, expect } from "@playwright/test";

// The interface follows the system language and can be switched in Settings.
test.use({ locale: "en-US" });

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    const state = window as unknown as {
      __TAURI_INTERNALS__: object;
      testCalls: { command: string; args: any }[];
    };
    state.testCalls = [];
    state.__TAURI_INTERNALS__ = {
      invoke: async (command: string, args?: any) => {
        state.testCalls.push({ command, args });
        if (command === "doctor_environment")
          return { schemaVersion: 1, os: "linux", notes: [], checks: [] };
        throw new Error(`Unexpected IPC ${command}`);
      },
    };
  });
  await page.goto("/");
});

test("defaults to English when the system language is not Portuguese", async ({ page }) => {
  await expect(page.getByRole("heading", { name: "Your workspace, in focus." })).toBeVisible();
  await expect(page.getByRole("navigation").getByRole("button", { name: "Docker assistant" })).toBeVisible();
  expect(await page.evaluate(() => document.documentElement.lang)).toBe("en");
});

test("switching the language in Settings applies immediately and is remembered", async ({ page }) => {
  await page.getByRole("navigation").getByRole("button", { name: "Settings", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Settings", level: 1 })).toBeVisible();
  await page.getByLabel("Interface language").selectOption("pt");
  await expect(page.getByRole("heading", { name: "Configurações", level: 1 })).toBeVisible();
  await expect(page.getByRole("navigation").getByRole("button", { name: "Assistente Docker" })).toBeVisible();
  expect(await page.evaluate(() => document.documentElement.lang)).toBe("pt-BR");
  await page.reload();
  await expect(page.getByRole("heading", { name: "Seu workspace, em foco." })).toBeVisible();
});

test("backend calls carry the selected language", async ({ page }) => {
  await page.getByRole("navigation").getByRole("button", { name: "Environment", exact: true }).click();
  await page.getByRole("button", { name: "Check environment" }).click();
  await expect(page.getByText("Diagnostics finished.")).toBeVisible();
  const english = await page.evaluate(() => (window as any).testCalls);
  expect(english.find((c: any) => c.command === "doctor_environment").args.lang).toBe("en");

  await page.getByRole("navigation").getByRole("button", { name: "Settings", exact: true }).click();
  await page.getByLabel("Interface language").selectOption("pt");
  await page.getByRole("navigation").getByRole("button", { name: "Ambiente", exact: true }).click();
  await page.getByRole("button", { name: "Atualizar diagnóstico" }).click();
  await expect(page.getByText("Diagnóstico concluído.")).toBeVisible();
  const calls = await page.evaluate(() => (window as any).testCalls);
  const last = calls.filter((c: any) => c.command === "doctor_environment").pop();
  expect(last.args.lang).toBe("pt");
});

test("the Docker assistant is fully translated", async ({ page }) => {
  await page.getByRole("navigation").getByRole("button", { name: "Docker assistant" }).click();
  await expect(page.getByRole("heading", { name: "Which project do you want to put in Docker?" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Select folder" })).toBeVisible();
  await expect(page.getByText("STEP 1 OF 4")).toBeVisible();
  await expect(page.getByRole("button", { name: /Continue/ })).toBeDisabled();
});

test("the Contribute tab explains PIX for people outside Brazil", async ({ page }) => {
  await page.getByRole("navigation").getByRole("button", { name: "Contribute", exact: true }).click();
  await expect(page.getByText("PIX is Brazil's instant payment system.")).toBeVisible();
  await expect(page.getByRole("img", { name: /PIX QR code for/ })).toBeVisible();
});
