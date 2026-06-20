import { defineConfig, devices } from "@playwright/test";

export default defineConfig({
  testDir: "./tests",
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  reporter: process.env.CI ? "github" : "list",
  use: {
    trace: "on-first-retry",
  },
  projects: [
    {
      name: "browser-only",
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
