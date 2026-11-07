import { test, expect, type Page } from "@playwright/test";

test.use({ locale: "en-US" });

const FOLDER = "/Users/maria/shop";

type Options = {
  /** The terminal cannot start (Docker is missing, for example). */
  startFails?: string;
  /** `terminal_start` answers only when the test says so. */
  holdStart?: boolean;
};

async function open(page: Page, options: Options = {}) {
  await page.addInitScript(
    ({ options, FOLDER }) => {
      const w = window as any;
      w.calls = [];
      w.callbacks = {};
      w.typed = [];
      w.sessions = 0;
      let nextCallback = 1;
      w.__TAURI_INTERNALS__ = {
        transformCallback: (callback: unknown) => {
          const id = nextCallback++;
          w.callbacks[id] = callback;
          return id;
        },
        unregisterCallback: (id: number) => delete w.callbacks[id],
        invoke: async (command: string, args: any) => {
          const plain = Object.fromEntries(
            Object.entries(args ?? {}).map(([key, value]: [string, any]) => [
              key,
              value && typeof value === "object" && "cleanupCallback" in value ? "[channel]" : value,
            ]),
          );
          w.calls.push({ command, args: plain });
          switch (command) {
            case "editor_open_root":
              return { path: args.path, name: args.path.split("/").pop() };
            case "editor_list":
              return { entries: [{ name: "pom.xml", kind: "file", size: 3 }], truncated: false };
            case "terminal_images":
              return [
                { id: "maven-jdk25", label: "Java (Maven + JDK 25)", image: "maven:3.9-eclipse-temurin-25" },
                { id: "node22", label: "Node.js 22", image: "node:22-bookworm-slim" },
                { id: "debian", label: "Debian (shell only)", image: "debian:bookworm-slim" },
              ];
            case "terminal_default_image":
              return "maven-jdk25";
            case "terminal_start": {
              if (options.startFails) throw options.startFails;
              if (options.holdStart) await new Promise<void>((resolve) => (w.releaseStart = resolve));
              const channel = { callback: args.onEvent.id, index: 0 };
              w.channel = channel;
              w.emit = (message: unknown) => w.callbacks[channel.callback]({ index: channel.index++, message });
              return ++w.sessions;
            }
            case "terminal_write":
              w.typed.push(args.data);
              return null;
            case "terminal_resize":
            case "terminal_stop":
              return null;
            default:
              throw new Error(`Unexpected IPC ${command}`);
          }
        },
      };
      void FOLDER;
    },
    { options, FOLDER },
  );
  await page.goto("/");
  await page.getByRole("navigation").getByRole("button", { name: "Editor", exact: true }).click();
  await page.getByLabel("Or type or paste the folder path").fill(FOLDER);
  await page.getByRole("button", { name: "Open this path" }).click();
  await expect(page.getByRole("treeitem", { name: "pom.xml" })).toBeVisible();
}

const calls = (page: Page, command: string) =>
  page.evaluate((name) => (window as any).calls.filter((c: any) => c.command === name).map((c: any) => c.args), command);
const emit = (page: Page, event: unknown) => page.evaluate((message) => (window as any).emit(message), event);
const print = (page: Page, text: string) => emit(page, { event: "data", data: Buffer.from(text).toString("base64") });
const typed = (page: Page) => page.evaluate(() => ((window as any).typed as string[]).join(""));
const toggle = (page: Page) => page.getByRole("button", { name: "Terminal", exact: true });
const panel = (page: Page) => page.getByRole("region", { name: "Integrated terminal" });
const screen = (page: Page) => panel(page).locator(".xterm-rows");

test("the terminal starts in the image that fits the folder, shows what the shell prints and sends what is typed", async ({ page }) => {
  await open(page);
  // nothing runs until the terminal is asked for
  expect(await calls(page, "terminal_start")).toHaveLength(0);
  await toggle(page).click();
  await expect(panel(page)).toBeVisible();
  await expect(panel(page).getByRole("combobox", { name: "Environment" })).toHaveValue("maven-jdk25");
  await expect.poll(async () => (await calls(page, "terminal_start")).length).toBe(1);

  const [start] = await calls(page, "terminal_start");
  expect(start).toMatchObject({ root: FOLDER, image: "maven-jdk25", onEvent: "[channel]", lang: "en" });
  expect(start.cols).toBeGreaterThan(20);
  expect(start.rows).toBeGreaterThan(3);

  await print(page, "hello from the container\r\n");
  await expect(screen(page)).toContainText("hello from the container");

  await page.locator(".xterm-helper-textarea").focus();
  await page.keyboard.type("ls -la");
  await page.keyboard.press("Enter");
  await expect.poll(() => typed(page)).toBe("ls -la\r");
  const writes = await calls(page, "terminal_write");
  expect(writes.every((w: any) => w.id === 1)).toBe(true);
});

test("what is typed while the container is still starting is kept and sent in order once it is up", async ({ page }) => {
  await open(page, { holdStart: true });
  await toggle(page).click();
  await expect(panel(page)).toContainText("Starting the container");
  await page.locator(".xterm-helper-textarea").focus();
  await page.keyboard.type("echo hi");
  await page.keyboard.press("Enter");
  expect(await typed(page)).toBe("");
  await page.evaluate(() => (window as any).releaseStart());
  await expect.poll(() => typed(page)).toBe("echo hi\r");
  await expect(panel(page)).not.toContainText("Starting the container");
});

