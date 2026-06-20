import { defineConfig, devices } from "@playwright/test";

export default defineConfig({
  testDir: "./tests",
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  reporter: process.env.CI ? "github" : "list",
  expect: {
    toHaveScreenshot: {
      maxDiffPixelRatio: 0.01,
      animations: "disabled",
    },
  },
  use: {
    trace: "on-first-retry",
  },
  projects: [
    {
      name: "visual",
      testMatch: "**/*.visual.spec.ts",
      use: {
        ...devices["Desktop Chrome"],
        mode: "browser",
        viewport: { width: 1280, height: 900 },
        deviceScaleFactor: 1,
      },
    },
    {
      name: "browser-only",
      testIgnore: "**/*.visual.spec.ts",
      use: { ...devices["Desktop Chrome"], mode: "browser" },
    },
    {
      name: "tauri",
      use: { mode: "tauri" },
    },
  ],
  webServer: {
    command: "npm --prefix ../../ui run dev",
    port: 1420,
    reuseExistingServer: !process.env.CI,
  },
});
