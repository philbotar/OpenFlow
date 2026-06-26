// @vitest-environment jsdom
import { render } from "solid-js/web";
import { afterEach, beforeEach, describe, expect, test } from "vitest";
import { SidebarNavButton } from "./SidebarNavButton";

describe("SidebarNavButton", () => {
  let container: HTMLDivElement;
  let dispose: (() => void) | undefined;

  beforeEach(() => {
    container = document.createElement("div");
    document.body.appendChild(container);
  });

  afterEach(() => {
    dispose?.();
    container.remove();
  });

  test("shows update badge when updateAvailable is true", () => {
    dispose = render(
      () => (
        <SidebarNavButton
          icon="settings"
          label="Settings"
          updateAvailable
          onClick={() => {}}
        />
      ),
      container,
    );
    expect(container.querySelector(".sidebar-nav-update-badge")).not.toBeNull();
    expect(container.querySelector("button")?.getAttribute("aria-label")).toBe(
      "Settings (update available)",
    );
  });

  test("hides update badge by default", () => {
    dispose = render(
      () => <SidebarNavButton icon="settings" label="Settings" onClick={() => {}} />,
      container,
    );
    expect(container.querySelector(".sidebar-nav-update-badge")).toBeNull();
    expect(container.querySelector("button")?.getAttribute("aria-label")).toBe("Settings");
  });
});
