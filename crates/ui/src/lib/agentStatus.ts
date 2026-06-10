import type { AgentStatus } from "./types";

/** User-facing label for a workflow node agent status pill. */
export function labelForAgentStatus(status: AgentStatus): string {
  switch (status) {
    case "idle":
      return "Idle";
    case "queued":
      return "Queued";
    case "started":
      return "Thinking";
    case "awaiting_input":
      return "Waiting for Input";
    case "awaiting_tool_approval":
      return "Awaiting Approval";
    case "running_tool":
      return "Running Tool";
    case "completed":
      return "Done";
    case "failed":
      return "Failed";
    case "interrupted":
      return "Interrupted";
    case "stopped":
      return "Stopped";
    default:
      return "Idle";
  }
}
