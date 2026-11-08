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

const convertPlan = (over: Record<string, unknown> = {}) => ({
  schemaVersion: 1,
  path: "/Users/maria/legacy-shop",
  name: "legacy-shop",
  destination: "/Users/maria/legacy-shop-maven",
  groupId: "com.acme",
  artifactId: "legacy-shop",
  version: "1.0.0-SNAPSHOT",
  packaging: "jar",
  javaLevel: 6,
  javaLevelDeclared: true,
  encoding: "UTF-8",
  counts: {
    mainSources: 120, testSources: 31, mainResources: 12, testResources: 3, webappFiles: 0,
    movedToTheirPackage: 2, librariesKnown: 2, librariesLocal: 1, notCopied: 9,
  },
  moves: [{ from: "src/com/acme/shop/App.java", to: "src/main/java/com/acme/shop/App.java" }],
  dependencies: [
    { groupId: "commons-lang", artifactId: "commons-lang", version: "2.6", scope: "compile", jar: "lib/commons-lang-2.6.jar", known: true },
    { groupId: "junit", artifactId: "junit", version: "4.12", scope: "test", jar: "lib/junit-4.12.jar", known: true },
    { groupId: "local", artifactId: "acme-legacy", version: "1.0", scope: "system", jar: "lib/acme-legacy-1.0.jar", known: false },
  ],
  leftOut: ["build.xml"],
  pom: "<project>\n  <groupId>com.acme</groupId>\n</project>\n",
  notes: ["The original project is not modified: AutoDock creates a copy."],
  warnings: ["src2/com/acme/A.java would land where src/com/acme/A.java already goes: only the first was kept."],
  ...over,
});

type Options = {
  project?: Record<string, unknown>;
  fail?: boolean;
  partial?: boolean;
  none?: boolean;
  /** What `java_map` answers (when absent the command fails, as a folder that cannot be read). */
  map?: Record<string, unknown>;
  /** What `java_convert_plan` answers; a string makes it fail with that text. */
  convert?: Record<string, unknown> | string;
};

