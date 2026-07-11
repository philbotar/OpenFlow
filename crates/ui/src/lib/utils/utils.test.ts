import { createRoot, createSignal } from "solid-js";
import { describe, expect, test, vi } from "vitest";
import {
  COMPOSER_INPUT_MAX_ROWS,
  clampDockHeight,
  createDebounced,
  formatIndentedValue,
  layoutViewportHeight,
  prettyJsonText,
  resizeComposerTextarea,
  restoredChatDockHeight,
  toastMessageForDebugMode,
} from "./index";

describe("prettyJsonText", () => {
  test("pretty-prints compact JSON objects", () => {
    expect(prettyJsonText('{"ok":true,"n":1}')).toBe('{\n  "ok": true,\n  "n": 1\n}');
  });

  test("leaves non-JSON text unchanged", () => {
    expect(prettyJsonText("Captured the welcome message.")).toBe(
      "Captured the welcome message.",
    );
  });
});

describe("formatIndentedValue", () => {
  test("formats JSON without brackets", () => {
    expect(formatIndentedValue('{"summary":"Done.","ok":true}')).toBe(
      "summary: Done.\nok: true",
    );
  });

  test("nests objects and lists by indentation", () => {
    expect(
      formatIndentedValue('{"items":["a","b"],"meta":{"n":1}}'),
    ).toBe("items:\n  - a\n  - b\nmeta:\n  n: 1");
  });

  test("leaves already-indented text unchanged", () => {
    expect(formatIndentedValue("summary: Done.")).toBe("summary: Done.");
  });
});

describe("dock height vs ui zoom", () => {
  test("layoutViewportHeight expands CSS space when zoomed out", () => {
    expect(layoutViewportHeight(1000, 0.7)).toBeCloseTo(1000 / 0.7);
    expect(layoutViewportHeight(1000, 1)).toBe(1000);
  });

  test("clampDockHeight raises max when zoomed out", () => {
    const atZoom1 = clampDockHeight(10_000, "chat", 1000, false, 1);
    const atZoom07 = clampDockHeight(10_000, "chat", 1000, false, 0.7);
    expect(atZoom1).toBe(840);
    expect(atZoom07).toBe(Math.round(1000 / 0.7) - 160);
    expect(atZoom07).toBeGreaterThan(atZoom1);
  });

  test("restoredChatDockHeight scales with zoom", () => {
    expect(restoredChatDockHeight(1000, false, 1)).toBe(750);
    expect(restoredChatDockHeight(1000, false, 0.7)).toBe(
      Math.round((1000 / 0.7) * 0.75),
    );
  });
});

describe("toastMessageForDebugMode", () => {
  test("preserves raw detail when debug output is enabled", () => {
    const message =
      "Could not reach Amazon Bedrock. Raw AWS SDK error: dispatch failure: connector error. Check AWS region.";

    expect(toastMessageForDebugMode(message, true)).toBe(message);
  });

  test("removes raw detail when debug output is disabled", () => {
    const message =
      "Could not reach Amazon Bedrock. Raw AWS SDK error: dispatch failure: connector error. Check AWS region.";

    expect(toastMessageForDebugMode(message, false)).toBe(
      "Could not reach Amazon Bedrock. Check AWS region.",
    );
  });
});

test("resizeComposerTextarea caps growth at four rows", () => {
  const textarea = document.createElement("textarea");
  textarea.style.paddingTop = "0px";
  textarea.style.paddingBottom = "0px";
  textarea.style.minHeight = "36px";
  textarea.style.lineHeight = "20px";
  textarea.placeholder =
    "Reply to workflow... Type / for skills or @ for files and folders.";
  document.body.appendChild(textarea);

  Object.defineProperty(textarea, "scrollHeight", {
    configurable: true,
    get() {
      if (textarea.value.length === 0) {
        return 80;
      }
      const lines = Math.max(1, Math.ceil(textarea.value.length / 20));
      return lines * 20;
    },
  });

  resizeComposerTextarea(textarea);
  expect(textarea.style.height).toBe("");
  expect(textarea.style.overflowY).toBe("hidden");

  textarea.value = "x".repeat(20);
  resizeComposerTextarea(textarea);
  expect(textarea.style.height).toBe("36px");
  expect(textarea.style.overflowY).toBe("hidden");

  textarea.value = "x".repeat(20 * COMPOSER_INPUT_MAX_ROWS);
  resizeComposerTextarea(textarea);
  expect(textarea.style.height).toBe("80px");
  expect(textarea.style.overflowY).toBe("hidden");

  textarea.value = "x".repeat(20 * (COMPOSER_INPUT_MAX_ROWS + 2));
  resizeComposerTextarea(textarea);
  expect(textarea.style.height).toBe("80px");
  expect(textarea.style.overflowY).toBe("auto");

  textarea.remove();
});

test("createDebounced trails the source by the delay", () => {
  vi.useFakeTimers();
  try {
    let debounced!: () => string;
    let setSource!: (value: string) => void;
    const dispose = createRoot((d) => {
      const [source, set] = createSignal("a");
      setSource = set;
      debounced = createDebounced(source, 150);
      return d;
    });
    expect(debounced()).toBe("a");
    setSource("ab");
    expect(debounced()).toBe("a");
    vi.advanceTimersByTime(150);
    expect(debounced()).toBe("ab");
    dispose();
  } finally {
    vi.useRealTimers();
  }
});
