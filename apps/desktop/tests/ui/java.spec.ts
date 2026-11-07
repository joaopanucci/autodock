import { test, expect, type Page } from "@playwright/test";

test.use({ locale: "en-US" });

const project = (over: Record<string, unknown> = {}) => ({
  schemaVersion: 1,
  path: "/Users/maria/legacy-shop",
  name: "legacy-shop",
  buildTool: "maven",
  packaging: "jar",
  javaLevel: 7,
  javaDeclared: "1.7",
  springBoot: "1.5.22.RELEASE",
  modules: [],
  jdkApis: [
    { name: "javax.xml.bind", files: 2, fixable: true },
    { name: "sun.* (JDK internals)", files: 1, fixable: false },
  ],
  upgradeAvailable: true,
  upgradeSupported: true,
  unsupportedReason: null,
  ...over,
});

const plan = {
  schemaVersion: 1,
  project: project(),
  fromJava: 7,
  toJava: 25,
  fromSpringBoot: "1.5.22.RELEASE",
  toSpringBoot: "4.0.x",
  image: "maven:3.9-eclipse-temurin-25",
  hops: [
    { id: "spring-boot-2.7", label: "Spring Boot to 2.7 (includes JUnit 4 → 5)", recipe: "r1", checked: false },
    { id: "java-17", label: "Java 17 (compiler, plugins and APIs removed from the JDK)", recipe: "r2", checked: true },
    { id: "spring-boot-3.5", label: "Spring Boot to 3.5 (javax → jakarta)", recipe: "r3", checked: true },
    { id: "spring-boot-4.0", label: "Spring Boot to 4.0", recipe: "r4", checked: true },
    { id: "java-25", label: "Java 25 (the current LTS)", recipe: "r5", checked: true },
  ],
  fixups: [{ module: "", dependency: "jakarta.xml.bind:jakarta.xml.bind-api:2.3.3", because: "javax.xml.bind" }],
  destination: "/Users/maria/legacy-shop-java25",
  command: "docker run --rm maven:3.9-eclipse-temurin-25 sh -c '…'",
  notes: ["The original project is not modified: AutoDock creates a copy next to it and upgrades the copy."],
  warnings: ["sun.* (JDK internals) (1 file(s)) is gone from the JDK and has no automatic replacement."],
  links: [{ label: "OpenRewrite recipes", url: "https://docs.openrewrite.org/recipes/java/migrate" }],
};

type Options = { project?: Record<string, unknown>; fail?: boolean; none?: boolean };

async function open(page: Page, options: Options = {}) {
  await page.addInitScript(
    ({ plan, options, base }) => {
      const w = window as any;
      w.calls = [];
      w.__TAURI_INTERNALS__ = {
        invoke: async (command: string, args: any) => {
          w.calls.push({ command, args });
          switch (command) {
            case "java_inspect":
              return options.none ? null : { ...base, ...options.project };
            case "java_upgrade_plan":
              // like the Rust side: the copy goes into the chosen folder, else next to the project
              return args.destination
                ? { ...plan, destination: `${args.destination}/legacy-shop-java25` }
                : plan;
            case "java_upgrade_run":
              if (options.fail) {
                return {
                  schemaVersion: 1, succeeded: false, destination: plan.destination, fromJava: 7, toJava: 25,
                  fromSpringBoot: "1.5.22.RELEASE", toSpringBoot: "4.0.x",
                  hops: plan.hops.map((h: any, i: number) => ({
                    id: h.id, label: h.label, status: i === 0 ? "ok" : i === 1 ? "failed" : "skipped",
                    compiled: i === 1 ? false : null,
                  })),
                  failedHop: "java-17", fixups: [], changes: { modified: [], added: [], removed: [], truncated: false, total: 0 },
                  tests: null,
                  output: "CHECK java-17 failed\n[ERROR] /work/src/Legacy.java:[5,28] cannot find symbol",
                  detail: "The upgrade stopped at step java-17. The original project was not modified; the copy was left in /Users/maria/legacy-shop-java25 for you to examine.",
                  durationMs: 8000, nextSteps: ["Read the errors of step java-17 below."],
                };
              }
              return {
                schemaVersion: 1, succeeded: true, destination: plan.destination, fromJava: 7, toJava: 25,
                fromSpringBoot: "1.5.22.RELEASE", toSpringBoot: "4.0.x",
                hops: plan.hops.map((h: any) => ({ id: h.id, label: h.label, status: "ok", compiled: h.checked ? true : null })),
                failedHop: null,
                fixups: plan.fixups,
                changes: { modified: ["pom.xml", "src/main/java/Product.java"], added: [], removed: ["README.md"], truncated: false, total: 3 },
                tests: { run: 2, failures: 0, errors: 0, skipped: 0, passed: true },
                output: "", detail: "", durationMs: 312000,
                nextSteps: ["Compare the copy with the original project and review what changed.", "Install JDK 25 to build on your computer."],
              };
            case "plugin:dialog|open":
              return String(args?.options?.title ?? "").includes("copy will be created")
                ? "/Volumes/backup"
                : "/Users/maria/legacy-shop";
            default:
              throw new Error(`Unexpected IPC ${command}`);
          }
        },
      };
    },
    { plan, options, base: project() },
  );
  await page.goto("/");
  await page.getByRole("navigation").getByRole("button", { name: "Java" }).click();
}

