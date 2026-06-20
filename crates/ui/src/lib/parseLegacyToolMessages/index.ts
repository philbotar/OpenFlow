import type { ChatMessage, ToolCallStatus } from "../types";

export interface LegacyToolGroup {
  toolName: string;
  argumentsText: string | null;
  output: string | null;
  status: ToolCallStatus;
  isError: boolean;
}

export type ConversationItem = ChatMessage | LegacyToolGroup;

export function isLegacyToolGroup(item: ConversationItem): item is LegacyToolGroup {
  return "toolName" in item;
}

function isThinkingRole(role: ChatMessage["role"]): boolean {
  return role === "thinking" || role === "Thinking";
}

/** Provider reasoning — distinct from legacy tool I/O lines that reuse the thinking role. */
export function isProviderThinkingMessage(message: ChatMessage): boolean {
  if (!isThinkingRole(message.role) || message.toolCallId) {
    return false;
  }
  if (message.content.match(/^Tool request: /)) return false;
  if (message.content.match(/^Running tool: /)) return false;
  if (message.content.match(/^Tool result: /)) return false;
  return true;
}

function isSystemRole(role: ChatMessage["role"]): boolean {
  return role === "system" || role === "System";
}

export function groupLegacyToolMessages(messages: ChatMessage[]): ConversationItem[] {
  const result: ConversationItem[] = [];
  let index = 0;

  while (index < messages.length) {
    const message = messages[index];

    if (message.toolCallId) {
      result.push(message);
      index += 1;
      continue;
    }

    const requestMatch = message.content.match(/^Tool request: ([^\n]+)/);
    if (requestMatch && isThinkingRole(message.role)) {
      const toolName = requestMatch[1].trim();
      const argsMarker = "\nArguments:\n";
      const argsIndex = message.content.indexOf(argsMarker);
      const argumentsText =
        argsIndex >= 0 ? message.content.slice(argsIndex + argsMarker.length) : null;

      let output: string | null = null;
      let isError = false;
      let cursor = index + 1;

      while (cursor < messages.length) {
        const next = messages[cursor];
        if (next.toolCallId) break;

        const runningMatch = next.content.match(/^Running tool: ([^\n]+)$/);
        if (runningMatch && runningMatch[1].trim() === toolName) {
          cursor += 1;
          continue;
        }

        const resultMatch = next.content.match(/^Tool result: ([^\n]+)\n([\s\S]*)$/);
        if (resultMatch && resultMatch[1].trim() === toolName) {
          output = resultMatch[2];
          isError = isSystemRole(next.role);
          cursor += 1;
          break;
        }

        break;
      }

      const status: ToolCallStatus =
        output === null ? "running" : isError ? "failed" : "completed";

      result.push({ toolName, argumentsText, output, status, isError });
      index = cursor;
      continue;
    }

    const runningMatch = message.content.match(/^Running tool: ([^\n]+)$/);
    if (runningMatch && isThinkingRole(message.role)) {
      result.push({
        toolName: runningMatch[1].trim(),
        argumentsText: null,
        output: null,
        status: "running",
        isError: false,
      });
      index += 1;
      continue;
    }

    const resultMatch = message.content.match(/^Tool result: ([^\n]+)\n([\s\S]*)$/);
    if (resultMatch) {
      result.push({
        toolName: resultMatch[1].trim(),
        argumentsText: null,
        output: resultMatch[2],
        status: isSystemRole(message.role) ? "failed" : "completed",
        isError: isSystemRole(message.role),
      });
      index += 1;
      continue;
    }

    result.push(message);
    index += 1;
  }

  return result;
}
