import { test, expect } from "../fixtures.visual.js";

test.describe("menu density", () => {
  test.beforeEach(async ({ context }) => {
    await context.addInitScript(() => {
      localStorage.setItem("openflow.firstRunOnboardingDismissed", "true");
      localStorage.setItem("openflow.uiZoom", "1.4");
    });
  });

  test("keeps compact menu rows aligned under UI zoom", async ({ tauriPage, mode }) => {
    test.skip(mode === "tauri", "CSS density is covered by the browser project");

    await tauriPage.waitForSelector(".sidebar", { timeout: 15_000 });

    const sidebarHeights = await tauriPage
      .locator(".sidebar-nav-button")
      .evaluateAll((rows) => rows.map((row) => getComputedStyle(row).height));

    await tauriPage.getByRole("button", { name: "Inspector", exact: true }).click();
    const inspectorTrigger = tauriPage.locator(".inspector-panel .text-select-trigger").first();
    await inspectorTrigger.click();
    const inspectorMenu = tauriPage.locator(".text-select-menu");
    await expect(inspectorMenu).toBeVisible();
    await expect
      .poll(async () => {
        const triggerBox = await inspectorTrigger.boundingBox();
        const menuBox = await inspectorMenu.boundingBox();
        return Math.abs((menuBox?.x ?? 0) - (triggerBox?.x ?? 0));
      })
      .toBeLessThanOrEqual(1);
    const inspectorTriggerBox = await inspectorTrigger.boundingBox();
    const inspectorMenuBox = await inspectorMenu.boundingBox();
    expect(inspectorTriggerBox).not.toBeNull();
    expect(inspectorMenuBox).not.toBeNull();
    expect(
      Math.abs((inspectorMenuBox?.x ?? 0) - (inspectorTriggerBox?.x ?? 0)),
    ).toBeLessThanOrEqual(1);
    expect(
      Math.abs((inspectorMenuBox?.width ?? 0) - (inspectorTriggerBox?.width ?? 0)),
    ).toBeLessThanOrEqual(1);
    await inspectorMenu.locator(".text-select-option").first().click();

    if (!("playwrightPage" in tauriPage)) {
      throw new Error("viewport collision test requires the browser page adapter");
    }
    await tauriPage.playwrightPage.setViewportSize({ width: 1280, height: 480 });
    const reasoningTrigger = tauriPage.locator(".inspector-panel .text-select-trigger").last();
    await reasoningTrigger.scrollIntoViewIfNeeded();
    const viewportHeight = await tauriPage.evaluate(() => window.innerHeight);
    await expect
      .poll(async () => {
        const box = await reasoningTrigger.boundingBox();
        return viewportHeight - ((box?.y ?? 0) + (box?.height ?? 0));
      })
      .toBeLessThan(200);
    const bottomTriggerBox = await reasoningTrigger.boundingBox();
    expect(bottomTriggerBox).not.toBeNull();

    await reasoningTrigger.press("Enter");
    const upwardMenu = tauriPage.locator(".text-select-menu");
    await expect(upwardMenu).toHaveClass(/text-select-menu--above/);
    const upwardMenuBox = await upwardMenu.boundingBox();
    expect(upwardMenuBox).not.toBeNull();
    expect(upwardMenuBox?.y ?? -1).toBeGreaterThanOrEqual(0);
    expect((upwardMenuBox?.y ?? 0) + (upwardMenuBox?.height ?? 0)).toBeLessThanOrEqual(
      (bottomTriggerBox?.y ?? 0) + 1,
    );
    await reasoningTrigger.press("Enter");
    await tauriPage.playwrightPage.setViewportSize({ width: 1280, height: 720 });

    const approvalTrigger = tauriPage.locator(".inspector-panel .text-select-trigger").nth(2);
    await approvalTrigger.scrollIntoViewIfNeeded();
    await approvalTrigger.press("Enter");
    const longApprovalOption = tauriPage.getByRole("option", {
      name: "Read auto-approve, write prompt",
      exact: true,
    });
    const approvalOptionOverflow = await longApprovalOption
      .locator(".text-select-option-label")
      .evaluate((label) => ({
        clientHeight: label.clientHeight,
        scrollHeight: label.scrollHeight,
        whiteSpace: getComputedStyle(label).whiteSpace,
      }));
    expect(approvalOptionOverflow.whiteSpace).toBe("nowrap");
    expect(approvalOptionOverflow.scrollHeight).toBeLessThanOrEqual(
      approvalOptionOverflow.clientHeight,
    );
    await approvalTrigger.press("Enter");

    await tauriPage.getByRole("button", { name: "Settings", exact: true }).click();
    const settingsHeights = await tauriPage
      .locator(".settings-back-button, .settings-nav-button")
      .evaluateAll((rows) => rows.map((row) => getComputedStyle(row).height));

    await tauriPage.getByRole("button", { name: "Providers", exact: true }).click();
    const selectTrigger = tauriPage.locator(
      'section[aria-labelledby="providers-active-heading"] .text-select-trigger',
    );
    await selectTrigger.click();
    await tauriPage.waitForSelector(".text-select-option");
    const selectMenu = tauriPage.locator(".text-select-menu");
    const selectHeights = await selectMenu
      .locator(".text-select-option")
      .evaluateAll((rows) => rows.map((row) => getComputedStyle(row).height));
    const triggerBox = await selectTrigger.boundingBox();
    const menuBox = await selectMenu.boundingBox();

    expect(sidebarHeights.length).toBeGreaterThan(0);
    expect(settingsHeights.length).toBeGreaterThan(0);
    expect(selectHeights.length).toBeGreaterThan(0);
    expect(new Set([...sidebarHeights, ...settingsHeights, ...selectHeights])).toEqual(
      new Set(["30px"]),
    );
    expect(triggerBox).not.toBeNull();
    expect(menuBox).not.toBeNull();
    expect(Math.abs((menuBox?.x ?? 0) - (triggerBox?.x ?? 0))).toBeLessThanOrEqual(1);
    expect(Math.abs((menuBox?.width ?? 0) - (triggerBox?.width ?? 0))).toBeLessThanOrEqual(1);
  });
});
