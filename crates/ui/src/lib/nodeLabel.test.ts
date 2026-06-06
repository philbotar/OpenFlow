import { describe, expect, test } from "vitest";
import { resolveCommittedNodeLabel } from "./nodeLabel";

describe("resolveCommittedNodeLabel", () => {
  test("trims and applies a non-empty draft", () => {
    expect(resolveCommittedNodeLabel("Idea", "  Better idea  ")).toBe("Better idea");
  });

  test("keeps the current label when the draft is blank", () => {
    expect(resolveCommittedNodeLabel("Idea", "   ")).toBe("Idea");
  });
});
