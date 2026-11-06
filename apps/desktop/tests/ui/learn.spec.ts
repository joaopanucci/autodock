import { test, expect } from "@playwright/test";

test.use({ locale: "en-US" });

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    (window as any).__TAURI_INTERNALS__ = {
      invoke: async (command: string) => {
        if (command === "doctor_environment")
          return { schemaVersion: 1, os: "linux", notes: [], checks: [] };
        throw new Error(`Unexpected IPC ${command}`);
      },
    };
  });
  await page.goto("/");
});

test("learn lists lessons, opens one, tracks progress and switches language", async ({ page }) => {
  const nav = page.getByRole("navigation");
  await nav.getByRole("button", { name: "Learn", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Learn Docker", level: 1 })).toBeVisible();
  await expect(page.getByText("0 of 6 lessons completed").first()).toBeVisible();

  await page.getByRole("button", { name: /Start.*How ports work/ }).click();
  await expect(page.getByRole("heading", { name: "How ports work" })).toBeVisible();
  await expect(page.locator(".learn-code").first()).toContainText("8080:80");
  await page.getByRole("button", { name: "Mark as completed" }).click();
  await page.getByRole("button", { name: "All lessons" }).click();
  await expect(page.getByText("1 of 6 lessons completed").first()).toBeVisible();

  await page.reload();
  await nav.getByRole("button", { name: "Learn", exact: true }).click();
  await expect(page.getByText("1 of 6 lessons completed").first()).toBeVisible();

  await nav.getByRole("button", { name: "Settings", exact: true }).click();
  await page.getByLabel("Interface language").selectOption("pt");
  await nav.getByRole("button", { name: "Aprender", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Aprender Docker", level: 1 })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Como funcionam as portas" })).toBeVisible();
});

test("a lesson can send the reader to the matching screen", async ({ page }) => {
  await page.getByRole("navigation").getByRole("button", { name: "Learn", exact: true }).click();
  await page.getByRole("button", { name: /Start.*Reading CPU, memory and network/ }).click();
  await page.getByRole("button", { name: "Open Monitoring" }).click();
  await expect(page.getByRole("heading", { name: "Monitoring", level: 1 })).toBeVisible();
});
