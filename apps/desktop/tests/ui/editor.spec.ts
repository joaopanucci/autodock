import { test, expect, type Page } from "@playwright/test";

test.use({ locale: "en-US" });

type Options = { conflict?: boolean };

async function open(page: Page, options: Options = {}) {
  await page.addInitScript((options) => {
    const w = window as any;
    w.calls = [];
    let clock = 1000;
    // A tiny project in memory.
    const tree: any = {
      dirs: {
        src: {
          dirs: { main: { dirs: { java: { dirs: {}, files: { "App.java": { text: "public class App {\n}\n", mtime: clock } } } }, files: {} } },
          files: { "notes.txt": { text: "remember\n", mtime: clock } },
        },
        docs: { dirs: {}, files: {} },
      },
      files: {
        "pom.xml": { text: "<project>\n  <artifactId>shop</artifactId>\n</project>\n", mtime: clock },
        "README.md": { text: "# Shop\n", mtime: clock },
        ".env": { text: "SECRET=1\n", mtime: clock },
        "logo.png": { binary: true, text: "", mtime: clock },
      },
      links: ["shortcut"],
    };
    w.tree = tree;
    const dirAt = (path: string) => {
      let node = tree;
      for (const part of path.split("/").filter(Boolean)) node = node.dirs[part];
      return node;
    };
    const fileAt = (path: string) => {
      const parts = path.split("/");
      const name = parts.pop()!;
      return { dir: dirAt(parts.join("/")), name };
    };
    w.__TAURI_INTERNALS__ = {
      invoke: async (command: string, args: any) => {
        w.calls.push({ command, args });
        switch (command) {
          case "editor_open_root":
            return { path: "/Users/maria/shop", name: "shop" };
          case "editor_list": {
            const node = dirAt(args.path);
            const entries = [
              ...Object.keys(node.dirs ?? {}).map((name) => ({ name, kind: "dir", size: 0 })),
              ...Object.keys(node.files ?? {}).map((name) => ({ name, kind: "file", size: node.files[name].text.length })),
              ...(args.path === "" ? tree.links.map((name: string) => ({ name, kind: "link", size: 0 })) : []),
            ];
            // like the Rust side: folders first, then by name without regard to case
            entries.sort((a: any, b: any) => Number(a.kind !== "dir") - Number(b.kind !== "dir") || a.name.toLowerCase().localeCompare(b.name.toLowerCase()));
            return { entries, truncated: false };
          }
          case "editor_read": {
            const { dir, name } = fileAt(args.path);
            const file = dir.files[name];
            if (file.binary) throw `${args.path} looks like a binary file, which the editor does not open.`;
            return { path: args.path, text: file.text, size: file.text.length, modifiedMs: file.mtime, bom: false, sensitive: name === ".env" };
          }
          case "editor_write": {
            const { dir, name } = fileAt(args.path);
            if (options.conflict && !args.overwrite) {
              options.conflict = false;
              throw `conflict: ${args.path} changed on disk after it was opened.`;
            }
            dir.files[name].text = args.text;
            dir.files[name].mtime = ++clock;
            return { modifiedMs: clock, size: args.text.length };
          }
          case "editor_create": {
            const parent = dirAt(args.parent);
            if (args.kind === "dir") parent.dirs[args.name] = { dirs: {}, files: {} };
            else parent.files[args.name] = { text: "", mtime: ++clock };
            return { name: args.name, kind: args.kind, size: 0 };
          }
          case "plugin:dialog|open":
            return "/Users/maria/shop";
          default:
            throw new Error(`Unexpected IPC ${command}`);
        }
      },
    };
  }, options);
  await page.goto("/");
  await page.getByRole("navigation").getByRole("button", { name: "Editor", exact: true }).click();
}

const calls = (page: Page, command: string) =>
  page.evaluate((name) => (window as any).calls.filter((c: any) => c.command === name).map((c: any) => c.args), command);

const lines = (page: Page) => page.locator(".monaco-editor .view-lines");

async function openFolder(page: Page) {
  await page.getByRole("button", { name: "Open folder" }).click();
  await expect(page.getByRole("treeitem", { name: "pom.xml" })).toBeVisible();
}

async function openFile(page: Page, name: string) {
  await page.getByRole("treeitem", { name, exact: true }).click();
  await expect(page.getByRole("tab", { name })).toBeVisible();
}

