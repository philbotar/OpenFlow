import { test, expect } from "../fixtures.visual.js";

test.describe("chat segment spacing", () => {
  test.beforeEach(async ({ context }) => {
    await context.addInitScript(() => {
      localStorage.setItem("step-through-theme", "dark");
      localStorage.setItem("openflow.rightPanelHidden", "true");
    });
  });

  test("settled multi-segment transcript", async ({ tauriPage }) => {
    const page = tauriPage.playwrightPage;
    await tauriPage.waitForSelector(".brand-title", { timeout: 15_000 });
    await tauriPage.getByRole("button", { name: "Chat" }).click();
    await tauriPage.waitForSelector(".chat-segment:nth-child(3)", 15_000);
    await tauriPage.getByRole("button", { name: "Focus chat" }).click();
    await tauriPage.waitForSelector(".editor-screen--chat-focus", 5_000);

    const panel = page.locator(".chat-settled");
    await expect(panel).toHaveScreenshot("chat-segments-settled-dark.png", {
      mask: [page.locator(".chat-live-streaming-dot")],
    });
  });
});
