import type { ChatMessage } from "../../lib/types";

/** Provider reasoning — distinct from legacy tool I/O lines that reuse the thinking role. */
export function isProviderThinkingMessage(message: ChatMessage): boolean {
  if (message.role !== "thinking" && message.role !== "Thinking") {
    return false;
  }
  if (message.toolCallId) {
    return false;
  }
  if (message.content.match(/^Tool request: /)) return false;
  if (message.content.match(/^Running tool: /)) return false;
  if (message.content.match(/^Tool result: /)) return false;
  return true;
}