const calls = (page: Page, command: string) =>
  page.evaluate((name) => (window as any).calls.filter((c: any) => c.command === name).map((c: any) => c.args), command);

async function choose(page: Page) {
  await page.getByRole("button", { name: "Choose folder" }).click();
}

test("shows the Java and Spring Boot of a legacy project, plans and upgrades a copy after a confirmation", async ({ page }) => {
  await open(page);
  await expect(page.getByRole("heading", { name: "Java projects" })).toBeVisible();
  await expect(page.getByText("No project chosen")).toBeVisible();
  // Nothing is read until the user picks a folder.
  expect(await calls(page, "java_inspect")).toEqual([]);

  await choose(page);
  const row = page.locator(".expo-projects");
  await expect(row).toContainText("legacy-shop");
  await expect(row).toContainText("Java 1.7");
  await expect(row).toContainText("Spring Boot 1.5.22.RELEASE");
  await expect(page.getByText("javax.xml.bind")).toBeVisible();
  await expect(page.getByText(/needs a manual rewrite/)).toBeVisible();
  expect(await calls(page, "java_inspect")).toEqual([{ path: "/Users/maria/legacy-shop", lang: "en" }]);

  await page.getByRole("button", { name: "Plan the upgrade" }).click();
  await expect(
    page.getByRole("heading", { name: "legacy-shop: Java 7 → 25 and Spring Boot 1.5.22.RELEASE → 4.0.x" }),
  ).toBeVisible();
  await expect(page.getByText("Spring Boot to 3.5 (javax → jakarta)")).toBeVisible();
  await expect(page.getByText("jakarta.xml.bind:jakarta.xml.bind-api:2.3.3")).toBeVisible();
  await expect(page.getByText("/Users/maria/legacy-shop-java25").first()).toBeVisible();
  await expect(page.getByText(/is gone from the JDK/)).toBeVisible();

  // nothing runs until the user confirms
  const start = page.locator(".multi-run-actions").getByRole("button", { name: "Create the upgraded copy" });
  await expect(start).toBeDisabled();
  expect(await calls(page, "java_upgrade_run")).toEqual([]);
  await page.getByLabel(/I understand AutoDock will copy the project/).check();
  await start.click();

  await expect(page.getByText("Upgraded copy created in 312 s")).toBeVisible();
  await expect(page.getByText("2 tests, all passed (0 skipped)")).toBeVisible();
  await expect(page.locator(".java-hops li")).toHaveCount(5);
  await expect(page.locator(".java-hops li").first()).toContainText("done");
  await page.getByText("Files that differ from the original (3)").click();
  await expect(page.getByText("src/main/java/Product.java")).toBeVisible();
  await expect(page.getByText("Install JDK 25 to build on your computer.")).toBeVisible();
  const run = await calls(page, "java_upgrade_run");
  expect(run.map((r: any) => r.request)).toEqual([{ path: "/Users/maria/legacy-shop", confirmed: true }]);
});

test("a failed upgrade names the step, shows the compiler output and says the original is untouched", async ({ page }) => {
  await open(page, { fail: true });
  await choose(page);
  await page.getByRole("button", { name: "Plan the upgrade" }).click();
  await page.getByLabel(/I understand AutoDock will copy the project/).check();
  await page.locator(".multi-run-actions").getByRole("button", { name: "Create the upgraded copy" }).click();
  await expect(page.getByText("The upgrade did not finish", { exact: true })).toBeVisible();
  await expect(page.getByText(/The original project was not modified/)).toBeVisible();
  await expect(page.getByText("cannot find symbol")).toBeVisible();
  await expect(page.locator(".java-hops li").nth(1)).toContainText("failed");
  await expect(page.locator(".java-hops li").nth(1)).toContainText("did not compile");
  await expect(page.locator(".java-hops li").nth(2)).toContainText("not run");
});

test("a project AutoDock cannot upgrade explains why and offers no plan", async ({ page }) => {
  await open(page, {
    project: {
      buildTool: "gradle",
      upgradeAvailable: false,
      upgradeSupported: false,
      unsupportedReason: "Gradle projects are not supported yet: the upgrade uses Maven.",
      jdkApis: [],
      springBoot: null,
      javaDeclared: null,
      javaLevel: null,
    },
  });
  await choose(page);
  await expect(page.locator(".expo-projects")).toContainText("Gradle projects are not supported yet");
  await expect(page.getByRole("button", { name: "Plan the upgrade" })).toHaveCount(0);
});

