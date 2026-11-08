import { test, expect, type Page } from "@playwright/test";

test.use({ locale: "en-US" });

const ORIGINAL = "/Users/maria/legacy-shop";
const COPY = "/Users/maria/legacy-shop-java25";

const project = {
  schemaVersion: 1, path: ORIGINAL, name: "legacy-shop", buildTool: "maven", packaging: "jar",
  javaLevel: 7, javaDeclared: "1.7", springBoot: null, springBootPinnedBy: null, modules: [], jdkApis: [],
  upgradeAvailable: true, upgradeSupported: true, unsupportedReason: null,
};

const plan = {
  schemaVersion: 1, project, fromJava: 7, toJava: 25, fromSpringBoot: null, toSpringBoot: null,
  image: "maven:3.9-eclipse-temurin-25",
  hops: [{ id: "java-17", label: "Java 17", recipe: "r", checked: true }, { id: "java-25", label: "Java 25", recipe: "r", checked: true }],
  fixups: [], destination: COPY, command: "docker run …", notes: [], warnings: [], links: [],
};

const report = {
  schemaVersion: 1, succeeded: true, destination: COPY, fromJava: 7, toJava: 25, fromSpringBoot: null, toSpringBoot: null,
  hops: plan.hops.map((h) => ({ id: h.id, label: h.label, status: "ok", compiled: true })),
  failedHop: null, rolledBack: false, reached: "java-25", fixups: [],
  changes: { modified: ["pom.xml", "src/Main.java"], added: ["src/New.java"], removed: ["README.md"], truncated: false, total: 4 },
  tests: { run: 2, failures: 0, errors: 0, skipped: 0, passed: true }, testsNeedDocker: false,
  output: "", detail: "", durationMs: 5000, nextSteps: [],
};

async function open(page: Page) {
  await page.addInitScript(
    ({ project, plan, report, ORIGINAL, COPY }) => {
      const w = window as any;
      w.calls = [];
      let clock = 5000;
      const folder = (files: Record<string, string>) => {
        const root: any = { dirs: {}, files: {} };
        for (const [path, text] of Object.entries(files)) {
          const parts = path.split("/");
          const name = parts.pop()!;
          let node = root;
          for (const part of parts) node = node.dirs[part] ??= { dirs: {}, files: {} };
          node.files[name] = { text, mtime: 1000 };
        }
        return root;
      };
      const roots: Record<string, any> = {
        [ORIGINAL]: folder({
          "pom.xml": "<java>1.7</java>\n",
          "src/Main.java": "import javax.servlet.Servlet;\n",
          "README.md": "# Shop\n",
          "notes.txt": "same\n",
        }),
        [COPY]: folder({
          "pom.xml": "<java>25</java>\n",
          "src/Main.java": "import jakarta.servlet.Servlet;\n",
          "src/New.java": "class New {}\n",
          "notes.txt": "same\n",
        }),
      };
      w.roots = roots;
      const dirAt = (root: string, path: string) => {
        let node = roots[root];
        for (const part of path.split("/").filter(Boolean)) node = node.dirs[part];
        return node;
      };
      const fileAt = (root: string, path: string) => {
        const parts = path.split("/");
        const name = parts.pop()!;
        const dir = dirAt(root, parts.join("/"));
        if (!dir.files[name]) throw `${path}: not found`;
        return { dir, name, file: dir.files[name] };
      };
      w.__TAURI_INTERNALS__ = {
        invoke: async (command: string, args: any) => {
          w.calls.push({ command, args });
          switch (command) {
            case "java_inspect":
              return project;
            case "java_upgrade_plan":
              return plan;
            case "java_upgrade_run":
              return report;
            case "plugin:dialog|open":
              return ORIGINAL;
            case "editor_open_root":
              return { path: args.path, name: args.path.split("/").pop() };
            case "editor_list": {
              const node = dirAt(args.root, args.path);
              const entries = [
                ...Object.keys(node.dirs).map((name) => ({ name, kind: "dir", size: 0 })),
                ...Object.keys(node.files).map((name) => ({ name, kind: "file", size: node.files[name].text.length })),
              ];
              entries.sort((a: any, b: any) => Number(a.kind !== "dir") - Number(b.kind !== "dir") || a.name.localeCompare(b.name));
              return { entries, truncated: false };
            }
            case "editor_read": {
              const { file } = fileAt(args.root, args.path);
              return { path: args.path, text: file.text, size: file.text.length, modifiedMs: file.mtime, bom: false, sensitive: false };
            }
            case "editor_write": {
              const { file } = fileAt(args.root, args.path);
              file.text = args.text;
              file.mtime = ++clock;
              return { modifiedMs: clock, size: args.text.length };
            }
            default:
              throw new Error(`Unexpected IPC ${command}`);
          }
        },
      };
    },
    { project, plan, report, ORIGINAL, COPY },
  );
  await page.goto("/");
}

