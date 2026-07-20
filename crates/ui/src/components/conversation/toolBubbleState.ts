import type { ChatMessage, NodeId, ToolCallSummary, ToolCallStatus, WorkflowRunState } from "../../lib/types";
import { relativizeDisplayPath } from "../../lib/relativizePath";
import { isProviderThinkingMessage } from "./providerThinking";

const TOOL_VERBS: Record<string, { active: string; done: string }> = {
  read: { active: "Reading", done: "Read" },
  write: { active: "Writing", done: "Wrote" },
  edit: { active: "Editing", done: "Edited" },
  apply_patch: { active: "Patching", done: "Patched" },
  bash: { active: "Running", done: "Ran" },
  search: { active: "Grepping", done: "Grepped" },
  find: { active: "Searching", done: "Searched" },
  ast_grep: { active: "Searching", done: "Searched" },
  web_search: { active: "Searching web", done: "Searched web" },
  openflow_write_plan_artifact: { active: "Sealing plan", done: "Sealed plan" },
  openflow_call_subagent: { active: "Calling subagent", done: "Called subagent" },
  openflow_declare_subagents: { active: "Declaring subagents", done: "Declared subagents" },
  openflow_submit_node_output: { active: "Submitting output", done: "Submitted output" },
  openflow_request_user_input: { active: "Requesting input", done: "Requested input" },
};

const TOOL_DISPLAY_NAMES: Record<string, string> = {
  read: "Read File",
  write: "Write File",
  edit: "Edit File",
  apply_patch: "Apply Patch",
  bash: "Run Command",
  search: "Search Files",
  find: "Search Folders",
  ast_grep: "AST Search",
  openflow_call_subagent: "Call Subagent",
  openflow_declare_subagents: "Declare Subagents",
  openflow_submit_node_output: "Submit Output",
  openflow_request_user_input: "Request Input",
  openflow_write_plan_artifact: "Freeze Plan Artifact",
};

const TOOL_STACK_NOUNS: Record<string, { one: string; many: string }> = {
  read: { one: "file", many: "files" },
  write: { one: "file", many: "files" },
  edit: { one: "file", many: "files" },
  apply_patch: { one: "file", many: "files" },
  bash: { one: "command", many: "commands" },
  search: { one: "pattern", many: "patterns" },
  find: { one: "folder", many: "folders" },
  ast_grep: { one: "pattern", many: "patterns" },
  web_search: { one: "result", many: "results" },
  openflow_call_subagent: { one: "subagent", many: "subagents" },
  openflow_declare_subagents: { one: "declaration", many: "declarations" },
  openflow_submit_node_output: { one: "output", many: "outputs" },
  openflow_request_user_input: { one: "request", many: "requests" },
  openflow_write_plan_artifact: { one: "plan", many: "plans" },
};

const ACTIVE_TOOL_STATUSES: ReadonlySet<ToolCallStatus> = new Set([
  "proposed",
  "running",
  "awaiting_approval",
]);

function toolStackNoun(toolName: string, count: number): string {
  const nouns = TOOL_STACK_NOUNS[toolName] ?? { one: "call", many: "calls" };
  return count === 1 ? nouns.one : nouns.many;
}

function toolVerb(toolName: string, active: boolean): string {
  const verbs = TOOL_VERBS[toolName];
  if (verbs) {
    return active ? verbs.active : verbs.done;
  }
  return active ? `Running ${toolName}` : `Ran ${toolName}`;
}

function toolBubbleVerb(toolName: string, status: ToolCallStatus): string {
  return toolVerb(toolName, ACTIVE_TOOL_STATUSES.has(status));
}

function toolStackFamilyVerb(toolName: string, active: boolean): string {
  return toolVerb(toolName, active);
}

/** Collapsed stack label: `Read 2 files · Grepped 3 patterns`. */
export function toolStackSummaryText(
  entries: ReadonlyArray<{ toolName: string; status: ToolCallStatus }>,
): string {
  const order: string[] = [];
  const counts = new Map<string, number>();
  const activeByFamily = new Map<string, boolean>();

  for (const entry of entries) {
    const name = entry.toolName || "Tool";
    if (!counts.has(name)) {
      order.push(name);
      counts.set(name, 0);
      activeByFamily.set(name, false);
    }
    counts.set(name, (counts.get(name) ?? 0) + 1);
    if (ACTIVE_TOOL_STATUSES.has(entry.status)) {
      activeByFamily.set(name, true);
    }
  }

  return order
    .map((name) => {
      const count = counts.get(name) ?? 0;
      const verb = toolStackFamilyVerb(name, activeByFamily.get(name) === true);
      return `${verb} ${count} ${toolStackNoun(name, count)}`;
    })
    .join(" · ");
}

