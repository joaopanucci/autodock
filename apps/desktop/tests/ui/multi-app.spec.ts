import { test, expect, type Page } from "@playwright/test";

test.use({ locale: "en-US" });

const app = (over: Record<string, unknown>) => ({
  framework: null,
  evidence: "x",
  startScript: null,
  packageManager: null,
  pythonFramework: null,
  hasDockerfile: false,
  selected: true,
  note: null,
  ...over,
});

const report = {
  schemaVersion: 1,
  project: "/Users/maria/shop",
  projectName: "shop",
  apps: [
    app({ path: "api", name: "api", stack: "go", role: "backend", framework: "go", evidence: "go.mod", port: 8080, hasDockerfile: true }),
    app({ path: "mobile", name: "mobile", stack: "nodejs", role: "mobile", framework: "expo", evidence: "package.json", port: 8081, selected: false, startScript: "start", packageManager: "npm", note: "Mobile app: the dev server must be reachable from the phone and is outside this version." }),
    app({ path: "web", name: "web", stack: "nodejs", role: "frontend", framework: "nextjs", evidence: "package.json", port: 3000, startScript: "dev", packageManager: "npm" }),
  ],
  skipped: [{ path: "packages/ui", reason: "Node.js package without a start script (library)" }],
  existingCompose: null,
  warnings: [],
};

const plan = {
  schemaVersion: 1,
  project: "/Users/maria/shop",
  seed: "0".repeat(64),
  ports: [
    { name: "api", containerPort: 8080, hostPort: 8081, preferredPort: 8080 },
    { name: "web", containerPort: 3000, hostPort: 3000, preferredPort: 3000 },
    { name: "postgres", containerPort: 5432, hostPort: 5432, preferredPort: 5432 },
  ],
  notes: ["Once it is up: api → http://localhost:8081 · web → http://localhost:3000."],
  files: [
    { name: "api/Dockerfile", description: "How the image of api is built.", content: "FROM golang:1.26-alpine AS build", sensitive: false, exists: true },
    { name: "web/Dockerfile", description: "How the image of web is built.", content: "FROM node:22-alpine", sensitive: false, exists: false },
    { name: "docker-compose.yml", description: "Runs every app and the chosen services.", content: "name: shop\nservices:\n  api:\n    build: ./api", sensitive: false, exists: false },
    { name: ".env.docker", description: "Strong passwords.", content: "POSTGRES_PASSWORD=x", sensitive: true, exists: false },
  ],
};

async function open(page: Page, running = false) {
  await page.addInitScript(
    ({ report, plan, running }) => {
      const w = window as any;
      w.calls = [];
      w.up = running;
      w.__TAURI_INTERNALS__ = {
        invoke: async (command: string, args: any) => {
          w.calls.push({ command, args });
          switch (command) {
            case "detect_stacks": return [];
            case "inspect_folder": return false;
            case "detect_ides": return [];
            case "detect_apps": return report;
            case "preview_multi_setup": return plan;
            case "apply_multi_setup": return { written: args.files, skipped: [] };
            case "compose_files": return ["docker-compose.yml"];
            case "compose_status":
              return {
                schemaVersion: 1, status: "available", detail: "",
                services: w.up
                  ? [
                      { service: "api", container: "shop-api", state: "running", status: "Up", health: null, ports: ["8081->8080/tcp"] },
                      { service: "postgres", container: "shop-postgres", state: "running", status: "Up", health: "healthy", ports: ["127.0.0.1:5432->5432/tcp"] },
                    ]
                  : [],
              };
            case "compose_run":
              if (args.request.action === "up") w.up = true;
              if (args.request.action !== "up") w.up = false;
              return { schemaVersion: 1, action: args.request.action, command: `docker compose -f ${args.request.file} ${args.request.action}`, status: "available", exitCode: 0, succeeded: true, output: "done", detail: "", durationMs: 2000 };
            default: throw new Error(`Unexpected IPC ${command}`);
          }
        },
      };
    },
    { report, plan, running },
  );
  await page.goto("/");
  await page.getByRole("navigation").getByRole("button", { name: "Docker assistant", exact: true }).click();
  await page.getByLabel("Project folder for Docker").fill("/Users/maria/shop");
}