const calls = (page: Page, command: string) =>
  page.evaluate((name) => (window as any).calls.filter((c: any) => c.command === name).map((c: any) => c.args), command);

const originalSide = (page: Page) => page.locator(".monaco-diff-editor .editor.original .view-lines");
const modifiedSide = (page: Page) => page.locator(".monaco-diff-editor .editor.modified .view-lines");

/** Runs the upgrade on the Java page and clicks the review button. */
async function reviewUpgrade(page: Page) {
  await open(page);
  await page.getByRole("navigation").getByRole("button", { name: "Java", exact: true }).click();
  await page.getByRole("button", { name: "Choose folder" }).click();
  await page.getByRole("button", { name: "Plan the upgrade" }).click();
  await page.getByLabel(/I understand AutoDock will copy the project/).check();
  await page.locator(".multi-run-actions").getByRole("button", { name: "Create the upgraded copy" }).click();
  await page.getByRole("button", { name: "Review the changes in the editor" }).click();
}

test("the upgrade report opens the copy in the editor with the list of changes", async ({ page }) => {
  await reviewUpgrade(page);
  await expect(page.getByRole("heading", { name: "Code editor" })).toBeVisible();
  // the copy is the open project, and nothing from the original is opened as the project
  await expect.poll(() => calls(page, "editor_open_root")).toEqual([{ path: COPY, lang: "en" }]);
  await expect(page.getByText(/Reviewing the upgrade of legacy-shop/)).toBeVisible();
  await expect(page.getByRole("tab", { name: "Changes (4)" })).toHaveAttribute("aria-selected", "true");
  const list = page.locator(".editor-changes");
  await expect(list).toContainText("Changed (2)");
  await expect(list).toContainText("New (1)");
  await expect(list).toContainText("Removed (1)");
  for (const file of ["pom.xml", "src/Main.java", "src/New.java", "README.md"]) {
    await expect(list.getByRole("button", { name: file })).toBeVisible();
  }
  // the normal file tree is one click away
  await page.getByRole("tab", { name: "Files" }).click();
  await expect(page.getByRole("treeitem", { name: "notes.txt" })).toBeVisible();
  await page.getByRole("tab", { name: "Changes (4)" }).click();
  await expect(list).toBeVisible();
});

test("a changed file opens side by side and the copy can be edited and saved", async ({ page }) => {
  await reviewUpgrade(page);
  await page.locator(".editor-changes").getByRole("button", { name: "pom.xml" }).click();
  await expect(page.getByTestId("diff-editor")).toBeVisible();
  await expect(originalSide(page)).toContainText("1.7");
  await expect(modifiedSide(page)).toContainText("25");
  await expect(page.locator(".editor-diff-head")).toContainText("Original");
  await expect(page.locator(".editor-diff-head")).toContainText("Upgraded copy (editable)");
  // both files were read, each from its own folder
  expect(await calls(page, "editor_read")).toEqual([
    { root: ORIGINAL, path: "pom.xml", lang: "en" },
    { root: COPY, path: "pom.xml", lang: "en" },
  ]);
  await expect(page.getByRole("button", { name: "Save", exact: true })).toBeDisabled();

  // typing goes into the copy, never the original
  await modifiedSide(page).click();
  await page.keyboard.press("Control+End");
  await page.keyboard.type("<!-- reviewed -->");
  await expect(page.getByRole("img", { name: "unsaved changes" })).toBeVisible();
  await page.keyboard.press("Control+S");
  await expect(page.getByText("pom.xml saved.")).toBeVisible();
  const writes = await calls(page, "editor_write");
  expect(writes).toHaveLength(1);
  expect(writes[0]).toMatchObject({ root: COPY, path: "pom.xml", expectedMs: 1000, overwrite: false });
  expect(writes[0].text).toContain("<java>25</java>");
  expect(writes[0].text).toContain("<!-- reviewed -->");
  await expect(originalSide(page)).not.toContainText("reviewed");
  const original = await page.evaluate(() => (window as any).roots["/Users/maria/legacy-shop"].files["pom.xml"].text);
  expect(original).toBe("<java>1.7</java>\n");
});

