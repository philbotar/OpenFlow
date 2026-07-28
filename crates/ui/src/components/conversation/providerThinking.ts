import type { ChatMessage } from "../../lib/types";

const LEGACY_TOOL_THINKING_PREFIXES = [
  "Tool request: ",
  "Running tool: ",
  "Tool result: ",
] as const;

/** Tool lifecycle prose from saved runs created before structured tool bubbles. */
export function isLegacyToolThinkingMessage(message: ChatMessage): boolean {
  if (message.role !== "thinking" && message.role !== "Thinking") {
    return false;
  }
  if (message.toolCallId) {
    return false;
  }
  return LEGACY_TOOL_THINKING_PREFIXES.some((prefix) =>
    message.content.startsWith(prefix),
  );
}

/** Provider reasoning — distinct from legacy tool I/O lines that reuse the thinking role. */
export function isProviderThinkingMessage(message: ChatMessage): boolean {
  if (message.role !== "thinking" && message.role !== "Thinking") {
    return false;
  }
  if (message.toolCallId) {
    return false;
  }
  return !isLegacyToolThinkingMessage(message);
}
