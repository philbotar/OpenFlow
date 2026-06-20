import { describe, expect, test } from "vitest";
import { executionCwdForRun } from "../executionCwd";

describe("executionCwd helpers", () => {
  test("executionCwdForRun maps blank to null", () => {
    expect(executionCwdForRun("")).toBeNull();
    expect(executionCwdForRun(" /tmp/repo ")).toBe("/tmp/repo");
  });
});