/** One label if the stack contains thinking — no counts. */
export function toolStackThinkingLabel(
  messages: ReadonlyArray<Pick<ChatMessage, "role" | "content" | "toolCallId" | "streaming">>,
): string | null {
  let any = false;
  let streaming = false;
  for (const message of messages) {
    if (!isProviderThinkingMessage(message as ChatMessage)) continue;
    any = true;
    if (message.streaming) streaming = true;
  }
  if (!any) return null;
  return streaming ? "Thinking" : "Thought for a while";
}

/** Tool family summary plus optional thinking substring. */
export function toolStackSummaryWithThinking(
  entries: ReadonlyArray<{ toolName: string; status: ToolCallStatus }>,
  messages: ReadonlyArray<Pick<ChatMessage, "role" | "content" | "toolCallId" | "streaming">>,
): string {
  const tools = toolStackSummaryText(entries);
  const thinking = toolStackThinkingLabel(messages);
  if (tools && thinking) return `${tools} · ${thinking}`;
  return tools || thinking || "";
}

/**
 * Map a raw tool name to a human-friendly display name.
 *
 * Known tools are looked up in `TOOL_DISPLAY_NAMES`; unknown tools are
 * returned unchanged so that third-party / future tools degrade gracefully.
 */
export function formatToolDisplayName(toolName: string | undefined | null): string {
  if (toolName == null) return "";
  return TOOL_DISPLAY_NAMES[toolName] ?? toolName;
}

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

function argStringOrArray(
  args: unknown,
  key: string,
  cwd?: string | null,
): string | undefined {
  if (!args || typeof args !== "object" || Array.isArray(args)) return undefined;
  const value = (args as Record<string, unknown>)[key];
  if (typeof value === "string" && value.trim()) {
    return relativizeDisplayPath(value.trim(), cwd);
  }
  if (Array.isArray(value)) {
    const parts = value
      .filter((entry): entry is string => typeof entry === "string" && entry.trim().length > 0)
      .map((entry) => relativizeDisplayPath(entry.trim(), cwd));
    if (parts.length > 0) return parts.join(", ");
  }
  return undefined;
}

function displayPath(path: string | undefined, cwd?: string | null): string | undefined {
  if (!path) return undefined;
  return relativizeDisplayPath(path, cwd);
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

/** Human-readable intent from the tool call, when present. */
export function toolBubbleIntentText(summary: Pick<ToolCallSummary, "intent" | "arguments">): string {
  if (typeof summary.intent === "string" && summary.intent.trim()) {
    return summary.intent.trim();
  }
  if (summary.arguments && typeof summary.arguments === "object" && !Array.isArray(summary.arguments)) {
    const raw = (summary.arguments as Record<string, unknown>)["_i"];
    if (typeof raw === "string" && raw.trim()) return raw.trim();
  }
  return "";
}

/** File path, search pattern, or other invocation target — never tool output. */
export function toolBubbleTargetText(
  toolName: string,
  args: unknown,
  cwd?: string | null,
): string {
  switch (toolName) {
    case "read":
    case "write":
      return displayPath(argString(args, "path"), cwd) ?? "";
    case "search": {
      const pattern = argString(args, "pattern");
      const paths = argStringOrArray(args, "paths", cwd);
      if (pattern && paths) return `${pattern} in ${paths}`;
      return pattern ?? paths ?? "";
    }
    case "find":
      return argStringOrArray(args, "paths", cwd) ?? "";
    case "bash":
      return truncate(argString(args, "command") ?? "");
    case "ast_grep": {
      const pat = argString(args, "pat");
      const paths = argStringOrArray(args, "paths", cwd);
      if (pat && paths) return `${pat} in ${paths}`;
      return pat ?? paths ?? "";
    }
    case "edit":
      return (
        displayPath(argString(args, "path"), cwd) ??
        displayPath(editInputHint(args), cwd) ??
        ""
      );
    case "apply_patch":
      return displayPath(patchFileHint(args), cwd) ?? "";
    case "openflow_call_subagent":
      return argString(args, "subagent_id") ?? "";
    default:
      return (
        displayPath(argString(args, "path"), cwd) ??
        argString(args, "pattern") ??
        argString(args, "command") ??
        argString(args, "subagent_id") ??
        ""
      );
  }
}

function toolBubbleFailureSuffix(status: ToolCallStatus): string {
  switch (status) {
    case "failed":
      return " failed";
    case "aborted":
      return " aborted";
    case "blocked":
      return " blocked";
    default:
      return "";
  }
}

/** Single-line chat label: verb + target, tense follows tool status. */
export function toolBubbleLineText(
  toolName: string,
  status: ToolCallStatus,
  args: unknown,
  intent?: string | null,
  cwd?: string | null,
): string {
  const target =
    (typeof intent === "string" && intent.trim()) ||
    toolBubbleTargetText(toolName, args, cwd);
  let line = toolBubbleVerb(toolName, status);
  line += toolBubbleFailureSuffix(status);
  if (target) {
    line += ` ${target}`;
  } else if (status === "proposed") {
    line += "…";
  } else if (status === "awaiting_approval") {
    line += " (awaiting approval)";
  }
  return line;
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
