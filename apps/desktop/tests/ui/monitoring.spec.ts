import { test, expect } from "@playwright/test";

test.use({ locale: "en-US" });

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    let n = 0;
    const base = Date.now() - 60_000;
    (window as any).__TAURI_INTERNALS__ = {
      invoke: async (command: string) => {
        if (command === "doctor_environment")
          return { schemaVersion: 1, os: "linux", notes: [], checks: [] };
        if (command === "collect_container_metrics") {
          n += 1;
          return {
            schemaVersion: 1,
            collectedAtUnixMs: base + n * 5000,
            containers: [
              {
                id: "abcdef1234567890",
                name: "web",
                cpuPercent: 10 + ((n * 17) % 60),
                memoryUsageBytes: 120_000_000,
                memoryLimitBytes: 1_000_000_000,
                memoryPercent: 12 + n,
                networkRxBytes: n * n * 40_000,
                networkTxBytes: n * 25_000,
              },
            ],
          };
        }
        throw new Error(`Unexpected IPC ${command}`);
      },
    };
  });
  await page.goto("/");
});

test("monitoring shows four charts with stats and a hover tooltip", async ({ page }) => {
  await page.getByRole("navigation").getByRole("button", { name: "Monitoring", exact: true }).click();
  const refresh = page.getByRole("button", { name: "Refresh" }).first();
  for (let i = 0; i < 5; i++) {
    await refresh.click();
    await expect(refresh).toBeEnabled();
  }
  const charts = page.locator("svg.ad-monitor-chart");
  await expect(charts).toHaveCount(4);
  await expect(page.getByText("Network received per second")).toBeVisible();
  await expect(page.locator(".ad-monitor-stats").first()).toContainText("Max");

  await charts.first().hover({ position: { x: 200, y: 80 } });
  await expect(page.locator(".ad-monitor-tooltip")).toBeVisible();

  await page.mouse.move(0, 0);
  await expect(page.locator(".ad-monitor-tooltip")).toHaveCount(0);
  await charts.nth(2).focus();
  await page.keyboard.press("ArrowLeft");
  await expect(page.locator(".ad-monitor-tooltip")).toBeVisible();
});
