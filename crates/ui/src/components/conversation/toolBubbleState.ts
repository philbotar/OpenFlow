import type { NodeId, ToolCallSummary, ToolCallStatus, WorkflowRunState } from "../../lib/types";

export function resolveToolSummary(
  nodeId: NodeId | null | undefined,
  toolCallId: string,
  runState: WorkflowRunState | null | undefined,
): ToolCallSummary | undefined {
  if (!nodeId || !runState) return undefined;
  return runState.toolCallsByNode[nodeId]?.find((call) => call.toolCallId === toolCallId);
}

function argString(args: unknown, key: string): string | undefined {
  if (!args || typeof args !== "object" || Array.isArray(args)) return undefined;
  const value = (args as Record<string, unknown>)[key];
  if (typeof value === "string" && value.trim()) return value.trim();
  return undefined;
}

function argStringOrArray(args: unknown, key: string): string | undefined {
  if (!args || typeof args !== "object" || Array.isArray(args)) return undefined;
  const value = (args as Record<string, unknown>)[key];
  if (typeof value === "string" && value.trim()) return value.trim();
  if (Array.isArray(value)) {
    const parts = value
      .filter((entry): entry is string => typeof entry === "string" && entry.trim().length > 0)
      .map((entry) => entry.trim());
    if (parts.length > 0) return parts.join(", ");
  }
  return undefined;
}

function truncate(text: string, max = 80): string {
  if (text.length <= max) return text;
  return `${text.slice(0, max - 1)}…`;
}

function editInputHint(args: unknown): string | undefined {
  const input = argString(args, "input");
  if (!input) return undefined;
  const firstLine = input.split("\n")[0]?.trim() ?? "";
  const pathMatch = /^¶([^#]+)/.exec(firstLine);
  if (pathMatch?.[1]) return pathMatch[1].trim();
  return truncate(firstLine);
}

function patchFileHint(args: unknown): string | undefined {
  const input = argString(args, "input");
  if (!input) return undefined;
  const match = /^\*\*\* (?:Update|Add|Delete) File:\s*(.+)$/m.exec(input);
  return match?.[1]?.trim();
}

/** File path, search pattern, or other invocation target — never tool output. */
export function toolBubbleTargetText(toolName: string, args: unknown): string {
  switch (toolName) {
    case "read":
    case "write":
      return argString(args, "path") ?? "";
    case "search": {
      const pattern = argString(args, "pattern");
      const paths = argStringOrArray(args, "paths");
      if (pattern && paths) return `${pattern} in ${paths}`;
      return pattern ?? paths ?? "";
    }
    case "find":
      return argStringOrArray(args, "paths") ?? "";
    case "bash":
      return truncate(argString(args, "command") ?? "");
    case "ast_grep": {
      const pat = argString(args, "pat");
      const paths = argStringOrArray(args, "paths");
      if (pat && paths) return `${pat} in ${paths}`;
      return pat ?? paths ?? "";
    }
    case "edit":
      return argString(args, "path") ?? editInputHint(args) ?? "";
    case "apply_patch":
      return patchFileHint(args) ?? "";
    case "openflow_call_subagent":
      return argString(args, "subagent_id") ?? "";
    default:
      return (
        argString(args, "path") ??
        argString(args, "pattern") ??
        argString(args, "command") ??
        argString(args, "subagent_id") ??
        ""
      );
  }
}

/** Status label for the collapsed tool row when no target is available yet. */
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

