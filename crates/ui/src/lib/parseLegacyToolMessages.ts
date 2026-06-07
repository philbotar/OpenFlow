import type { ChatMessage, ToolCallStatus } from "./types";

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
    if (requestMatch && message.role === "Thinking") {
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
          isError = next.role === "System";
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
    if (runningMatch && message.role === "Thinking") {
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
        status: message.role === "System" ? "failed" : "completed",
        isError: message.role === "System",
      });
      index += 1;
      continue;
    }

    result.push(message);
    index += 1;
  }

  return result;
}