test("opens a folder, lists it folders first and opens a file in a tab with its colours", async ({ page }) => {
  await open(page);
  await expect(page.getByRole("heading", { name: "Code editor" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Open a project" })).toBeVisible();
  // nothing is read until a folder is chosen
  expect(await calls(page, "editor_list")).toEqual([]);

  await openFolder(page);
  expect(await calls(page, "editor_open_root")).toEqual([{ path: "/Users/maria/shop", lang: "en" }]);
  const names = await page.getByRole("tree").first().getByRole("treeitem").allTextContents();
  expect(names.map((n) => n.trim())).toEqual(["docs", "src", ".env", "logo.png", "pom.xml", "README.md"]);

  // a folder opens on click and loads its content only then
  await page.getByRole("treeitem", { name: "Open folder src" }).click();
  await expect(page.getByRole("treeitem", { name: "notes.txt" })).toBeVisible();
  expect((await calls(page, "editor_list")).map((a: any) => a.path)).toEqual(["", "src"]);
  await page.getByRole("treeitem", { name: "Open folder main" }).click();
  await page.getByRole("treeitem", { name: "Open folder java" }).click();
  await openFile(page, "App.java");
  await expect(lines(page)).toContainText("public class App");
  await expect(page.locator(".editor-status")).toContainText("java");
  await expect(page.locator(".monaco-editor .mtk6, .monaco-editor [class*='mtk']").first()).toBeVisible();
  expect(await calls(page, "editor_read")).toEqual([{ root: "/Users/maria/shop", path: "src/main/java/App.java", lang: "en" }]);

  // opening it again only shows its tab
  await page.getByRole("treeitem", { name: "App.java" }).click();
  expect(await calls(page, "editor_read")).toHaveLength(1);
  // the link is listed but cannot be opened
  await expect(page.locator(".editor-row.is-link")).toContainText("shortcut");
});

test("a folder can be opened by typing or pasting its path", async ({ page }) => {
  await open(page);
  const open_ = page.getByRole("button", { name: "Open this path" });
  await expect(open_).toBeDisabled();
  await page.getByLabel("Or type or paste the folder path").fill("  /Users/maria/shop  ");
  await open_.click();
  await expect(page.getByRole("treeitem", { name: "pom.xml" })).toBeVisible();
  // the path goes as typed, without the spaces around it
  expect(await calls(page, "editor_open_root")).toEqual([{ path: "/Users/maria/shop", lang: "en" }]);
});

test("typing marks the tab as unsaved and Ctrl+S saves it with the time it was read", async ({ page }) => {
  await open(page);
  await openFolder(page);
  await openFile(page, "README.md");
  await expect(lines(page)).toContainText("# Shop");
  await expect(page.getByRole("button", { name: "Save", exact: true })).toBeDisabled();

  await lines(page).click();
  await page.keyboard.press("Control+End");
  await page.keyboard.type("more text");
  await expect(page.getByRole("img", { name: "unsaved changes" })).toBeVisible();
  await expect(page.locator(".editor-status")).toContainText("Unsaved changes");
  expect(await calls(page, "editor_write")).toEqual([]);

  await page.keyboard.press("Control+S");
  await expect(page.getByText("README.md saved.")).toBeVisible();
  await expect(page.getByRole("img", { name: "unsaved changes" })).toHaveCount(0);
  const writes = await calls(page, "editor_write");
  expect(writes).toHaveLength(1);
  expect(writes[0]).toMatchObject({ root: "/Users/maria/shop", path: "README.md", bom: false, expectedMs: 1000, overwrite: false });
  expect(writes[0].text).toContain("# Shop");
  expect(writes[0].text).toContain("more text");
  // the next save compares with the time of this one
  await page.keyboard.type("!");
  await page.getByRole("button", { name: "Save", exact: true }).click();
  await expect(async () => expect(await calls(page, "editor_write")).toHaveLength(2)).toPass();
  expect((await calls(page, "editor_write"))[1].expectedMs).toBe(1001);
});

test("a file that changed on disk is not overwritten until the person says so", async ({ page }) => {
  await open(page, { conflict: true });
  await openFolder(page);
  await openFile(page, "pom.xml");
  await lines(page).click();
  await page.keyboard.type("x");
  await page.keyboard.press("Control+S");
  await expect(page.getByText("The file changed on disk")).toBeVisible();
  await expect(page.getByRole("img", { name: "unsaved changes" })).toBeVisible();
  expect((await calls(page, "editor_write"))[0].overwrite).toBe(false);

  await page.getByRole("button", { name: "Overwrite" }).click();
  await expect(page.getByText("pom.xml saved.")).toBeVisible();
  const writes = await calls(page, "editor_write");
  expect(writes).toHaveLength(2);
  expect(writes[1].overwrite).toBe(true);
  await expect(page.getByText("The file changed on disk")).toHaveCount(0);
});

test("a file that may hold secrets shows a warning, a binary one shows why it is not opened", async ({ page }) => {
  await open(page);
  await openFolder(page);
  await openFile(page, ".env");
  await expect(page.getByText(/may contain secrets/)).toBeVisible();
  await expect(lines(page)).toContainText("SECRET=1");
  // the warning goes away with another file
  await openFile(page, "README.md");
  await expect(page.getByText(/may contain secrets/)).toHaveCount(0);

  await page.getByRole("treeitem", { name: "logo.png" }).click();
  await expect(page.locator(".feedback.error")).toContainText("looks like a binary file");
  await expect(page.getByRole("tab", { name: "logo.png" })).toHaveCount(0);
});

test("creates a file inside the selected folder and opens it, and never overwrites", async ({ page }) => {
  await open(page);
  await openFolder(page);
  await page.getByRole("treeitem", { name: "Open folder docs" }).click();
  await page.getByRole("button", { name: "New file" }).click();
  await expect(page.getByText(/in docs/)).toBeVisible();
  await page.getByLabel("Name of the new file").fill("guide.md");
  await page.getByRole("button", { name: "Create", exact: true }).click();
  await expect(page.getByRole("tab", { name: "guide.md" })).toBeVisible();
  expect(await calls(page, "editor_create")).toEqual([
    { root: "/Users/maria/shop", parent: "docs", name: "guide.md", kind: "file", lang: "en" },
  ]);
  await expect(page.getByRole("treeitem", { name: "guide.md" })).toBeVisible();

  // a folder at the top, from the new folder button
  await page.getByRole("treeitem", { name: /folder docs/ }).click(); // select docs again (it closes)
  await page.getByRole("button", { name: "New folder" }).click();
  await page.getByLabel("Name of the new folder").fill("images");
  await page.getByRole("button", { name: "Create", exact: true }).click();
  expect((await calls(page, "editor_create"))[1]).toMatchObject({ parent: "docs", name: "images", kind: "dir" });
});

test("closing a tab with unsaved changes asks first and discarding forgets them", async ({ page }) => {
  await open(page);
  await openFolder(page);
  await openFile(page, "README.md");
  await openFile(page, "pom.xml");
  await lines(page).click();
  await page.keyboard.type("zzz");
  await page.getByRole("button", { name: "Close pom.xml" }).click();
  await expect(page.getByText("Save the changes to pom.xml?")).toBeVisible();
  await expect(page.getByRole("tab", { name: /pom.xml/ })).toBeVisible();

  await page.getByRole("button", { name: "Cancel" }).click();
  await expect(page.getByText("Save the changes to pom.xml?")).toHaveCount(0);
  await page.getByRole("button", { name: "Close pom.xml" }).click();
  await page.getByRole("button", { name: "Discard" }).click();
  await expect(page.getByRole("tab", { name: /pom.xml/ })).toHaveCount(0);
  expect(await calls(page, "editor_write")).toEqual([]);
  // the other tab takes over
  await expect(lines(page)).toContainText("# Shop");

  // a file without changes closes at once
  await page.getByRole("button", { name: "Close README.md" }).click();
  await expect(page.getByRole("tab")).toHaveCount(0);
  await expect(page.getByText("No file open")).toBeVisible();
});

test("saving from the close question saves and then closes", async ({ page }) => {
  await open(page);
  await openFolder(page);
  await openFile(page, "README.md");
  await lines(page).click();
  await page.keyboard.type("keep me");
  await page.getByRole("button", { name: "Close README.md" }).click();
  await page.getByRole("button", { name: "Save and close" }).click();
  await expect(page.getByRole("tab", { name: "README.md" })).toHaveCount(0);
  const writes = await calls(page, "editor_write");
  expect(writes).toHaveLength(1);
  expect(writes[0].text).toContain("keep me");
});

test("the open files survive a visit to another page", async ({ page }) => {
  await open(page);
  await openFolder(page);
  await openFile(page, "README.md");
  await lines(page).click();
  await page.keyboard.type("draft");
  await page.getByRole("navigation").getByRole("button", { name: "Java", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Java projects" })).toBeVisible();
  await page.getByRole("navigation").getByRole("button", { name: "Editor", exact: true }).click();
  await expect(page.getByRole("tab", { name: /README.md/ })).toBeVisible();
  await expect(page.getByRole("img", { name: "unsaved changes" })).toBeVisible();
  await expect(lines(page)).toContainText("draft");
  // no file is read again
  expect(await calls(page, "editor_read")).toHaveLength(1);
});

test("follows the app theme", async ({ page }) => {
  await open(page);
  await openFolder(page);
  await openFile(page, "README.md");
  await page.getByRole("group", { name: /theme|Theme/ }).getByRole("button", { name: "Dark theme" }).click();
  await expect(page.locator(".monaco-editor.vs-dark")).toBeVisible();
  await page.getByRole("group", { name: /theme|Theme/ }).getByRole("button", { name: "Light theme" }).click();
  await expect(page.locator(".monaco-editor.vs-dark")).toHaveCount(0);
});

test.describe("in Portuguese", () => {
  test.use({ locale: "pt-BR" });

  test("the editor is written in Portuguese", async ({ page }) => {
    await open(page);
    await expect(page.getByRole("heading", { name: "Editor de código" })).toBeVisible();
    await page.getByRole("button", { name: "Abrir pasta" }).click();
    await expect(page.getByRole("button", { name: "Novo arquivo" })).toBeVisible();
    await expect(page.getByText("Nenhum arquivo aberto")).toBeVisible();
  });
});