test("a big paste is sent in pieces the backend accepts, in order", async ({ page }) => {
  await open(page);
  await toggle(page).click();
  await expect.poll(async () => (await calls(page, "terminal_start")).length).toBe(1);
  const text = Array.from({ length: 20000 }, (_, i) => String.fromCharCode(97 + (i % 26))).join("");
  await page.locator(".xterm-helper-textarea").focus();
  await page.evaluate((text) => {
    const data = new DataTransfer();
    data.setData("text/plain", text);
    document.querySelector(".xterm-helper-textarea")!.dispatchEvent(new ClipboardEvent("paste", { clipboardData: data, bubbles: true, cancelable: true }));
  }, text);
  await expect.poll(() => typed(page)).toBe(text);
  const sizes = (await calls(page, "terminal_write")).map((w: any) => w.data.length);
  expect(sizes.length).toBeGreaterThan(1);
  expect(Math.max(...sizes)).toBeLessThanOrEqual(8000);
});

test("when the shell ends the screen says so with its code, and restart opens a new one", async ({ page }) => {
  await open(page);
  await toggle(page).click();
  await expect.poll(async () => (await calls(page, "terminal_start")).length).toBe(1);
  await emit(page, { event: "exit", code: 3 });
  await expect(screen(page)).toContainText("the shell ended (code 3)");
  await expect(panel(page)).toContainText("The terminal ended.");
  await panel(page).getByRole("button", { name: "Restart the terminal" }).click();
  await expect.poll(async () => (await calls(page, "terminal_start")).length).toBe(2);
  // the old container is released, the new one starts clean
  expect(await calls(page, "terminal_stop")).toEqual([{ id: 1 }]);
  await expect(panel(page)).not.toContainText("The terminal ended.");
  await print(page, "again\r\n");
  await expect(screen(page)).toContainText("again");
});

test("another environment restarts the terminal in that image", async ({ page }) => {
  await open(page);
  await toggle(page).click();
  await expect.poll(async () => (await calls(page, "terminal_start")).length).toBe(1);
  await panel(page).getByRole("combobox", { name: "Environment" }).selectOption("node22");
  await expect.poll(async () => (await calls(page, "terminal_start")).length).toBe(2);
  expect((await calls(page, "terminal_start"))[1]).toMatchObject({ root: FOLDER, image: "node22" });
  expect(await calls(page, "terminal_stop")).toEqual([{ id: 1 }]);
});

test("hiding the terminal keeps the shell running, showing it again does not start another", async ({ page }) => {
  await open(page);
  await toggle(page).click();
  await expect.poll(async () => (await calls(page, "terminal_start")).length).toBe(1);
  await print(page, "still here\r\n");
  await panel(page).getByRole("button", { name: "Hide the terminal" }).click();
  await expect(panel(page)).toBeHidden();
  await expect(toggle(page)).toHaveAttribute("aria-pressed", "false");
  await print(page, "while hidden\r\n");
  await toggle(page).click();
  await expect(panel(page)).toBeVisible();
  await expect(toggle(page)).toHaveAttribute("aria-pressed", "true");
  await expect(screen(page)).toContainText("still here");
  await expect(screen(page)).toContainText("while hidden");
  expect(await calls(page, "terminal_start")).toHaveLength(1);
  expect(await calls(page, "terminal_stop")).toHaveLength(0);
});

test("closing the folder ends the shell and its container", async ({ page }) => {
  await open(page);
  await toggle(page).click();
  await expect.poll(async () => (await calls(page, "terminal_start")).length).toBe(1);
  await page.getByRole("button", { name: "Close folder" }).click();
  await expect.poll(async () => await calls(page, "terminal_stop")).toEqual([{ id: 1 }]);
  await expect(panel(page)).toHaveCount(0);
});

test("a terminal that cannot start says why", async ({ page }) => {
  await open(page, { startFails: "Docker is not installed or not running." });
  await toggle(page).click();
  await expect(panel(page).getByRole("alert")).toContainText("Docker is not installed or not running.");
  expect(await calls(page, "terminal_write")).toHaveLength(0);
});

test("Tab goes to the shell and Ctrl+Shift+E leaves the terminal", async ({ page }) => {
  await open(page);
  await toggle(page).click();
  await expect.poll(async () => (await calls(page, "terminal_start")).length).toBe(1);
  await page.locator(".xterm-helper-textarea").focus();
  await page.keyboard.press("Tab");
  await expect.poll(() => typed(page)).toBe("\t");
  await page.keyboard.press("Control+Shift+E");
  await expect(panel(page).getByRole("combobox", { name: "Environment" })).toBeFocused();
  expect(await typed(page)).toBe("\t");
});

test("the file list can be refreshed after a command changed the folder", async ({ page }) => {
  await open(page);
  const before = (await calls(page, "editor_list")).length;
  await page.getByRole("button", { name: "Refresh the file list" }).click();
  await expect.poll(async () => (await calls(page, "editor_list")).length).toBe(before + 1);
  expect((await calls(page, "editor_list")).at(-1)).toMatchObject({ root: FOLDER, path: "" });
});
