import { test, expect, type Page } from "@playwright/test";

test.use({ locale: "en-US" });

const project = (over: Record<string, unknown> = {}) => ({
  schemaVersion: 1,
  path: "/Users/maria/meu-app",
  name: "meu-app",
  declaredVersion: "~57.0.32",
  installedVersion: "57.0.32",
  sdk: 57,
  reactNative: "0.86.3",
  usesRouter: true,
  packageManager: "npm",
  lockFile: "package-lock.json",
  nativeFolders: [],
  upgradeSupported: true,
  unsupportedReason: null,
  ...over,
});

const plan = {
  schemaVersion: 1,
  project: project(),
  fromSdk: 57,
  toSdk: 58,
  packageManager: "npm",
  image: "node:22-bookworm-slim",
  steps: [
    "Backup of package.json and the lock file.",
    "npm install expo@^58.0.0: installs the expo of SDK 58.",
    "npx expo install --fix: aligns every dependency with the version SDK 58 expects.",
    "npx expo-doctor: checks the project and produces the report.",
  ],
  command: "docker run --rm node:22-bookworm-slim sh -c '…'",
  filesTouched: ["package.json", "package-lock.json"],
  notes: ["Expo Go only opens projects on the latest SDK; a project on an older SDK needs a development build."],
  warnings: ["The android folder has native code under version control."],
  links: [{ label: "SDK 58 release notes", url: "https://expo.dev/changelog/sdk-58" }],
};

type Options = {
  latest?: number | null;
  project?: Record<string, unknown>;
  fail?: boolean;
  dismissed?: boolean;
};

async function open(page: Page, options: Options = {}) {
  await page.addInitScript(
    ({ plan, options, base }) => {
      const w = window as any;
      w.calls = [];
      const latest = options.latest === undefined ? 58 : options.latest;
      w.state = {
        schemaVersion: 1,
        checkOnStart: false,
        latest: latest ? { checkedAt: 1794000000, latestVersion: `${latest}.0.23`, latestSdk: latest, availableSdks: [57, latest] } : null,
        projects: [{ path: base.path, name: base.name, project: { ...base, ...options.project }, update: null }],
        alerts: 0,
        checkError: null,
      };
      w.dismissed = !!options.dismissed;
      w.backups = [];
      // What the Rust side derives: an update exists when the project is behind the latest SDK.
      const recalc = () => {
        let alerts = 0;
        for (const item of w.state.projects) {
          const sdk = item.project?.sdk;
          const latestSdk = w.state.latest?.latestSdk;
          if (sdk && latestSdk && latestSdk > sdk) {
            item.update = { currentSdk: sdk, latestSdk, nextSdk: sdk + 1, stepsBehind: latestSdk - sdk, dismissed: w.dismissed };
            if (!w.dismissed) alerts += 1;
          } else {
            item.update = null;
          }
        }
        w.state.alerts = alerts;
        return JSON.parse(JSON.stringify(w.state));
      };
      w.__TAURI_INTERNALS__ = {
        invoke: async (command: string, args: any) => {
          w.calls.push({ command, args });
          switch (command) {
            case "expo_watch_state":
            case "expo_auto_check":
              return recalc();
            case "expo_check_latest":
              w.state.latest = { checkedAt: 1794000100, latestVersion: "58.0.23", latestSdk: 58, availableSdks: [57, 58] };
              return recalc();
            case "expo_set_check_on_start":
              w.state.checkOnStart = args.enabled;
              return recalc();
            case "expo_watch_dismiss":
              w.dismissed = true;
              return recalc();
            case "expo_watch_remove":
              w.state.projects = [];
              return recalc();
            case "expo_watch_add":
              return recalc();
            case "expo_inspect":
              return null;
            case "expo_upgrade_plan":
              return plan;
            case "expo_backups":
              return w.backups;
            case "expo_upgrade_run":
              if (w.fail || options.fail) {
                return {
                  schemaVersion: 1, succeeded: false, restored: true, fromSdk: 57, toSdk: 58, changes: [],
                  doctor: "", doctorPassed: null, usedLegacyPeerDeps: false,
                  output: "STEP install ok\nSTEP align failed\nnpm error code ERESOLVE",
                  detail: "The upgrade did not finish. package.json and the lock file are back to how they were.",
                  durationMs: 9000, backupId: "1794000300-57-58", nextSteps: [],
                };
              }
              w.state.projects[0].project = { ...w.state.projects[0].project, sdk: 58, declaredVersion: "^58.0.23" };
              w.backups = [{ id: "1794000300-57-58", fromSdk: 57, toSdk: 58, createdAt: 1794000300, status: "done", files: ["package.json", "package-lock.json"], modifiedSinceUpgrade: false }];
              return {
                schemaVersion: 1, succeeded: true, restored: false, fromSdk: 57, toSdk: 58,
                changes: [
                  { name: "expo", section: "dependencies", from: "~57.0.32", to: "^58.0.23" },
                  { name: "react-native", section: "dependencies", from: "0.86.3", to: "0.88.1" },
                ],
                doctor: "Running 20 checks on your project...\n20/20 checks passed. No issues detected!",
                doctorPassed: true, usedLegacyPeerDeps: true, output: "",
                detail: "npm could not resolve the peer dependencies and the upgrade used --legacy-peer-deps.",
                durationMs: 41000, backupId: "1794000300-57-58",
                nextSteps: ["Run npm install in the project to update the node_modules on your computer.", "Read the SDK 58 release notes and test the app."],
              };
            case "expo_restore":
              // like the real backup, it stays after being restored
              w.state.projects[0].project = { ...w.state.projects[0].project, sdk: 57, declaredVersion: "~57.0.32" };
              return { schemaVersion: 1, restored: ["package.json", "package-lock.json"], removed: [], detail: "package.json and the lock file are back to the SDK 57 state." };
            case "plugin:dialog|open":
              return "/Users/maria/outro-app";
            default:
              throw new Error(`Unexpected IPC ${command}`);
          }
        },
      };
    },
    { plan, options, base: project() },
  );
  await page.goto("/");
}

