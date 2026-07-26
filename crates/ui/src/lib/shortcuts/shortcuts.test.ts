// @vitest-environment jsdom
import { describe, expect, test } from "vitest";
import { formatShortcutParts, eventMatchesShortcut } from "./index";

describe("formatShortcutParts", () => {
  test("formats save on Mac", () => {
    expect(formatShortcutParts("save", true)).toEqual(["⌘", "S"]);
  });

  test("formats save on non-Mac", () => {
    expect(formatShortcutParts("save", false)).toEqual(["Ctrl", "S"]);
  });

  test("formats run Enter", () => {
    expect(formatShortcutParts("run", true)).toEqual(["⌘", "↵"]);
  });

  test("formats toggleChatFocus with Shift", () => {
    expect(formatShortcutParts("toggleChatFocus", true)).toEqual(["⌘", "⇧", "F"]);
  });
});

describe("eventMatchesShortcut", () => {
  test("matches Mod+I for toggleInspector", () => {
    const event = new KeyboardEvent("keydown", { key: "i", metaKey: true });
    expect(eventMatchesShortcut(event, "toggleInspector")).toBe(true);
  });

  test("requires Shift for toggleChatFocus", () => {
    const withShift = new KeyboardEvent("keydown", {
      key: "f",
      metaKey: true,
      shiftKey: true,
    });
    const without = new KeyboardEvent("keydown", { key: "f", metaKey: true });
    expect(eventMatchesShortcut(withShift, "toggleChatFocus")).toBe(true);
    expect(eventMatchesShortcut(without, "toggleChatFocus")).toBe(false);
  });

  test("matches zoom shortcuts when the symbol requires Shift", () => {
    const zoomIn = new KeyboardEvent("keydown", {
      key: "+",
      metaKey: true,
      shiftKey: true,
    });
    const zoomOut = new KeyboardEvent("keydown", {
      key: "_",
      metaKey: true,
      shiftKey: true,
    });

    expect(eventMatchesShortcut(zoomIn, "zoomIn")).toBe(true);
    expect(eventMatchesShortcut(zoomOut, "zoomOut")).toBe(true);
  });
});
