import { describe, expect, it } from "vitest";
import type { NodeId, ToolCallStatus, WorkflowRunState } from "../../lib/types";
import { resolveToolSummary, toolBubbleOutputText } from "./toolBubbleState";

describe("toolBubbleOutputText", () => {
  it("returns 'Preparing…' for proposed status without argument preview", () => {
    expect(toolBubbleOutputText("proposed", null, { foo: "bar" }, false)).toBe("Preparing…");
  });

  it("returns 'Preparing…' for proposed status with null args", () => {
    expect(toolBubbleOutputText("proposed", null, null, false)).toBe("Preparing…");
  });

  it("returns 'Running…' for running status", () => {
    expect(toolBubbleOutputText("running", null, undefined, false)).toBe("Running…");
  });

  it("returns output text when output is non-empty", () => {
    expect(toolBubbleOutputText("completed", "some result", undefined, false)).toBe("some result");
  });

  it("returns empty string for completed status with no output", () => {
    expect(toolBubbleOutputText("completed", null, undefined, false)).toBe("");
  });

  it("returns 'Tool failed.' for failed status", () => {
    expect(toolBubbleOutputText("failed", null, undefined, true)).toBe("Tool failed.");
  });

  it("returns 'Tool aborted.' for aborted status", () => {
    expect(toolBubbleOutputText("aborted", null, undefined, true)).toBe("Tool aborted.");
  });

  it("returns 'Tool blocked.' for blocked status", () => {
    expect(toolBubbleOutputText("blocked", null, undefined, false)).toBe("Tool blocked.");
  });

  it("returns 'Awaiting approval…' for awaiting_approval status", () => {
    expect(toolBubbleOutputText("awaiting_approval", null, undefined, false)).toBe(
      "Awaiting approval…",
    );
  });

  it("prefers output over status text when output is present", () => {
    expect(toolBubbleOutputText("failed", "custom error", undefined, true)).toBe("custom error");
  });
});

describe("resolveToolSummary", () => {
  const runState: WorkflowRunState = {
    toolCallsByNode: {
      "node-1": [
        {
          toolCallId: "tc-1",
          toolName: "search",
          status: "completed",
          arguments: { query: "hello" },
          lastOutput: "results here",
          isError: false,
        },
        {
          toolCallId: "tc-2",
          toolName: "write_file",
          status: "failed",
          arguments: null,
          lastOutput: "permission denied",
          isError: true,
        },
      ],
    },
  } as unknown as WorkflowRunState;

  it("finds correct tool call by ID within a node", () => {
    const result = resolveToolSummary("node-1" as NodeId, "tc-1", runState);
    expect(result).toBeDefined();
    expect(result!.toolName).toBe("search");
    expect(result!.status).toBe("completed");
  });

  it("returns undefined for unknown node", () => {
    expect(resolveToolSummary("unknown-node" as NodeId, "tc-1", runState)).toBeUndefined();
  });

  it("returns undefined for unknown tool call ID", () => {
    expect(resolveToolSummary("node-1" as NodeId, "tc-999", runState)).toBeUndefined();
  });

  it("returns undefined when nodeId is null", () => {
    expect(resolveToolSummary(null, "tc-1", runState)).toBeUndefined();
  });

  it("returns undefined when runState is null", () => {
    expect(resolveToolSummary("node-1" as NodeId, "tc-1", null)).toBeUndefined();
  });
});