const alert = (page: Page) => page.getByTestId("expo-alert");
const calls = (page: Page, command: string) =>
  page.evaluate((name) => (window as any).calls.filter((c: any) => c.command === name).map((c: any) => c.args), command);

test("tells the user the project is behind and upgrades it after a confirmation", async ({ page }) => {
  await open(page);
  await expect(alert(page)).toContainText(
    "Your project meu-app is on SDK 57, but Expo released stable SDK 58. Do you want to upgrade?",
  );
  await expect(page.getByRole("navigation").getByRole("button", { name: /^Expo/ })).toContainText("1");

  await alert(page).getByRole("button", { name: "See upgrade" }).click();
  await expect(page.getByRole("heading", { name: "Expo projects" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "meu-app: SDK 57 → 58" })).toBeVisible();
  await expect(page.getByText("npm install expo@^58.0.0: installs the expo of SDK 58.")).toBeVisible();
  await expect(page.getByText("The android folder has native code under version control.")).toBeVisible();
  await expect(page.getByText("https://expo.dev/changelog/sdk-58")).toBeVisible();
  await expect(alert(page)).toHaveCount(0);
  expect(await calls(page, "expo_upgrade_plan")).toEqual([{ path: "/Users/maria/meu-app", lang: "en" }]);

  // nothing runs until the user confirms
  const start = page.locator(".multi-run-actions").getByRole("button", { name: "Upgrade to SDK 58" });
  await expect(start).toBeDisabled();
  expect(await calls(page, "expo_upgrade_run")).toEqual([]);
  await page.getByLabel(/I understand AutoDock will run these commands in Docker/).check();
  await start.click();

  await expect(page.getByText("Upgraded from SDK 57 to 58 in 41 s.")).toBeVisible();
  await expect(page.getByText(/used --legacy-peer-deps/)).toBeVisible();
  await expect(page.getByText("expo-doctor found no problems.")).toBeVisible();
  await expect(page.getByText("2 dependencies changed")).toBeVisible();
  await page.getByText("2 dependencies changed").click();
  await expect(page.getByRole("table")).toContainText("react-native");
  await expect(page.getByRole("table")).toContainText("0.88.1");
  await expect(page.getByText("Run npm install in the project")).toBeVisible();
  const run = await calls(page, "expo_upgrade_run");
  expect(run.map((r: any) => r.request)).toEqual([
    { path: "/Users/maria/meu-app", targetSdk: 58, confirmed: true },
  ]);
  // the project now shows the new SDK and is up to date
  await expect(page.locator(".expo-projects")).toContainText("SDK 58");
  await expect(page.locator(".expo-projects")).toContainText("Up to date");
  await expect(page.getByRole("navigation").getByRole("button", { name: /^Expo/ })).not.toContainText("1");

  // the backup can bring the old files back, after a second click
  await expect(page.getByText("Backups of package.json and the lock file")).toBeVisible();
  await page.getByRole("button", { name: "Restore" }).click();
  await expect(page.getByRole("button", { name: "Confirm restore" })).toBeVisible();
  expect(await calls(page, "expo_restore")).toEqual([]);
  await page.getByRole("button", { name: "Confirm restore" }).click();
  await expect(page.getByText("back to the SDK 57 state")).toBeVisible();
  expect((await calls(page, "expo_restore")).map((r: any) => [r.path, r.backupId, r.confirmed])).toEqual([
    ["/Users/maria/meu-app", "1794000300-57-58", true],
  ]);
});

