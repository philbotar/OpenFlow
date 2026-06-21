import { test, expect } from "../fixtures.js";

test.describe("settings providers", () => {
  test.beforeEach(async ({ tauriPage }) => {
    await tauriPage.waitForSelector(".brand-title", { timeout: 15_000 });
    await tauriPage.getByRole("button", { name: "Settings", exact: true }).click();
    await tauriPage.getByRole("button", { name: "Providers", exact: true }).click();
    await tauriPage.waitForSelector(".providers-section", { timeout: 5_000 });
  });

  test("renders providers section with readiness chip", async ({ tauriPage }) => {
    await expect(tauriPage.locator(".providers-section")).toBeVisible();
    await expect(tauriPage.locator(".readiness-chip")).toBeVisible();
    await expect(tauriPage.locator('input[type="password"]')).toBeVisible();
    await expect(tauriPage.locator(".readiness-chip")).toContainText("Ready via env var");
  });

  test("switches provider and saves API key", async ({ tauriPage }) => {
    const apiKey = tauriPage.locator('input[type="password"]');
    await expect(apiKey).toHaveValue("stored-openai-key");

    await tauriPage.evaluate(`
      (async () => {
        const trigger = document.querySelector(
          'section[aria-labelledby="providers-active-heading"] .text-select-trigger',
        );
        trigger?.click();
        await new Promise((resolve) =>
          requestAnimationFrame(() => requestAnimationFrame(resolve)),
        );
        const option = [...document.querySelectorAll(".text-select-option")].find((el) =>
          el.textContent?.includes("Compatible"),
        );
        option?.click();
      })();
    `);

    await expect(apiKey).toHaveValue("stored-compatible-key");

    await apiKey.fill("updated-compatible-key");
    await tauriPage.getByRole("button", { name: "Save settings" }).click();

    await expect(tauriPage.getByText("Settings saved successfully.")).toBeVisible({
      timeout: 5_000,
    });

    const calls = await tauriPage.evaluate(
      () =>
        (window as unknown as { __openflowE2e?: { calls: unknown[] } }).__openflowE2e?.calls ?? [],
    );

    expect(calls).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          type: "save_provider_api_key",
          providerId: "custom_openai_compatible",
          apiKey: "updated-compatible-key",
        }),
        expect.objectContaining({
          type: "save_settings",
          settings: expect.objectContaining({
            active_provider: "custom_openai_compatible",
          }),
        }),
      ]),
    );
  });
});