test("a folder with several apps offers to set them all up", async ({ page }) => {
  await open(page);
  await expect(page.getByText("We found 3 apps in this project")).toBeVisible();
  await page.getByRole("button", { name: "Set up all apps" }).click();

  // step 1: apps, with the mobile app unticked and the notes visible
  await expect(page.getByRole("heading", { name: "Which apps go to Docker?" })).toBeVisible();
  await expect(page.getByRole("checkbox", { name: "api", exact: true })).toBeChecked();
  await expect(page.getByRole("checkbox", { name: "web", exact: true })).toBeChecked();
  await expect(page.getByRole("checkbox", { name: "mobile", exact: true })).not.toBeChecked();
  await expect(page.getByText("outside this version")).toBeVisible();
  await expect(page.getByText("Already has a Dockerfile")).toBeVisible();
  await expect(page.getByText("1 folder skipped")).toBeVisible();

  // validation blocks bad names
  const continueButton = page.getByRole("button", { name: /^Continue/ });
  await page.locator(".multi-app-field input").first().fill("postgres");
  await expect(page.getByText(/not the name of a service/)).toBeVisible();
  await expect(continueButton).toBeDisabled();
  await page.locator(".multi-app-field input").first().fill("api");
  await expect(continueButton).toBeEnabled();
  await continueButton.click();

  // step 2: services
  await expect(page.getByRole("heading", { name: "Shared services" })).toBeVisible();
  await page.locator(".choice", { hasText: "PostgreSQL" }).click();
  await page.getByRole("button", { name: /^Review/ }).click();

  // step 3: review shows every file and sends the right request
  await expect(page.getByRole("heading", { name: "Review before creating" })).toBeVisible();
  await expect(page.getByText("api/Dockerfile")).toBeVisible();
  await expect(page.getByText("docker-compose.yml").first()).toBeVisible();
  const request = await page.evaluate(() => (window as any).calls.find((c: any) => c.command === "preview_multi_setup").args.request);
  expect(request.projectName).toBe("shop");
  expect(request.services).toEqual(["postgres"]);
  expect(request.apps.map((a: any) => [a.name, a.path, a.stack, a.role, a.port])).toEqual([
    ["api", "api", "go", "backend", 8080],
    ["web", "web", "nodejs", "frontend", 3000],
  ]);
  // the file that already exists cannot be selected
  await expect(page.getByRole("checkbox", { name: "api/Dockerfile" })).toBeDisabled();
  await page.getByRole("button", { name: "Create 3 files" }).click();
  await expect(page.getByText("3 files created.")).toBeVisible();
  const applied = await page.evaluate(() => (window as any).calls.find((c: any) => c.command === "apply_multi_setup").args);
  expect(applied.files).toEqual(["web/Dockerfile", "docker-compose.yml", ".env.docker"]);
  expect(applied.request.seed).toBe("0".repeat(64));
  expect(applied.request.ports).toHaveLength(3);

  // step 4: nothing runs without the confirmation
  await page.getByRole("button", { name: "Go to run" }).click();
  await expect(page.getByRole("heading", { name: "Run everything" })).toBeVisible();
  await expect(page.getByText("docker compose -f docker-compose.yml up -d --build")).toBeVisible();
  await expect(page.getByRole("button", { name: "Run everything" })).toBeDisabled();
  await expect(page.getByText("There are no containers for this project yet.")).toBeVisible();
  await page.getByLabel("I understand this runs commands in my Docker").check();
  await page.getByRole("button", { name: "Run everything" }).click();
  await expect(page.getByText("Finished in 2 s.")).toBeVisible();
  const run = await page.evaluate(() => (window as any).calls.filter((c: any) => c.command === "compose_run").map((c: any) => c.args.request));
  expect(run).toEqual([{ path: "/Users/maria/shop", file: "docker-compose.yml", action: "up", confirmed: true }]);

  // the state table and the URLs appear once the services are up
  await expect(page.getByRole("table")).toContainText("postgres");
  await expect(page.getByRole("table")).toContainText("healthy");
  await expect(page.getByText("http://localhost:8081")).toBeVisible();
  await expect(page.getByText("http://localhost:3000")).toBeVisible();

  // stopping uses the same confirmation and updates the table
  await page.getByRole("button", { name: "Stop" }).click();
  await expect(page.getByText("There are no containers for this project yet.")).toBeVisible({ timeout: 8000 });
});

test("the user can go back from the multi-app screen to the single-app wizard", async ({ page }) => {
  await open(page);
  await page.getByRole("button", { name: "Set up all apps" }).click();
  await page.getByRole("button", { name: "Back to a single app" }).click();
  await expect(page.getByRole("heading", { name: "Which project do you want to put in Docker?" })).toBeVisible();
  await expect(page.getByText("We found 3 apps in this project")).toBeVisible();
});