test("dismissing the alert hides it everywhere but keeps the project listed", async ({ page }) => {
  await open(page);
  await expect(alert(page)).toBeVisible();
  await alert(page).getByRole("button", { name: "Dismiss" }).click();
  await expect(alert(page)).toHaveCount(0);
  expect(await calls(page, "expo_watch_dismiss")).toEqual([{ path: "/Users/maria/meu-app", lang: "en" }]);
  await page.getByRole("navigation").getByRole("button", { name: /^Expo/ }).click();
  await expect(page.locator(".expo-projects")).toContainText("meu-app");
  await expect(page.locator(".expo-projects")).toContainText("Alert dismissed");
});

test("a failed upgrade says the files were put back and shows the output", async ({ page }) => {
  await open(page, { fail: true });
  await alert(page).getByRole("button", { name: "See upgrade" }).click();
  await page.getByLabel(/I understand AutoDock will run these commands in Docker/).check();
  await page.locator(".multi-run-actions").getByRole("button", { name: "Upgrade to SDK 58" }).click();
  await expect(page.getByText("The upgrade did not finish.", { exact: true })).toBeVisible();
  await expect(page.getByText("package.json and the lock file were put back as they were.")).toBeVisible();
  await expect(page.getByText("npm error code ERESOLVE")).toBeVisible();
  // the project is still on the old SDK, so the alert is still pending
  await expect(page.locator(".expo-projects")).toContainText("SDK 57");
});

test("the latest SDK is only looked up on request and the automatic check is opt-in", async ({ page }) => {
  await open(page, { latest: null });
  await expect(alert(page)).toHaveCount(0);
  await page.getByRole("navigation").getByRole("button", { name: /^Expo/ }).click();
  await expect(page.getByText("Not looked up yet.")).toBeVisible();
  await expect(page.locator(".expo-projects")).toContainText("Look up the latest SDK to compare");
  const automatic = page.getByLabel(/Check when AutoDock opens/);
  await expect(automatic).not.toBeChecked();
  expect(await calls(page, "expo_check_latest")).toEqual([]);

  await page.getByRole("button", { name: "Check now" }).click();
  await expect(page.getByText("SDK 58", { exact: true }).first()).toBeVisible();
  await expect(page.locator(".expo-projects")).toContainText("SDK 58 is out");
  expect(await calls(page, "expo_check_latest")).toHaveLength(1);

  await automatic.check();
  await expect(automatic).toBeChecked();
  expect(await calls(page, "expo_set_check_on_start")).toEqual([{ enabled: true, lang: "en" }]);
});

test("a project AutoDock cannot upgrade explains why and offers no upgrade", async ({ page }) => {
  await open(page, {
    project: { upgradeSupported: false, unsupportedReason: "This project uses bun, which AutoDock does not upgrade yet." },
  });
  await page.getByRole("navigation").getByRole("button", { name: /^Expo/ }).click();
  await expect(page.locator(".expo-projects")).toContainText("does not upgrade yet");
  await expect(page.locator(".expo-projects").getByRole("button", { name: /^Upgrade to SDK/ })).toHaveCount(0);
});

test("a project can be removed from the list and another added from a folder", async ({ page }) => {
  await open(page);
  await page.getByRole("navigation").getByRole("button", { name: /^Expo/ }).click();
  await page.getByRole("button", { name: "Stop watching" }).click();
  await expect(page.getByText("No watched projects")).toBeVisible();
  expect(await calls(page, "expo_watch_remove")).toEqual([{ path: "/Users/maria/meu-app", lang: "en" }]);

  await page.getByRole("button", { name: "Add project…" }).click();
  await expect.poll(() => calls(page, "expo_watch_add")).toEqual([{ path: "/Users/maria/outro-app", lang: "en" }]);
});
