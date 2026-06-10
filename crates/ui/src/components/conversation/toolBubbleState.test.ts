import { describe, expect, it } from "vitest";
import type { NodeId, ToolCallStatus, WorkflowRunState } from "../../lib/types";
import {
  resolveToolSummary,
  toolBubbleOutputText,
  toolBubbleRowStatusText,
} from "./toolBubbleState";

describe("toolBubbleRowStatusText", () => {
  it("returns status labels without tool output", () => {
    expect(toolBubbleRowStatusText("proposed")).toBe("Preparing…");
    expect(toolBubbleRowStatusText("running")).toBe("Running…");
    expect(toolBubbleRowStatusText("awaiting_approval")).toBe("Awaiting approval…");
    expect(toolBubbleRowStatusText("blocked")).toBe("Tool blocked.");
    expect(toolBubbleRowStatusText("failed")).toBe("Tool failed.");
    expect(toolBubbleRowStatusText("aborted")).toBe("Tool aborted.");
    expect(toolBubbleRowStatusText("completed")).toBe("");
  });
});

describe("toolBubbleOutputText", () => {
  it("returns output text when output is non-empty", () => {
    expect(toolBubbleOutputText("completed", "some result", undefined, false)).toBe("some result");
  });

  it("falls back to row status when output is empty", () => {
    expect(toolBubbleOutputText("proposed", null, { foo: "bar" }, false)).toBe("Preparing…");
    expect(toolBubbleOutputText("running", null, undefined, false)).toBe("Running…");
    expect(toolBubbleOutputText("completed", null, undefined, false)).toBe("");
    expect(toolBubbleOutputText("failed", null, undefined, true)).toBe("Tool failed.");
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