test("added files open as a normal file and removed ones as a read-only comparison", async ({ page }) => {
  await reviewUpgrade(page);
  await page.locator(".editor-changes").getByRole("button", { name: "src/New.java" }).click();
  await expect(page.getByRole("tab", { name: "New.java" })).toBeVisible();
  await expect(page.locator(".monaco-editor .view-lines").first()).toContainText("class New");
  // only the copy was read for a file that did not exist before
  expect((await calls(page, "editor_read")).map((a: any) => a.root)).toEqual([COPY]);

  await page.locator(".editor-changes").getByRole("button", { name: "README.md" }).click();
  await expect(page.locator(".editor-diff-head")).toContainText("Removed in the copy (read-only)");
  await expect(originalSide(page)).toContainText("# Shop");
  const reads = await calls(page, "editor_read");
  expect(reads[reads.length - 1]).toEqual({ root: ORIGINAL, path: "README.md", lang: "en" });
  // nothing can be typed on the right side
  await modifiedSide(page).click({ force: true });
  await page.keyboard.type("nope");
  await expect(page.getByRole("img", { name: "unsaved changes" })).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Save", exact: true })).toBeDisabled();
});

test("a comparison is shown again instead of opened twice, and closing it asks about unsaved changes", async ({ page }) => {
  await reviewUpgrade(page);
  await page.locator(".editor-changes").getByRole("button", { name: "src/Main.java" }).click();
  await expect(modifiedSide(page)).toContainText("jakarta");
  await page.locator(".editor-changes").getByRole("button", { name: "pom.xml" }).click();
  await expect(modifiedSide(page)).toContainText("25");
  await page.locator(".editor-changes").getByRole("button", { name: "src/Main.java" }).click();
  await expect(modifiedSide(page)).toContainText("jakarta");
  expect((await calls(page, "editor_read")).length).toBe(4); // two files, two sides each, once

  await modifiedSide(page).click();
  await page.keyboard.press("Control+End");
  await page.keyboard.type("// edit");
  await page.getByRole("button", { name: "Close Main.java" }).click();
  await expect(page.getByText("Save the changes to Main.java?")).toBeVisible();
  await page.getByRole("button", { name: "Discard" }).click();
  await expect(page.getByRole("tab", { name: /Main.java/ })).toHaveCount(0);
  expect(await calls(page, "editor_write")).toEqual([]);
});

test("closing the folder with unsaved changes asks first", async ({ page }) => {
  await reviewUpgrade(page);
  await page.locator(".editor-changes").getByRole("button", { name: "pom.xml" }).click();
  await modifiedSide(page).click();
  await page.keyboard.type("x");
  await page.getByRole("button", { name: "Close folder" }).click();
  await expect(page.getByText("You have unsaved changes")).toBeVisible();
  await page.getByRole("button", { name: "Cancel" }).click();
  await expect(page.getByText("You have unsaved changes")).toHaveCount(0);
  await expect(page.getByRole("tab", { name: /pom.xml/ })).toBeVisible();

  await page.getByRole("button", { name: "Close folder" }).click();
  await page.getByRole("button", { name: "Discard and continue" }).click();
  await expect(page.getByRole("heading", { name: "Open a project" })).toBeVisible();
  expect(await calls(page, "editor_write")).toEqual([]);
});

test("stopping the review keeps the files and hides the changes view", async ({ page }) => {
  await reviewUpgrade(page);
  await page.getByRole("button", { name: "Stop reviewing" }).click();
  await expect(page.getByText(/Reviewing the upgrade of/)).toHaveCount(0);
  await expect(page.getByRole("tab", { name: /Changes/ })).toHaveCount(0);
  await expect(page.getByRole("treeitem", { name: "notes.txt" })).toBeVisible();
});

test.describe("in Portuguese", () => {
  test.use({ locale: "pt-BR" });

  test("the review is written in Portuguese", async ({ page }) => {
    await open(page);
    await page.getByRole("navigation").getByRole("button", { name: "Java", exact: true }).click();
    await page.getByRole("button", { name: "Escolher pasta" }).click();
    await page.getByRole("button", { name: "Planejar o upgrade" }).click();
    await page.getByLabel(/Entendo que o AutoDock vai copiar o projeto/).check();
    await page.locator(".multi-run-actions").getByRole("button", { name: "Criar a cópia atualizada" }).click();
    await page.getByRole("button", { name: "Revisar as mudanças no editor" }).click();
    await expect(page.getByRole("tab", { name: "Mudanças (4)" })).toBeVisible();
    await expect(page.getByText(/Revisando o upgrade de legacy-shop/)).toBeVisible();
    await page.locator(".editor-changes").getByRole("button", { name: "pom.xml" }).click();
    await expect(page.locator(".editor-diff-head")).toContainText("Cópia atualizada (editável)");
  });
});
