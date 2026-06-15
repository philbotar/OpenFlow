import { describe, expect, it } from "vitest";
import type { NodeId, ToolCallStatus, WorkflowRunState } from "../../lib/types";
import {
  resolveToolSummary,
  toolBubbleIntentText,
  toolBubbleRowStatusText,
  toolBubbleTargetText,
} from "./toolBubbleState";

describe("toolBubbleIntentText", () => {
  it("prefers projected tool intent", () => {
    expect(
      toolBubbleIntentText({
        intent: "inspect config",
        arguments: { path: "config.toml", _i: "read file" },
      }),
    ).toBe("inspect config");
  });

  it("falls back to _i from arguments", () => {
    expect(
      toolBubbleIntentText({
        intent: null,
        arguments: { path: "config.toml", _i: "read file" },
      }),
    ).toBe("read file");
  });
});

describe("toolBubbleTargetText", () => {
  it("extracts file path for read", () => {
    expect(toolBubbleTargetText("read", { path: "crates/ui/src/App.tsx" })).toBe(
      "crates/ui/src/App.tsx",
    );
  });

  it("extracts search pattern and paths", () => {
    expect(
      toolBubbleTargetText("search", { pattern: "TODO", paths: "crates/ui" }),
    ).toBe("TODO in crates/ui");
  });

  it("extracts bash command", () => {
    expect(toolBubbleTargetText("bash", { command: "cargo test" })).toBe("cargo test");
  });

  it("returns empty when args are missing", () => {
    expect(toolBubbleTargetText("read", null)).toBe("");
  });
});

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
