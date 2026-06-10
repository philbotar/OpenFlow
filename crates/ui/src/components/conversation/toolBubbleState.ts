import type { NodeId, ToolCallSummary, ToolCallStatus, WorkflowRunState } from "../../lib/types";

export function resolveToolSummary(
  nodeId: NodeId | null | undefined,
  toolCallId: string,
  runState: WorkflowRunState | null | undefined,
): ToolCallSummary | undefined {
  if (!nodeId || !runState) return undefined;
  return runState.toolCallsByNode[nodeId]?.find((call) => call.toolCallId === toolCallId);
}

/** Status label for the collapsed tool row — never includes tool output. */
export function toolBubbleRowStatusText(status: ToolCallStatus): string {
  switch (status) {
    case "proposed":
      return "Preparing…";
    case "awaiting_approval":
      return "Awaiting approval…";
    case "running":
      return "Running…";
    case "blocked":
      return "Tool blocked.";
    case "failed":
      return "Tool failed.";
    case "aborted":
      return "Tool aborted.";
    case "completed":
      return "";
    default:
      return "";
  }
}

/** Full output for the expanded panel — actual tool output when present. */
export function toolBubbleOutputText(
  status: ToolCallStatus,
  output: string | null | undefined,
  _args: unknown,
  _isError: boolean,
): string {
  if (output?.trim()) return output;
  return toolBubbleRowStatusText(status);
}
