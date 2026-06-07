import type { NodeId, ToolCallSummary, ToolCallStatus, WorkflowRunState } from "../../lib/types";
import { prettyJson } from "../../lib/workflow";

export function resolveToolSummary(
  nodeId: NodeId | null | undefined,
  toolCallId: string,
  runState: WorkflowRunState | null | undefined,
): ToolCallSummary | undefined {
  if (!nodeId || !runState) return undefined;
  return runState.toolCallsByNode[nodeId]?.find((call) => call.toolCallId === toolCallId);
}

export function toolBubbleOutputText(
  status: ToolCallStatus,
  output: string | null | undefined,
  args: unknown,
  isError: boolean,
): string {
  if (output?.trim()) return output;

  switch (status) {
    case "proposed":
      return formatArgumentsPreview(args);
    case "awaiting_approval":
      return "Awaiting approval…";
    case "running":
      return "Running…";
    case "blocked":
      return "Tool blocked.";
    case "failed":
      return isError ? "Tool failed." : "Tool failed.";
    case "completed":
      return "";
    default:
      return "";
  }
}

function formatArgumentsPreview(args: unknown): string {
  if (args === undefined || args === null) return "Preparing tool call…";
  const json = prettyJson(args).trim();
  return json ? `Arguments:\n${json}` : "Preparing tool call…";
}
