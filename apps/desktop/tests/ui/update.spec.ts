import { test, expect, type Page } from "@playwright/test";

test.use({ locale: "en-US" });

const OFFER = {
  currentVersion: "0.3.1",
  version: "0.4.0",
  notes: "AutoDock 0.4.0 is available. What's new: https://github.com/joaopanucci/autodock/releases/tag/v0.4.0",
  date: 1794200000,
};

type Options = {
  offer?: typeof OFFER | null;
  /** What "Check now" finds. */
  nowOffer?: typeof OFFER | null;
  nowError?: string;
  installFails?: string;
};

async function open(page: Page, options: Options = {}) {
  await page.addInitScript(
    ({ options, offer }) => {
      const w = window as any;
      w.calls = [];
      w.callbacks = {};
      w.prefs = { schemaVersion: 1, checkOnStart: true, lastCheck: null, skippedVersion: null };
      w.installFails = options.installFails ?? "";
      let nextId = 1;
      w.__TAURI_INTERNALS__ = {
        transformCallback: (callback: unknown) => {
          const id = nextId++;
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
            case "update_auto_check":
              return { checked: true, offer: options.offer === undefined ? offer : options.offer, error: null };
            case "update_check_now":
              if (options.nowError) return { checked: true, offer: null, error: options.nowError };
              return { checked: true, offer: options.nowOffer ?? null, error: null };
            case "update_get_prefs":
              return w.prefs;
            case "update_set_check_on_start":
              w.prefs = { ...w.prefs, checkOnStart: args.enabled };
              return w.prefs;
            case "update_skip_version":
              w.prefs = { ...w.prefs, skippedVersion: args.version };
              return w.prefs;
            case "update_install": {
              if (w.installFails) throw w.installFails;
              const send = (index: number, message: unknown) => w.callbacks[args.onEvent.id]({ index, message });
              send(0, { event: "started", contentLength: 1000 });
              send(1, { event: "progress", chunkLength: 400 });
              // the test decides when the download ends, so the progress can be observed
              await new Promise<void>((resolve) => (w.releaseInstall = resolve));
              send(2, { event: "progress", chunkLength: 600 });
              send(3, { event: "finished" });
              return null;
            }
            case "update_restart":
              return null;
            default:
              throw new Error(`Unexpected IPC ${command}`);
          }
        },
      };
    },
    { options, offer: OFFER },
  );
  await page.goto("/");
}

const banner = (page: Page) => page.getByTestId("update-banner");
const calls = (page: Page, command: string) =>
  page.evaluate((name) => (window as any).calls.filter((c: any) => c.command === name).map((c: any) => c.args), command);
const settings = (page: Page) => page.getByRole("navigation").getByRole("button", { name: "Settings", exact: true }).click();

test("offers the new version when the app opens, with the release notes, and installs only after a click", async ({ page }) => {
  await open(page);
  await expect(banner(page)).toContainText("AutoDock 0.4.0 is available (you have 0.3.1).");
  expect(await calls(page, "update_auto_check")).toHaveLength(1);
  expect(await calls(page, "update_install"), "nothing is installed by itself").toEqual([]);

  // the notes are plain text, behind a "What's new"
  await expect(banner(page).locator(".update-notes")).toBeHidden();
  await banner(page).locator("summary").click();
  await expect(banner(page).locator(".update-notes")).toContainText("releases/tag/v0.4.0");

  await banner(page).getByRole("button", { name: "Update now" }).click();
  await expect(banner(page)).toContainText("Downloading AutoDock 0.4.0… 40%");
  await expect(banner(page).getByRole("progressbar")).toHaveAttribute("value", "40");
  await expect(banner(page).getByRole("button", { name: "Update now" })).toHaveCount(0);

  await page.evaluate(() => (window as any).releaseInstall());
  await expect(banner(page)).toContainText("AutoDock 0.4.0 is installed. Restart to finish.");
  expect(await calls(page, "update_restart")).toEqual([]);
  await banner(page).getByRole("button", { name: "Restart now" }).click();
  await expect.poll(() => calls(page, "update_restart")).toHaveLength(1);
});

test("a failed install says why and can be tried again", async ({ page }) => {
  await open(page, { installFails: "the signature does not match" });
  await banner(page).getByRole("button", { name: "Update now" }).click();
  await expect(banner(page)).toContainText("The update could not be installed: the signature does not match");
  await page.evaluate(() => ((window as any).installFails = ""));
  await banner(page).getByRole("button", { name: "Try again" }).click();
  await expect(banner(page)).toContainText("Downloading AutoDock 0.4.0… 40%");
});

test("Later hides the notice for now and Skip remembers the version", async ({ page }) => {
  await open(page);
  await banner(page).getByRole("button", { name: "Later" }).click();
  await expect(banner(page)).toHaveCount(0);
  expect(await calls(page, "update_skip_version")).toEqual([]);

  await page.reload();
  await expect(banner(page)).toBeVisible();
  await banner(page).getByRole("button", { name: "Skip this version" }).click();
  await expect(banner(page)).toHaveCount(0);
  expect(await calls(page, "update_skip_version")).toEqual([{ version: "0.4.0", lang: "en" }]);
});

test("nothing is shown when there is no new version", async ({ page }) => {
  await open(page, { offer: null });
  await expect(page.getByRole("heading", { name: "Your workspace, in focus." })).toBeVisible();
  await expect(banner(page)).toHaveCount(0);
});

test("Settings shows the version, the automatic check is on by default and can be turned off", async ({ page }) => {
  await open(page, { offer: null });
  await settings(page);
  await expect(page.getByText(/Installed version: \d+\.\d+\.\d+/)).toBeVisible();
  const automatic = page.getByLabel(/Check for updates when AutoDock opens/);
  await expect(automatic).toBeChecked();
  await automatic.uncheck();
  await expect(automatic).not.toBeChecked();
  expect(await calls(page, "update_set_check_on_start")).toEqual([{ enabled: false, lang: "en" }]);
});

test("Check now says when the installed version is the latest", async ({ page }) => {
  await open(page, { offer: null });
  await settings(page);
  await page.getByRole("button", { name: "Check now" }).click();
  await expect(page.getByText("You have the latest version.")).toBeVisible();
  expect(await calls(page, "update_check_now")).toHaveLength(1);
  await expect(banner(page)).toHaveCount(0);
});

test("Check now says why it could not check", async ({ page }) => {
  await open(page, { offer: null, nowError: "could not reach the update server" });
  await settings(page);
  await page.getByRole("button", { name: "Check now" }).click();
  await expect(page.getByText("Could not check for updates: could not reach the update server")).toBeVisible();
  await expect(banner(page)).toHaveCount(0);
});

test("Check now can offer a version that was skipped before", async ({ page }) => {
  await open(page, { offer: null, nowOffer: OFFER });
  await settings(page);
  await page.getByRole("button", { name: "Check now" }).click();
  await expect(banner(page)).toContainText("AutoDock 0.4.0 is available");
  await expect(page.getByText("AutoDock 0.4.0 is available: see the notice at the top.")).toBeVisible();
});
