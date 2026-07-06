import { createRoot, createSignal } from "solid-js";
import { describe, expect, test, vi } from "vitest";
import { createDebounced, toastMessageForDebugMode } from "./index";

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
