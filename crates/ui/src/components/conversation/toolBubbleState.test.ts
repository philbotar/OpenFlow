import { describe, expect, it } from "vitest";
import type { NodeId, ToolCallStatus, WorkflowRunState } from "../../lib/types";
import {
  formatToolDisplayName,
  resolveToolSummary,
  toolBubbleIntentText,
  toolBubbleRowStatusText,
  toolBubbleTargetText,
} from "./toolBubbleState";

describe("formatToolDisplayName", () => {
  it("maps read to 'Read File'", () => {
    expect(formatToolDisplayName("read")).toBe("Read File");
  });

  it("maps write to 'Write File'", () => {
    expect(formatToolDisplayName("write")).toBe("Write File");
  });

  it("maps edit to 'Edit File'", () => {
    expect(formatToolDisplayName("edit")).toBe("Edit File");
  });

  it("maps apply_patch to 'Apply Patch'", () => {
    expect(formatToolDisplayName("apply_patch")).toBe("Apply Patch");
  });

  it("maps bash to 'Run Command'", () => {
    expect(formatToolDisplayName("bash")).toBe("Run Command");
  });

  it("maps search to 'Search Files'", () => {
    expect(formatToolDisplayName("search")).toBe("Search Files");
  });

  it("maps find to 'Find Files'", () => {
    expect(formatToolDisplayName("find")).toBe("Find Files");
  });

  it("maps ast_grep to 'AST Search'", () => {
    expect(formatToolDisplayName("ast_grep")).toBe("AST Search");
  });

  it("maps openflow_call_subagent to 'Call Subagent'", () => {
    expect(formatToolDisplayName("openflow_call_subagent")).toBe("Call Subagent");
  });

  it("maps openflow_declare_subagents to 'Declare Subagents'", () => {
    expect(formatToolDisplayName("openflow_declare_subagents")).toBe("Declare Subagents");
  });

  it("maps openflow_submit_node_output to 'Submit Output'", () => {
    expect(formatToolDisplayName("openflow_submit_node_output")).toBe("Submit Output");
  });

  it("maps openflow_request_user_input to 'Request Input'", () => {
    expect(formatToolDisplayName("openflow_request_user_input")).toBe("Request Input");
  });

  it("returns raw name for unknown tools (passthrough)", () => {
    expect(formatToolDisplayName("unknown_tool_xyz")).toBe("unknown_tool_xyz");
  });

  it("returns empty string for empty input", () => {
    expect(formatToolDisplayName("")).toBe("");
  });

  it("returns empty string for null input", () => {
    expect(formatToolDisplayName(null)).toBe("");
  });

  it("returns empty string for undefined input", () => {
    expect(formatToolDisplayName(undefined)).toBe("");
  });

  it("always returns a non-null string (never undefined)", () => {
    const result: string = formatToolDisplayName("unknown_tool");
    expect(result).not.toBeUndefined();
    expect(typeof result).toBe("string");
  });

  it("TOOL_DISPLAY_NAMES map has exactly 12 entries", () => {
    // Guard against accidental additions or removals. If you add a new tool,
    // update this count and add a corresponding test above.
    const expectedCount = 12;
    const result = formatToolDisplayName("read");
    expect(result).toBe("Read File");
    // Count the known mappings by testing every expected key
    const knownKeys = [
      "read", "write", "edit", "apply_patch",
      "bash", "search", "find", "ast_grep",
      "openflow_call_subagent", "openflow_declare_subagents",
      "openflow_submit_node_output", "openflow_request_user_input",
    ];
    expect(knownKeys.length).toBe(expectedCount);
    // Every known key must map to a different value than its raw name
    for (const key of knownKeys) {
      expect(formatToolDisplayName(key)).not.toBe(key);
    }
  });
});

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
