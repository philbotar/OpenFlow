import { describe, expect, it } from "vitest";
import {
  completedDurationMs,
  formatDuration,
  formatMessageTimestamp,
  isoMessageTimestamp,
} from "./timing";

describe("conversation timing", () => {
  it("formats short and long elapsed durations compactly", () => {
    expect(formatDuration(420)).toBe("420ms");
    expect(formatDuration(5_250)).toBe("5.3s");
    expect(formatDuration(95_000)).toBe("1m 35s");
    expect(formatDuration(7_500_000)).toBe("2h 5m");
  });

  it("rejects incomplete or reversed timing ranges", () => {
    expect(completedDurationMs(undefined, 20)).toBeNull();
    expect(completedDurationMs(20, undefined)).toBeNull();
    expect(completedDurationMs(20, 10)).toBeNull();
    expect(completedDurationMs(20, 45)).toBe(25);
  });

  it("formats a visible local date and time with an exact datetime value", () => {
    const timestampMs = Date.UTC(2026, 6, 30, 3, 4, 5);
    expect(formatMessageTimestamp(timestampMs)).toContain("2026");
    expect(isoMessageTimestamp(timestampMs)).toBe("2026-07-30T03:04:05.000Z");
  });
});
