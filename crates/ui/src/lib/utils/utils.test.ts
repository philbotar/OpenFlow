import { createRoot, createSignal } from "solid-js";
import { describe, expect, test, vi } from "vitest";
import { COMPOSER_INPUT_MAX_ROWS, createDebounced, resizeComposerTextarea, toastMessageForDebugMode } from "./index";

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
