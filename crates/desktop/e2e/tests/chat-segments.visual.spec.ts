import { test, expect } from "../fixtures.visual.js";

test.describe("chat segment spacing", () => {
  test.beforeEach(async ({ context }) => {
    await context.addInitScript(() => {
      localStorage.setItem("step-through-theme", "dark");
      localStorage.setItem("openflow.rightPanelHidden", "true");
      localStorage.setItem("openflow.firstRunOnboardingDismissed", "true");
    });
  });

  test("settled multi-segment transcript", async ({ tauriPage }) => {
    if (!("playwrightPage" in tauriPage)) {
      throw new Error("visual test requires the browser page adapter");
    }
    const page = tauriPage.playwrightPage;
    await expect(
      page.getByRole("banner").getByText("Feature-to-Implementation Pipeline"),
    ).toBeVisible();
    await page.getByRole("button", { name: "Chat", exact: true }).click();
    await expect(page.locator(".chat-segment")).toHaveCount(3);
    await page.getByRole("button", { name: "Focus panel" }).click();
    await expect(page.locator(".editor-screen--chat-focus")).toBeVisible();

    const panel = page.locator(".chat-settled");
    await expect(panel).toContainText("Wrote architecture doc");
    await expect(panel).not.toContainText("Tool request:");
    await expect(panel).toHaveScreenshot("chat-segments-settled-dark.png", {
      mask: [page.locator(".chat-live-streaming-dot")],
    });
  });
});