async function open(page: Page, options: Options = {}) {
  await page.addInitScript(
    ({ plan, options, base, convertBase }) => {
      const w = window as any;
      w.calls = [];
      w.__TAURI_INTERNALS__ = {
        invoke: async (command: string, args: any) => {
          w.calls.push({ command, args });
          switch (command) {
            case "java_inspect":
              // the Maven copy of a folder without a pom is a Maven project
              if (options.none && !String(args.path).endsWith("-maven")) return null;
              return { ...base, ...options.project };
            case "java_convert_plan": {
              if (typeof options.convert === "string") throw options.convert;
              const given = args.options ?? {};
              const plan = { ...convertBase, ...(options.convert ?? {}) } as any;
              return {
                ...plan,
                groupId: given.groupId ?? plan.groupId,
                artifactId: given.artifactId ?? plan.artifactId,
                version: given.version ?? plan.version,
                javaLevel: given.javaLevel ?? plan.javaLevel,
                destination: given.into ? `${given.into}/legacy-shop-maven` : plan.destination,
              };
            }
            case "java_convert_run":
              return {
                schemaVersion: 1,
                succeeded: true,
                destination: "/Users/maria/legacy-shop-maven",
                filesWritten: 5,
                counts: convertBase.counts,
                packaging: "jar",
                nextSteps: ["Open the copy's pom.xml and check the coordinates, the Java level and the dependencies."],
              };
            case "java_upgrade_plan":
              // like the Rust side: the copy goes into the chosen folder, else next to the project
              return args.destination
                ? { ...plan, destination: `${args.destination}/legacy-shop-java25` }
                : plan;
            case "java_upgrade_run":
              if (options.partial) {
                return {
                  schemaVersion: 1, succeeded: false, destination: plan.destination, fromJava: 7, toJava: 25,
                  fromSpringBoot: "1.5.22.RELEASE", toSpringBoot: "4.0.x",
                  hops: plan.hops.map((h: any, i: number) => ({
                    id: h.id, label: h.label, status: i < 2 ? "ok" : i === 2 ? "failed" : "skipped",
                    compiled: i === 1 ? true : i === 2 ? false : null,
                  })),
                  failedHop: "spring-boot-3.5", rolledBack: true, reached: "java-17",
                  fixups: [], changes: { modified: ["pom.xml"], added: [], removed: [], truncated: false, total: 1 },
                  tests: { run: 4, failures: 0, errors: 0, skipped: 1, passed: true }, testsNeedDocker: true,
                  output: "CHECK spring-boot-3.5 failed\n[ERROR] 'dependencies.dependency.version' for org.ehcache:ehcache:jar:jakarta is missing",
                  detail: "Step spring-boot-3.5 failed and was undone: the copy was left at the last step that worked (java-17). The original project was not modified.",
                  durationMs: 600000, nextSteps: ["The copy is ready up to step java-17: you can use it as a starting point and do the rest by hand."],
                };
              }
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
            case "java_map":
              if (!options.map) throw "cannot map";
              return options.map;
            case "plugin:dialog|open":
              return /copy will be created|Maven copy/.test(String(args?.options?.title ?? ""))
                ? "/Volumes/backup"
                : "/Users/maria/legacy-shop";
            default:
              throw new Error(`Unexpected IPC ${command}`);
          }
        },
      };
    },
    { plan, options, base: project(), convertBase: convertPlan() },
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

const messyMap = (over: Record<string, unknown> = {}) => ({
  schemaVersion: 1,
  path: "/Users/maria/legacy-shop",
  name: "legacy-shop",
  layout: "custom",
  buildSystems: [
    { kind: "ant", files: ["build.xml"] },
    { kind: "eclipse", files: [".classpath", ".project"] },
  ],
  sourceRoots: [
    { dir: "src", kind: "main", files: 120, packages: 14, topPackage: "com.acme.shop" },
    { dir: "test", kind: "test", files: 31, packages: 6, topPackage: "com.acme.shop" },
  ],
  scatteredFiles: ["src/old/Place.java"],
  scatteredCount: 1,
  mainClasses: [{ file: "src/com/acme/shop/App.java", class: "com.acme.shop.App" }],
  libraries: [
    { path: "lib/commons-lang-2.6.jar", size: 284220, sha256: "ab", artifact: "commons-lang", version: "2.6" },
    { path: "lib/activation.jar", size: 55000, sha256: "cd", artifact: "activation", version: null },
  ],
  libraryCount: 2,
  webApps: ["WebContent"],
  javaLevel: { level: 6, declared: "1.6", source: ".classpath" },
  counts: { java: 151, jars: 2, xml: 9, properties: 4, other: 30, folders: 22, bytes: 1_000_000 },
  nonUtf8Files: 0,
  truncated: false,
  notes: ["The folder has no pom.xml: the upgrade to Java 25 and Spring Boot 4 needs a Maven project."],
  ...over,
});

test("a folder with Java code but no pom.xml shows a map of what is in it", async ({ page }) => {
  await open(page, { none: true, map: messyMap() });
  await choose(page);
  // it is not an error: the folder is read, and nothing offers an upgrade it cannot do
  await expect(page.getByRole("alert")).toHaveCount(0);
  const map = page.getByTestId("java-map");
  await expect(map).toBeVisible();
  await expect(page.getByRole("heading", { name: "Folder map" })).toBeVisible();
  await expect(map).toContainText("Sources in custom folders");
  await expect(map).toContainText("Java 1.6");
  await expect(map).toContainText("151 .java · 2 .jar");
  await expect(map).toContainText("The Java level comes from .classpath.");
  await expect(map.getByText("Ant", { exact: true })).toBeVisible();
  await expect(map.getByText("Eclipse", { exact: true })).toBeVisible();
  const rows = map.locator(".java-map-table tbody tr");
  await expect(rows).toHaveCount(2);
  await expect(rows.nth(0)).toContainText("src");
  await expect(rows.nth(0)).toContainText("code");
  await expect(rows.nth(0)).toContainText("120");
  await expect(rows.nth(0)).toContainText("com.acme.shop");
  await expect(rows.nth(1)).toContainText("tests");
  await expect(map).toContainText("com.acme.shop.App");
  await expect(map).toContainText("WebContent");
  await expect(map).toContainText("The folder has no pom.xml");
  await map.getByText("Libraries in the folder (2)").click();
  await expect(map).toContainText("lib/commons-lang-2.6.jar");
  await expect(map).toContainText("commons-lang 2.6");
  await map.getByText("Files in a folder that does not match their package (1)").click();
  await expect(map).toContainText("src/old/Place.java");
  expect(await calls(page, "java_map")).toEqual([{ path: "/Users/maria/legacy-shop", lang: "en" }]);
  await expect(page.getByRole("button", { name: "Plan the upgrade" })).toHaveCount(0);
});

test("a folder without a pom.xml is planned as a Maven copy and made only after a confirmation", async ({ page }) => {
  await open(page, { none: true, map: messyMap() });
  await choose(page);
  const panel = page.getByTestId("java-convert");
  await expect(panel).toBeVisible();
  // nothing is planned or written until the person asks
  expect(await calls(page, "java_convert_plan")).toEqual([]);
  await panel.getByRole("button", { name: "Plan the Maven copy" }).click();
  expect(await calls(page, "java_convert_plan")).toEqual([
    { path: "/Users/maria/legacy-shop", options: { into: null, groupId: null, artifactId: null, version: null, javaLevel: null }, lang: "en" },
  ]);
  await expect(panel).toContainText("Packaging: jar");
  await expect(panel).toContainText("Java 6");
  await expect(panel.getByLabel("groupId")).toHaveValue("com.acme");
  await expect(panel.getByLabel("artifactId")).toHaveValue("legacy-shop");
  await expect(panel.getByLabel("Java level")).toHaveValue("6");
  await expect(panel).toContainText("120 source(s) in src/main/java and 31 test(s) in src/test/java");
  await expect(panel).toContainText("15 resource(s)");
  await expect(panel).toContainText("2 source(s) were in a folder that does not match their package");
  await expect(panel).toContainText("9 file(s) do not go into the copy");
  await expect(panel).toContainText("Dependencies (3)");
  await expect(panel).toContainText("commons-lang:commons-lang:2.6 · compile");
  await expect(panel).toContainText("junit:junit:4.12 · tests");
  await expect(panel).toContainText("local:acme-legacy:1.0 · local jar · no known coordinates: the jar stays in lib/");
  await expect(panel).toContainText("only the first was kept");
  await expect(panel).toContainText("/Users/maria/legacy-shop-maven");
  await panel.getByText("The pom.xml that will be written").click();
  await expect(panel).toContainText("<groupId>com.acme</groupId>");
  await panel.getByText("Where each file goes (1)").click();
  await expect(panel).toContainText("src/main/java/com/acme/shop/App.java");

  // the copy needs a confirmation
  const start = panel.getByRole("button", { name: "Create the Maven copy" });
  await expect(start).toBeDisabled();
  expect(await calls(page, "java_convert_run")).toEqual([]);
  await panel.getByLabel(/I understand AutoDock will copy the project files/).check();
  await start.click();
  await expect(panel).toContainText("Maven copy created (5 files)");
  expect(await calls(page, "java_convert_run")).toEqual([
    {
      request: {
        path: "/Users/maria/legacy-shop",
        options: { into: null, groupId: "com.acme", artifactId: "legacy-shop", version: "1.0.0-SNAPSHOT", javaLevel: 6 },
        confirmed: true,
      },
      lang: "en",
    },
  ]);
  // the copy is a Maven project now: the page goes on to the upgrade with it
  await panel.getByRole("button", { name: "Continue: plan the upgrade of the copy" }).click();
  expect(await calls(page, "java_inspect")).toEqual([
    { path: "/Users/maria/legacy-shop", lang: "en" },
    { path: "/Users/maria/legacy-shop-maven", lang: "en" },
  ]);
  await expect(page.locator(".expo-projects")).toContainText("legacy-shop");
  await expect(page.getByRole("button", { name: "Plan the upgrade" })).toBeVisible();
  await expect(page.getByTestId("java-convert")).toHaveCount(0);
});

test("the coordinates and the folder of the Maven copy can be changed before it is made", async ({ page }) => {
  await open(page, { none: true, map: messyMap() });
  await choose(page);
  const panel = page.getByTestId("java-convert");
  await panel.getByRole("button", { name: "Plan the Maven copy" }).click();
  await panel.getByLabel("artifactId").fill("shop");
  await panel.getByLabel("version").fill("2.0");
  await panel.getByLabel("Java level").fill("11");
  await panel.getByRole("button", { name: "Update the plan" }).click();
  await expect.poll(async () => (await calls(page, "java_convert_plan")).length).toBe(2);
  expect((await calls(page, "java_convert_plan"))[1].options).toEqual({
    into: null, groupId: "com.acme", artifactId: "shop", version: "2.0", javaLevel: 11,
  });
  // the typed values stay in the fields after the plan answers
  await expect(panel.getByLabel("artifactId")).toHaveValue("shop");

  await panel.getByRole("button", { name: "Choose another folder" }).click();
  await expect(panel).toContainText("/Volumes/backup/legacy-shop-maven");
  expect((await calls(page, "java_convert_plan"))[2].options).toMatchObject({ into: "/Volumes/backup", artifactId: "shop" });
  await panel.getByRole("button", { name: "Back to next to the project" }).click();
  await expect(panel).toContainText("/Users/maria/legacy-shop-maven");
  expect((await calls(page, "java_convert_plan"))[3].options.into).toBeNull();
});

test("a plan the backend refuses is shown and nothing is made", async ({ page }) => {
  await open(page, { none: true, map: messyMap(), convert: "This folder already has a pom.xml: use the upgrade." });
  await choose(page);
  const panel = page.getByTestId("java-convert");
  await panel.getByRole("button", { name: "Plan the Maven copy" }).click();
  await expect(panel.getByRole("alert")).toContainText("This folder already has a pom.xml");
  await expect(panel.getByRole("button", { name: "Create the Maven copy" })).toHaveCount(0);
  expect(await calls(page, "java_convert_run")).toEqual([]);
});

test("a Maven project is not mapped, and a Gradle one shows its notice and the map", async ({ page }) => {
  await open(page);
  await choose(page);
  await expect(page.locator(".expo-projects")).toContainText("legacy-shop");
  expect(await calls(page, "java_map")).toEqual([]);
  await expect(page.getByTestId("java-map")).toHaveCount(0);

  await page.reload();
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
    map: messyMap({ layout: "standard", buildSystems: [{ kind: "gradle", files: ["build.gradle"] }], notes: [] }),
  });
  await choose(page);
  await expect(page.locator(".expo-projects")).toContainText("Gradle projects are not supported yet");
  await expect(page.getByTestId("java-map")).toContainText("Maven-style folders, no pom.xml");
  await expect(page.getByTestId("java-map")).toContainText("Gradle");
});

