import { test, expect, type Page } from "@playwright/test";

test.use({ locale: "en-US" });

type Suggestion = {
  stack: string;
  evidence: string;
  scripts: string[];
  packageManager: string | null;
  pythonFramework: string | null;
  vite: boolean;
  nodeFramework?: string | null;
};

async function openAssistant(page: Page, detected: Suggestion[], empty: boolean) {
  await page.addInitScript(
    ({ detected, empty }) => {
      (window as any).__TAURI_INTERNALS__ = {
        invoke: async (command: string) => {
          if (command === "detect_stacks") return detected;
          if (command === "inspect_folder") return empty;
          if (command === "detect_ides") return [];
          throw new Error(`Unexpected IPC ${command}`);
        },
      };
    },
    { detected, empty },
  );
  await page.goto("/");
  await page.getByRole("navigation").getByRole("button", { name: "Docker assistant", exact: true }).click();
  await page.getByLabel("Project folder for Docker").fill("C:\\work\\my-new-app");
}

test("a new project offers the popular technologies with their logos and official starters", async ({ page }) => {
  await openAssistant(page, [], true);
  await page.getByRole("button", { name: "Create new project" }).click();

  const technologies = page.getByRole("radiogroup", { name: "Technology" });
  for (const name of ["Node.js", "Java / Spring Boot", "Python", "PHP / Laravel", "Go", ".NET / ASP.NET Core"]) {
    await expect(technologies.getByRole("radio", { name: new RegExp(name.replace(/[./]/g, "\\$&")) })).toBeVisible();
  }
  // Rails has no starter, so it is only offered for existing projects.
  await expect(technologies.getByRole("radio", { name: /Ruby on Rails/ })).toHaveCount(0);
  await expect(technologies.locator(".tech-logo")).toHaveCount(6);

  await technologies.getByRole("radio", { name: /^.*Node\.js/ }).click();
  const starters = page.getByRole("radiogroup", { name: "How to start" });
  for (const name of ["Vite + React", "Vite + Vue", "Next.js", "NestJS"]) {
    await expect(starters.getByRole("radio", { name: new RegExp(name.replace(/[+.]/g, "\\$&")) })).toBeVisible();
  }
  await starters.getByRole("radio", { name: /Next\.js/ }).click();
  await expect(starters.getByRole("radio", { name: /Next\.js/ })).toHaveAttribute("aria-checked", "true");
  await expect(starters.getByRole("radio", { name: /Next\.js/ })).toContainText("Official installer");
  await expect(page.getByLabel("Application port")).toHaveValue("3000");
  await expect(page.getByLabel("Node.js version")).toHaveValue("22");
  // NestJS needs the npm that ships with Node 24.
  await starters.getByRole("radio", { name: /NestJS/ }).click();
  await expect(page.getByLabel("Node.js version")).toHaveValue("24");
  await starters.getByRole("radio", { name: /Next\.js/ }).click();
  await expect(page.getByLabel("Node.js version")).toHaveValue("22");
});

test("a detected Next.js project is recognised and starts with the dev script", async ({ page }) => {
  await openAssistant(
    page,
    [
      {
        stack: "nodejs",
        evidence: "package.json",
        scripts: ["dev", "build", "start", "lint"],
        packageManager: "npm",
        pythonFramework: null,
        vite: false,
        nodeFramework: "nextjs",
      },
    ],
    false,
  );
  await expect(page.getByText("Found: Node.js · Next.js (package.json)")).toBeVisible();
  await page.getByRole("button", { name: /Continue/ }).click();
  await expect(page.getByLabel("Script that starts the app")).toHaveValue("dev");
  await expect(page.getByLabel("Node.js version")).toHaveValue("22");
});

test("a detected NestJS project starts with start:dev on Node 24", async ({ page }) => {
  await openAssistant(
    page,
    [
      {
        stack: "nodejs",
        evidence: "package.json",
        scripts: ["build", "start", "start:dev"],
        packageManager: "npm",
        pythonFramework: null,
        vite: false,
        nodeFramework: "nestjs",
      },
    ],
    false,
  );
  await page.getByRole("button", { name: /Continue/ }).click();
  await expect(page.getByLabel("Script that starts the app")).toHaveValue("start:dev");
  await expect(page.getByLabel("Node.js version")).toHaveValue("24");
});

test("an existing Rails project can be chosen", async ({ page }) => {
  await openAssistant(
    page,
    [{ stack: "rails", evidence: "Gemfile (rails)", scripts: [], packageManager: null, pythonFramework: null, vite: false }],
    false,
  );
  await expect(page.getByText("Found: Ruby on Rails (Gemfile (rails))")).toBeVisible();
  await page.getByRole("button", { name: /Continue/ }).click();
  const rails = page.getByRole("radiogroup", { name: "Technology" }).getByRole("radio", { name: /Ruby on Rails/ });
  await expect(rails).toHaveAttribute("aria-checked", "true");
  await expect(page.getByLabel("Ruby version")).toHaveValue("3.4");
});
