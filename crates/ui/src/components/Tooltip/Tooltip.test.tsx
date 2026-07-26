// @vitest-environment jsdom
import { render } from "solid-js/web";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";
import { Tooltip } from "./Tooltip";

describe("Tooltip", () => {
  let container: HTMLDivElement;
  let dispose: () => void;

  beforeEach(() => {
    vi.useFakeTimers();
    if (typeof globalThis.PointerEvent === "undefined") {
      class PointerEventPolyfill extends MouseEvent {
        constructor(type: string, params?: MouseEventInit) {
          super(type, params);
        }
      }
      vi.stubGlobal("PointerEvent", PointerEventPolyfill);
    }
    container = document.createElement("div");
    document.body.append(container);
  });

  afterEach(() => {
    dispose?.();
    container?.remove();
    vi.useRealTimers();
    document.querySelectorAll(".app-tooltip").forEach((el) => el.remove());
  });

  test("shows label and shortcut chips after delay on hover", () => {
    dispose = render(
      () => (
        <Tooltip label="Save workflow" shortcutId="save">
          <button type="button" aria-label="Save workflow">
            Save
          </button>
        </Tooltip>
      ),
      container,
    );

    const trigger = container.querySelector("button")!;
    trigger.dispatchEvent(new PointerEvent("pointerenter", { bubbles: true }));
    expect(document.querySelector(".app-tooltip")).toBeNull();

    vi.advanceTimersByTime(400);
    const tip = document.querySelector(".app-tooltip");
    expect(tip).not.toBeNull();
    expect(tip?.textContent).toContain("Save workflow");
    expect(tip?.querySelectorAll(".app-tooltip-key").length).toBeGreaterThan(0);
  });

  test("shows disabledReason after delay when hovering wrapper around disabled button", () => {
    dispose = render(
      () => (
        <Tooltip label="Run" disabledReason="Stop the run first">
          <button type="button" disabled aria-label="Run">
            Run
          </button>
        </Tooltip>
      ),
      container,
    );

    const wrapper = container.querySelector(".app-tooltip-trigger")!;
    wrapper.dispatchEvent(new PointerEvent("pointerenter", { bubbles: true }));
    vi.advanceTimersByTime(400);
    const tip = document.querySelector(".app-tooltip");
    expect(tip).not.toBeNull();
    expect(tip?.textContent).toContain("Stop the run first");
  });

  test("hides on pointer leave", () => {
    dispose = render(
      () => (
        <Tooltip label="Inspector">
          <button type="button" aria-label="Inspector">
            I
          </button>
        </Tooltip>
      ),
      container,
    );
    const trigger = container.querySelector("button")!;
    trigger.dispatchEvent(new PointerEvent("pointerenter", { bubbles: true }));
    vi.advanceTimersByTime(400);
    trigger.dispatchEvent(new PointerEvent("pointerleave", { bubbles: true }));
    expect(document.querySelector(".app-tooltip")).toBeNull();
  });
});
