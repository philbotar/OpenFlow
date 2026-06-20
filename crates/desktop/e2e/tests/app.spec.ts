import { test, expect } from "../fixtures.js";

test.describe("OpenFlow shell", () => {
  test("shows brand title after bootstrap", async ({ tauriPage, mode }) => {
    test.skip(mode === "tauri" && !!process.env.CI, "Tauri E2E runs locally or in a dedicated job");

    await tauriPage.waitForSelector(".brand-title", { timeout: 15_000 });
    await expect(tauriPage.locator(".brand-title")).toContainText("OpenFlow");
  });
});
