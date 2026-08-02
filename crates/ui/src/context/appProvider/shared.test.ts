import { describe, expect, it } from "vitest";
import { acceptsRunStateUpdate } from "./shared";

describe("acceptsRunStateUpdate", () => {
  it("keeps the active run authoritative over stale events from another run", () => {
    const current = { active: true, runId: "new-run" };

    expect(acceptsRunStateUpdate(current, { active: false, runId: "old-run" })).toBe(false);
    expect(acceptsRunStateUpdate(current, { active: true, runId: "old-run" })).toBe(false);
  });

  it("accepts updates for the same run and replacement of an inactive run", () => {
    expect(
      acceptsRunStateUpdate(
        { active: true, runId: "run-1" },
        { active: false, runId: "run-1" },
      ),
    ).toBe(true);
    expect(
      acceptsRunStateUpdate(
        { active: false, runId: "run-1" },
        { active: true, runId: "run-2" },
      ),
    ).toBe(true);
  });
});