test("a folder with no Java code at all is still reported as not a Java project", async ({ page }) => {
  await open(page, { none: true, map: messyMap({ layout: "none", counts: { java: 0, jars: 0, xml: 0, properties: 0, other: 3, folders: 1, bytes: 10 } }) });
  await choose(page);
  await expect(page.getByRole("alert")).toContainText("Choose a folder to see the project");
  await expect(page.getByTestId("java-map")).toHaveCount(0);
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

test("a step that fails is undone and the report says the copy is at the last step that worked", async ({ page }) => {
  await open(page, { partial: true });
  await choose(page);
  await page.getByRole("button", { name: "Plan the upgrade" }).click();
  await page.getByLabel(/I understand AutoDock will copy the project/).check();
  await page.locator(".multi-run-actions").getByRole("button", { name: "Create the upgraded copy" }).click();
  await expect(page.getByText("Partial upgrade: a step failed and was undone")).toBeVisible();
  await expect(page.getByText(/was left at the last step that worked \(java-17\)/)).toBeVisible();
  // the tests of the state that worked and what differs from the original are shown
  await expect(page.getByText("4 tests, all passed (1 skipped)")).toBeVisible();
  await expect(page.getByText(/Some tests need Docker \(Testcontainers\)/)).toBeVisible();
  await expect(page.getByText("Files that differ from the original (1)")).toBeVisible();
  await expect(page.getByText("The copy is ready up to step java-17")).toBeVisible();
  await expect(page.getByText("ehcache:jar:jakarta is missing")).toBeVisible();
  await expect(page.locator(".java-hops li").nth(2)).toContainText("failed");
  await expect(page.locator(".java-hops li").nth(1)).toContainText("done");
});

test("a Spring Boot pinned by a third-party BOM is shown on the project", async ({ page }) => {
  await open(page, {
    project: { springBoot: "2.7.3", springBootPinnedBy: "tech.jhipster:jhipster-dependencies", javaDeclared: "11", javaLevel: 11 },
  });
  await choose(page);
  await expect(page.locator(".expo-projects")).toContainText("Spring Boot 2.7.3");
  await expect(page.locator(".expo-projects")).toContainText("Spring Boot pinned by tech.jhipster:jhipster-dependencies");
});
