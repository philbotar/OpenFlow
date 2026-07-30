import { defineConfig, devices } from "@playwright/test";

export default defineConfig({
  testDir: "./walkthroughs",
  testMatch: "**/*.capture.ts",
  outputDir: "./test-results/readme-demo",
  fullyParallel: false,
  workers: 1,
  retries: 0,
  timeout: 120_000,
  reporter: "list",
  use: {
    ...devices["Desktop Chrome"],
    viewport: { width: 1440, height: 900 },
    deviceScaleFactor: 1,
    trace: "retain-on-failure",
  },
  webServer: {
    command: "npm --prefix ../../ui run dev",
    port: 1420,
    reuseExistingServer: true,
  },
});
