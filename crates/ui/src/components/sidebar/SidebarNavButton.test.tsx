// @vitest-environment jsdom
import { render } from "solid-js/web";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";
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

  test("shows a trailing Update action when updateAvailable is true", () => {
    const onClick = vi.fn();
    dispose = render(
      () => (
        <SidebarNavButton
          icon="settings"
          label="Settings"
          updateAvailable
          onClick={onClick}
        />
      ),
      container,
    );
    const button = container.querySelector("button");
    const updateAction = container.querySelector(".sidebar-nav-update-action");
    expect(updateAction?.textContent).toBe("Update");
    expect(container.querySelector(".sidebar-nav-update-badge")).toBeNull();
    expect(button?.getAttribute("aria-label")).toBe(
      "Settings (update available)",
    );
    updateAction?.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    expect(onClick).toHaveBeenCalledOnce();
  });

  test("hides the Update action by default", () => {
    dispose = render(
      () => <SidebarNavButton icon="settings" label="Settings" onClick={() => {}} />,
      container,
    );
    expect(container.querySelector(".sidebar-nav-update-action")).toBeNull();
    expect(container.querySelector("button")?.getAttribute("aria-label")).toBe("Settings");
  });
});
