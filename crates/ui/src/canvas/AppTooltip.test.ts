// @vitest-environment jsdom
import { createElement } from "react";
import { cleanup, fireEvent, render, act } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { AppTooltip } from "./AppTooltip.react";

afterEach(() => { cleanup(); vi.useRealTimers(); });

test.each(["Escape", "scroll", "resize"])("dismisses the canvas tooltip on %s", (action) => {
  vi.useFakeTimers();
  const { getByRole } = render(createElement(AppTooltip, {
    label: "Inspector",
    children: createElement("button", null, "Inspector"),
  }));
  fireEvent.focus(getByRole("button"));
  act(() => { vi.advanceTimersByTime(400); });
  expect(document.querySelector('[role="tooltip"]')).not.toBeNull();
  act(() => {
    window.dispatchEvent(action === "Escape" ? new KeyboardEvent("keydown", { key: "Escape" }) : new Event(action));
  });
  expect(document.querySelector('[role="tooltip"]')).toBeNull();
});
