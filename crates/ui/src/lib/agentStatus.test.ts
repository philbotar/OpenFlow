import { describe, expect, it } from "vitest";
import { labelForAgentStatus } from "./agentStatus";
import type { AgentStatus } from "./types";

describe("labelForAgentStatus", () => {
  const cases: [AgentStatus, string][] = [
    ["idle", "Idle"],
    ["queued", "Queued"],
    ["started", "Thinking"],
    ["awaiting_input", "Waiting for Input"],
    ["awaiting_tool_approval", "Awaiting Approval"],
    ["running_tool", "Running Tool"],
    ["completed", "Done"],
    ["failed", "Failed"],
    ["stopped", "Stopped"],
  ];

  it.each(cases)("maps %s to %s", (status, label) => {
    expect(labelForAgentStatus(status)).toBe(label);
  });
});
