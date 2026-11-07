import { test, expect, type Page } from "@playwright/test";

test.use({ locale: "en-US" });

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
});

const theme = (page: Page) => page.evaluate(() => document.documentElement.dataset.theme);
const background = (page: Page) =>
  page.evaluate(() => getComputedStyle(document.body).backgroundColor);
const switchButton = (page: Page, name: string) =>
  page.getByRole("group", { name: "Theme" }).first().getByRole("button", { name });

test("by default the theme follows the operating system", async ({ page }) => {
  await page.emulateMedia({ colorScheme: "dark" });
  await page.goto("/");
  expect(await theme(page)).toBe("dark");
  await page.emulateMedia({ colorScheme: "light" });
  await expect.poll(() => theme(page)).toBe("light");
});

test("the switch is in the top bar of every screen and the choice is remembered", async ({ page }) => {
  await page.emulateMedia({ colorScheme: "light" });
  await page.goto("/");
  const lightBackground = await background(page);

  await switchButton(page, "Dark theme").click();
  expect(await theme(page)).toBe("dark");
  expect(await background(page)).not.toBe(lightBackground);

  for (const screen of ["Containers", "Learn", "Settings", "Console"]) {
    await page.getByRole("navigation").getByRole("button", { name: screen, exact: true }).click();
    await expect(switchButton(page, "Dark theme")).toHaveAttribute("aria-pressed", "true");
    expect(await theme(page)).toBe("dark");
  }

  await page.reload();
  expect(await theme(page)).toBe("dark");
  await switchButton(page, "Light theme").click();
  expect(await theme(page)).toBe("light");
  expect(await background(page)).toBe(lightBackground);
});

test("an explicit choice ignores the system and system mode takes over again", async ({ page }) => {
  await page.emulateMedia({ colorScheme: "dark" });
  await page.goto("/");
  await switchButton(page, "Light theme").click();
  expect(await theme(page)).toBe("light");
  await page.emulateMedia({ colorScheme: "light" });
  await page.emulateMedia({ colorScheme: "dark" });
  expect(await theme(page)).toBe("light");

  await switchButton(page, "Follow the system theme").click();
  expect(await theme(page)).toBe("dark");
  await page.emulateMedia({ colorScheme: "light" });
  await expect.poll(() => theme(page)).toBe("light");
});

test("Settings offers the same switch and both stay in sync", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("navigation").getByRole("button", { name: "Settings", exact: true }).click();
  const groups = page.getByRole("group", { name: "Theme" });
  await expect(groups).toHaveCount(2);
  await groups.nth(1).getByRole("button", { name: "Dark theme" }).click();
  await expect(groups.nth(0).getByRole("button", { name: "Dark theme" })).toHaveAttribute("aria-pressed", "true");
  expect(await theme(page)).toBe("dark");
});
