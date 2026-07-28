// @vitest-environment jsdom
import { readFileSync } from "node:fs";
import { afterEach, describe, expect, test } from "vitest";
import { applyTheme } from "../lib/theme";

const sonnerCss = readFileSync("node_modules/solid-sonner/dist/styles.css", "utf8");
const tokensCss = readFileSync("src/styles/tokens.css", "utf8");
const indexCss = readFileSync("src/styles/index.css", "utf8");

describe("toast presentation", () => {
  let style: HTMLStyleElement | undefined;

  afterEach(() => {
    document.body.innerHTML = "";
    style?.remove();
    style = undefined;
    document.documentElement.removeAttribute("data-theme");
    document.documentElement.style.removeProperty("color-scheme");
  });

  test("uses the app surface and status tokens instead of the Sonner palette", () => {
    style = document.createElement("style");
    style.textContent = `${sonnerCss}\n${tokensCss}\n${indexCss}`;
    document.head.append(style);
    applyTheme("dark");

    const toast = document.createElement("li");
    toast.className = "app-toast";
    toast.setAttribute("data-sonner-toast", "");
    toast.setAttribute("data-styled", "true");
    toast.setAttribute("data-rich-colors", "true");
    toast.setAttribute("data-type", "success");

    const icon = document.createElement("span");
    icon.setAttribute("data-icon", "");
    toast.append(icon);

    const closeButton = document.createElement("button");
    closeButton.className = "app-toast-close-button";
    closeButton.setAttribute("data-close-button", "");
    toast.append(closeButton);
    document.body.append(toast);

    expect(getComputedStyle(toast).background).toBe("var(--surface-raised)");
    expect(getComputedStyle(toast).borderRadius).toBe("var(--radius-md)");
    expect(getComputedStyle(toast).color).toBe("var(--text-primary)");
    expect(getComputedStyle(icon).color).toBe("var(--status-success)");
    expect(getComputedStyle(closeButton).transform).toBe("none");
  });
});
