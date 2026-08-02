import { test, expect } from "../fixtures.chat-runtime.js";

test.describe("chat runtime controls", () => {
  test.beforeEach(async ({ context }) => {
    await context.addInitScript(() => {
      localStorage.setItem("openflow.firstRunOnboardingDismissed", "true");
    });
  });

  test("shows nested runtime options on hover in a horizontal flyout", async ({
    tauriPage,
  }) => {
    await tauriPage.waitForSelector(".sidebar", 15_000);
    await tauriPage.getByRole("button", { name: "Runtime chat", exact: true }).click();

    await tauriPage
      .getByRole("button", { name: /Chat runtime settings:/ })
      .click();
    const viewportHeight = (await tauriPage.evaluate("window.innerHeight")) as number;
    const selectOption = async (triggerName: string, optionName: string) => {
      const trigger = tauriPage.getByRole("button", { name: triggerName });
      const triggerBox = await trigger.boundingBox();
      expect(triggerBox).not.toBeNull();
      await trigger.hover();
      const option = tauriPage.getByRole("option", {
        name: optionName,
        exact: true,
      });
      await expect(option).toBeVisible();
      const optionBox = await option.boundingBox();
      expect(optionBox).not.toBeNull();
      const triggerLeft = triggerBox?.x ?? 0;
      const triggerRight = triggerLeft + (triggerBox?.width ?? 0);
      const optionLeft = optionBox?.x ?? 0;
      const optionRight = optionLeft + (optionBox?.width ?? 0);
      expect(
        optionRight <= triggerLeft || optionLeft >= triggerRight,
      ).toBe(true);
      expect(optionBox?.y ?? -1).toBeGreaterThanOrEqual(0);
      expect((optionBox?.y ?? 0) + (optionBox?.height ?? 0)).toBeLessThanOrEqual(
        viewportHeight,
      );
      await option.click();
      await expect(trigger).toContainText(optionName);
    };

    await selectOption("Chat model", "gpt-5");
    await selectOption("Chat reasoning effort", "High");
    await selectOption("Chat speed", "Fast");
  });
});
