// @vitest-environment jsdom
import { fireEvent, render } from "@testing-library/react";
import { createElement } from "react";
import { act } from "react";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";
import { AppTooltip } from "./AppTooltip.react";

describe("AppTooltip", () => {
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
  });

  afterEach(() => {
    vi.useRealTimers();
    document.querySelectorAll(".app-tooltip").forEach((el) => el.remove());
    document.body.innerHTML = "";
  });

  test("shows label and shortcut chips after delay on hover", () => {
    render(
      createElement(AppTooltip, {
        label: "Save workflow",
        shortcutId: "save",
        children: createElement(
          "button",
          { type: "button", "aria-label": "Save workflow" },
          "Save",
        ),
      }),
    );

    const trigger = document.querySelector("button")!;
    fireEvent.pointerEnter(trigger);
    expect(document.querySelector(".app-tooltip")).toBeNull();

    act(() => {
      vi.advanceTimersByTime(400);
    });
    const tip = document.querySelector(".app-tooltip");
    expect(tip).not.toBeNull();
    expect(tip?.textContent).toContain("Save workflow");
    expect(tip?.querySelectorAll(".app-tooltip-key").length).toBeGreaterThan(0);
  });

  test("hides on pointer leave", () => {
    render(
      createElement(AppTooltip, {
        label: "Inspector",
        children: createElement(
          "button",
          { type: "button", "aria-label": "Inspector" },
          "I",
        ),
      }),
    );
    const trigger = document.querySelector("button")!;
    fireEvent.pointerEnter(trigger);
    act(() => {
      vi.advanceTimersByTime(400);
    });
    fireEvent.pointerLeave(trigger);
    expect(document.querySelector(".app-tooltip")).toBeNull();
  });
});
