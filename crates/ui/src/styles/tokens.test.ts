// @vitest-environment jsdom
import { readFileSync } from "node:fs";
import { afterEach, describe, expect, test } from "vitest";
import { applyTheme } from "../lib/theme";

const tokensCss = readFileSync("src/styles/tokens.css", "utf8");
const indexCss = readFileSync("src/styles/index.css", "utf8");
const onboardingCss = readFileSync(
  "src/components/FirstRunOnboarding/FirstRunOnboarding.css",
  "utf8",
);

describe("dark theme palette", () => {
  let style: HTMLStyleElement | undefined;

  afterEach(() => {
    document.body.innerHTML = "";
    style?.remove();
    style = undefined;
    document.documentElement.removeAttribute("data-theme");
    document.documentElement.style.removeProperty("color-scheme");
  });

  test("uses neutral charcoal surfaces and actions", () => {
    expect(tokensCss).toContain('[data-theme="dark"]');
    style = document.createElement("style");
    style.textContent = tokensCss.slice(tokensCss.indexOf('[data-theme="dark"]'));
    document.head.append(style);
    applyTheme("dark");

    const theme = getComputedStyle(document.documentElement);

    expect(theme.getPropertyValue("--base-sand-100").trim()).toBe("#101010");
    expect(theme.getPropertyValue("--base-sand-50").trim()).toBe("#181818");
    expect(theme.getPropertyValue("--surface-panel").trim()).toBe(
      "rgba(28, 28, 29, 0.9)",
    );
    expect(theme.getPropertyValue("--accent-primary").trim()).toBe(
      "var(--base-action-500)",
    );
    expect(theme.getPropertyValue("--base-action-500").trim()).toBe("#e8e8ea");
    expect(theme.getPropertyValue("--sidebar-active").trim()).toBe(
      "rgba(255, 255, 255, 0.08)",
    );
  });

  test("uses dark text for the dark topbar run action", () => {
    style = document.createElement("style");
    style.textContent = `${tokensCss}\n${indexCss}`;
    document.head.append(style);
    applyTheme("dark");

    const runButton = document.createElement("button");
    runButton.className = "topbar-primary-button";
    document.body.append(runButton);

    expect(
      getComputedStyle(document.documentElement)
        .getPropertyValue("--action-foreground")
        .trim(),
    ).toBe("#111111");
    expect(getComputedStyle(runButton).color).toBe("var(--action-foreground)");

    runButton.remove();
  });

  test("uses theme-safe contrast for dark onboarding actions", () => {
    style = document.createElement("style");
    style.textContent = `${tokensCss}\n${onboardingCss}`;
    document.head.append(style);
    applyTheme("dark");

    const nextButton = document.createElement("button");
    nextButton.className = "of-tour-next";
    const skipButton = document.createElement("button");
    skipButton.className = "of-tour-skip";
    document.body.append(nextButton, skipButton);

    expect(getComputedStyle(nextButton).color).toBe("var(--action-foreground)");
    expect(getComputedStyle(skipButton).color).toBe("var(--text-muted)");
  });

  test("matches agent delete geometry to save", () => {
    style = document.createElement("style");
    style.textContent = `${tokensCss}\n${indexCss}`;
    document.head.append(style);

    const deleteButton = document.createElement("button");
    deleteButton.className = "danger-button compact agent-delete-button";
    const saveButton = document.createElement("button");
    saveButton.className = "primary-button compact";
    document.body.append(deleteButton, saveButton);

    const deleteStyle = getComputedStyle(deleteButton);
    const saveStyle = getComputedStyle(saveButton);
    expect(deleteStyle.minHeight).toBe(saveStyle.minHeight);
    expect(deleteStyle.padding).toBe(saveStyle.padding);
    expect(deleteStyle.fontSize).toBe(saveStyle.fontSize);
    expect(deleteStyle.fontWeight).toBe(saveStyle.fontWeight);
    expect(deleteStyle.borderRadius).toBe(saveStyle.borderRadius);
  });

  test("constrains sidebar lists horizontally while menus overlay vertically", () => {
    style = document.createElement("style");
    style.textContent = `${tokensCss}\n${indexCss}`;
    document.head.append(style);

    const section = document.createElement("div");
    section.className = "sidebar-section-group sidebar-workflows-section";
    const collapsible = document.createElement("div");
    collapsible.className =
      "collapsible-section collapsible-section--open sidebar-workflows-collapsible";
    const inner = document.createElement("div");
    inner.className = "collapsible-section-inner";
    collapsible.append(inner);
    section.append(collapsible);
    document.body.append(section);

    expect(getComputedStyle(inner).overflowX).toBe("clip");
    expect(getComputedStyle(inner).overflowY).toBe("visible");
    expect(getComputedStyle(inner).minWidth).toBe("0");
    expect(getComputedStyle(section).position).toBe("relative");
    expect(getComputedStyle(section).zIndex).toBe("var(--z-dropdown)");
  });

  test("stacks chat menus above the following workflow section", () => {
    style = document.createElement("style");
    style.textContent = `${tokensCss}\n${indexCss}`
      .split("calc(var(--z-dropdown) + 1)")
      .join("121")
      .split("var(--z-dropdown)")
      .join("120");
    document.head.append(style);

    const chatsSection = document.createElement("div");
    chatsSection.className = "sidebar-section-group sidebar-chats-section";
    const workflowsSection = document.createElement("div");
    workflowsSection.className = "sidebar-section-group sidebar-workflows-section";
    document.body.append(chatsSection, workflowsSection);

    expect(Number(getComputedStyle(chatsSection).zIndex)).toBeGreaterThan(
      Number(getComputedStyle(workflowsSection).zIndex),
    );
  });
});
