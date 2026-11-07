import { test, expect } from "@playwright/test";

test.use({ locale: "en-US" });

const TOTAL = 13;

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

const openLearn = (page: import("@playwright/test").Page) =>
  page.getByRole("navigation").getByRole("button", { name: "Learn", exact: true }).click();

test("learn lists lessons, opens one, tracks progress and switches language", async ({ page }) => {
  const nav = page.getByRole("navigation");
  await openLearn(page);
  await expect(page.getByRole("heading", { name: "Learn Docker", level: 1 })).toBeVisible();
  await expect(page.getByText(`0 of ${TOTAL} lessons completed`).first()).toBeVisible();

  await page.getByRole("button", { name: /Start.*How ports work/ }).click();
  await expect(page.getByRole("heading", { name: "How ports work" })).toBeVisible();
  await expect(page.locator(".learn-code").first()).toContainText("8080:80");
  await page.getByRole("button", { name: "Mark as completed" }).click();
  await page.getByRole("button", { name: "All lessons" }).click();
  await expect(page.getByText(`1 of ${TOTAL} lessons completed`).first()).toBeVisible();

  await page.reload();
  await openLearn(page);
  await expect(page.getByText(`1 of ${TOTAL} lessons completed`).first()).toBeVisible();

  await nav.getByRole("button", { name: "Settings", exact: true }).click();
  await page.getByLabel("Interface language").selectOption("pt");
  await nav.getByRole("button", { name: "Aprender", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Aprender Docker", level: 1 })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Como funcionam as portas" })).toBeVisible();
});

test("a lesson can send the reader to the matching screen", async ({ page }) => {
  await openLearn(page);
  await page.getByRole("button", { name: /Start.*Reading CPU, memory and network/ }).click();
  await page.getByRole("button", { name: "Open Monitoring" }).click();
  await expect(page.getByRole("heading", { name: "Monitoring", level: 1 })).toBeVisible();
});

test("lessons are grouped in tracks and can be searched and filtered", async ({ page }) => {
  await openLearn(page);
  await expect(page.getByRole("heading", { name: "First steps" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Building your own" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Running and fixing" })).toBeVisible();
  await expect(page.locator(".learn-card")).toHaveCount(TOTAL);

  await page.getByLabel("Search lessons").fill("dockerfile");
  await expect(page.locator(".learn-card")).toHaveCount(1);
  await expect(page.getByRole("heading", { name: "Writing a Dockerfile" })).toBeVisible();
  await page.getByLabel("Search lessons").fill("zzzz-nothing");
  await expect(page.getByText("No lessons found")).toBeVisible();
  await page.getByLabel("Search lessons").fill("");

  await page.getByRole("button", { name: "Beginner", exact: true }).click();
  const beginners = await page.locator(".learn-card").count();
  expect(beginners).toBeGreaterThan(0);
  expect(beginners).toBeLessThan(TOTAL);
  await page.getByRole("button", { name: "All", exact: true }).click();
  await expect(page.locator(".learn-card")).toHaveCount(TOTAL);
});

test("continue resumes the last lesson and the pager moves between lessons", async ({ page }) => {
  await openLearn(page);
  await page.getByRole("button", { name: /Continue: What Docker is/ }).click();
  await expect(page.getByRole("heading", { name: "What Docker is and why use it" })).toBeVisible();
  await expect(page.getByText(/Lesson 1 of 13/)).toBeVisible();
  await page.getByRole("button", { name: /Next lesson/ }).click();
  await expect(page.getByRole("heading", { name: "Docker on Windows" })).toBeVisible();
  await page.getByRole("button", { name: "All lessons" }).click();
  await expect(page.getByRole("button", { name: /Continue: Docker on Windows/ })).toBeVisible();
});

test("the self-check quiz explains right and wrong answers", async ({ page }) => {
  await openLearn(page);
  await page.getByRole("button", { name: /Start.*How ports work/ }).click();
  const first = page.locator(".learn-question").first();
  await first.getByLabel("8080").check();
  await expect(first.getByText("Not quite.")).toBeVisible();
  await first.getByLabel("80", { exact: true }).check();
  await expect(first.getByText("Correct.")).toBeVisible();
  await page.locator(".learn-question").nth(1).getByLabel("So only your own computer can reach it.").check();
  await expect(page.getByText("2 of 2 correct")).toBeVisible();
});

test("commands follow the chosen terminal and the choice is remembered", async ({ page }) => {
  await openLearn(page);
  await page.getByRole("button", { name: /Start.*Docker on Windows/ }).click();
  const code = page.locator(".learn-code").filter({ hasText: "-w /app node:22" });
  await code.getByRole("tab", { name: "PowerShell" }).click();
  await expect(code).toContainText("${PWD}:/app");
  await code.getByRole("tab", { name: "CMD" }).click();
  await expect(code).toContainText("%cd%:/app");
  await code.getByRole("tab", { name: "Bash" }).click();
  await expect(code).toContainText('"$PWD":/app');

  await page.reload();
  await openLearn(page);
  await page.getByRole("tab", { name: "Quick reference" }).click();
  await expect(page.getByRole("tab", { name: "Bash" })).toHaveAttribute("aria-selected", "true");
  await expect(page.getByText('docker run --rm -v "$PWD":/app -w /app node:22 npm test')).toBeVisible();
  await page.getByRole("tab", { name: "PowerShell" }).click();
  await expect(page.getByText('docker run --rm -v "${PWD}:/app" -w /app node:22 npm test')).toBeVisible();
});

test("quick reference and glossary can be searched", async ({ page }) => {
  await openLearn(page);
  await page.getByRole("tab", { name: "Quick reference" }).click();
  await page.getByLabel("Search commands").fill("prune");
  await expect(page.locator(".learn-reference").getByText("docker container prune")).toBeVisible();
  await expect(page.locator(".learn-reference").getByText("docker ps -a")).toHaveCount(0);
  await page.getByLabel("Search commands").fill("zzzz-nothing");
  await expect(page.getByText("No command matches your search.")).toBeVisible();

  await page.getByRole("tab", { name: "Glossary" }).click();
  await page.getByLabel("Search terms").fill("wsl");
  await expect(page.getByRole("term")).toHaveCount(1);
  await expect(page.getByRole("term")).toHaveText("WSL 2");
});
