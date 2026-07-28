import { test, expect } from "../fixtures.inspector.js";

test.describe("inspector skill menu", () => {
  test.beforeEach(async ({ context }) => {
    await context.addInitScript(() => {
      localStorage.setItem("openflow.firstRunOnboardingDismissed", "true");
      localStorage.setItem("openflow.uiZoom", "1.4");
    });
  });

  test("paints above the following inspector section", async ({ tauriPage, mode }) => {
    test.skip(mode === "tauri", "CSS paint order is covered by the browser project");

    if (!("playwrightPage" in tauriPage)) {
      throw new Error("paint-order test requires the browser page adapter");
    }
    await tauriPage.playwrightPage.setViewportSize({ width: 1600, height: 900 });
    await tauriPage.waitForSelector(".sidebar", { timeout: 15_000 });
    await tauriPage.getByRole("button", { name: "Inspector", exact: true }).click();

    const taskPrompt = tauriPage.getByRole("combobox", { name: "Task prompt" });
    await taskPrompt.fill("Complete this task with /");

    const menu = tauriPage.locator(".skill-command-combobox");
    const nextSection = tauriPage.getByRole("button", {
      name: "Output schema",
      exact: true,
    });
    await expect(menu).toBeVisible();
    await nextSection.scrollIntoViewIfNeeded();

    const menuBox = await menu.boundingBox();
    const nextSectionBox = await nextSection.boundingBox();
    expect(menuBox).not.toBeNull();
    expect(nextSectionBox).not.toBeNull();

    const overlapTop = Math.max(menuBox?.y ?? 0, nextSectionBox?.y ?? 0);
    const overlapBottom = Math.min(
      (menuBox?.y ?? 0) + (menuBox?.height ?? 0),
      (nextSectionBox?.y ?? 0) + (nextSectionBox?.height ?? 0),
    );
    expect(overlapBottom).toBeGreaterThan(overlapTop);

    const topmostElementIsMenu = await tauriPage.playwrightPage.evaluate(
      ({ x, y }) =>
        document.elementFromPoint(x, y)?.closest(".skill-command-combobox") !== null,
      {
        x: (nextSectionBox?.x ?? 0) + (nextSectionBox?.width ?? 0) / 2,
        y: overlapTop + (overlapBottom - overlapTop) / 2,
      },
    );
    expect(topmostElementIsMenu).toBe(true);
  });
});