test("a project that is already current offers no upgrade", async ({ page }) => {
  await open(page, {
    project: { javaLevel: 25, javaDeclared: "25", springBoot: "4.0.9", jdkApis: [], upgradeAvailable: false },
  });
  await choose(page);
  await expect(page.locator(".expo-projects")).toContainText("Already up to date");
  await expect(page.getByRole("button", { name: "Plan the upgrade" })).toHaveCount(0);
});

test("a folder that is not a Java project is reported", async ({ page }) => {
  await open(page, { none: true });
  await choose(page);
  await expect(page.getByRole("alert")).toContainText("Choose a folder to see the project");
});

test("explains the Java upgrade before a folder is chosen and folds away once there is a project", async ({ page }) => {
  await open(page);
  const how = page.getByTestId("java-how");
  await expect(page.getByRole("heading", { name: "From old Java to Java 25, without touching your project" })).toBeVisible();
  await expect(how).toHaveAttribute("open", "");
  await expect(how.locator(".java-journey li")).toHaveCount(5);
  await expect(how).toContainText("Spring Boot 3.5");
  await expect(how).toContainText("javax.* with jakarta.*");
  await expect(how).toContainText("What AutoDock fixes by itself");
  await expect(how).toContainText("What is left for you");
  await expect(how).toContainText("The original is never modified");
  await expect(how).toContainText("A tested example");

  // choosing a project folds it (the person can open it again)
  await choose(page);
  await expect(page.locator(".expo-projects")).toContainText("legacy-shop");
  await expect(how).not.toHaveAttribute("open", "");
  await how.getByText("How the Java upgrade works").click();
  await expect(how).toHaveAttribute("open", "");
  await expect(how.locator(".java-journey li").first()).toBeVisible();
});

test("the explanation fits a narrow window without horizontal scrolling", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 800 });
  await open(page);
  const how = page.getByTestId("java-how");
  await expect(how.locator(".java-journey li")).toHaveCount(5);
  await how.scrollIntoViewIfNeeded();
  const overflow = await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
  expect(overflow).toBeLessThanOrEqual(0);
});

test.describe("in Portuguese", () => {
  test.use({ locale: "pt-BR" });

  test("the explanation is written in Portuguese", async ({ page }) => {
    await open(page);
    const how = page.getByTestId("java-how");
    await expect(page.getByRole("heading", { name: "Do Java antigo ao Java 25, sem mexer no seu projeto" })).toBeVisible();
    await expect(how.locator(".java-journey li")).toHaveCount(5);
    await expect(how).toContainText("O que o AutoDock resolve sozinho");
    await expect(how).toContainText("O que sobra para você");
    await expect(how).toContainText("O original nunca é alterado");
  });
});

test("the person can choose where the upgraded copy is created", async ({ page }) => {
  await open(page);
  await choose(page);
  await page.getByRole("button", { name: "Plan the upgrade" }).click();
  // by default the copy goes next to the project and the plan is made without a destination
  await expect(page.getByText("/Users/maria/legacy-shop-java25").first()).toBeVisible();
  expect(await calls(page, "java_upgrade_plan")).toEqual([{ path: "/Users/maria/legacy-shop", lang: "en" }]);
  await expect(page.getByRole("button", { name: "Back to next to the project" })).toHaveCount(0);

  await page.getByRole("button", { name: "Choose another folder" }).click();
  await expect(page.getByText("/Volumes/backup/legacy-shop-java25").first()).toBeVisible();
  expect(await calls(page, "java_upgrade_plan")).toEqual([
    { path: "/Users/maria/legacy-shop", lang: "en" },
    { path: "/Users/maria/legacy-shop", destination: "/Volumes/backup", lang: "en" },
  ]);

  // the choice travels with the confirmation
  await page.getByLabel(/I understand AutoDock will copy the project/).check();
  await page.locator(".multi-run-actions").getByRole("button", { name: "Create the upgraded copy" }).click();
  await expect(page.getByText("Upgraded copy created in 312 s")).toBeVisible();
  const run = await calls(page, "java_upgrade_run");
  expect(run.map((r: any) => r.request)).toEqual([
    { path: "/Users/maria/legacy-shop", confirmed: true, destination: "/Volumes/backup" },
  ]);
});

test("the chosen folder can be dropped to go back to the default place", async ({ page }) => {
  await open(page);
  await choose(page);
  await page.getByRole("button", { name: "Plan the upgrade" }).click();
  await page.getByRole("button", { name: "Choose another folder" }).click();
  await expect(page.getByText("/Volumes/backup/legacy-shop-java25").first()).toBeVisible();
  await page.getByRole("button", { name: "Back to next to the project" }).click();
  await expect(page.getByText("/Users/maria/legacy-shop-java25").first()).toBeVisible();
  await expect(page.getByRole("button", { name: "Back to next to the project" })).toHaveCount(0);
  const plans = await calls(page, "java_upgrade_plan");
  expect(plans[plans.length - 1]).toEqual({ path: "/Users/maria/legacy-shop", lang: "en" });
});
