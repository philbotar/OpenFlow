import type { AgentStatus } from "./types";

const AGENT_STATUS_LABELS: Record<AgentStatus, string> = {
  idle: "Idle",
  queued: "Queued",
  started: "Thinking",
  awaiting_input: "Waiting for Input",
  awaiting_tool_approval: "Awaiting Approval",
  running_tool: "Running Tool",
  completed: "Done",
  failed: "Failed",
  interrupted: "Interrupted",
  stopped: "Stopped",
};

/** User-facing label for a workflow node agent status pill. */
export function labelForAgentStatus(status: AgentStatus): string {
  return AGENT_STATUS_LABELS[status] ?? "Idle";
}
