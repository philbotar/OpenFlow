import type { ChatMessage } from "../../lib/types";
import { displayChatContent, extractThinkContent } from "../../lib/stripToolCallMarkup";
import { isProviderThinkingMessage } from "./providerThinking";

export type GroupedConversationItem =
  | { kind: "message"; message: ChatMessage }
  | { kind: "tool"; message: ChatMessage }
  | { kind: "toolStack"; messages: ChatMessage[] };

const DEFAULT_STACK_THRESHOLD = 2;

function isToolMarker(message: ChatMessage): boolean {
  return Boolean(message.toolCallId);
}

function isApprovalSystemLine(message: ChatMessage): boolean {
  if (message.role !== "system" && message.role !== "System") return false;
  return message.content.startsWith("Approval required for tool ");
}

/**
 * Turn assistant `<think>` markup into Thinking-role messages so UI uses ThinkingBubble.
 * One original assistant turn → at most one thinking bubble + optional prose message.
 */
export function expandThinkMessages(messages: ChatMessage[]): ChatMessage[] {
  const out: ChatMessage[] = [];
  for (const message of messages) {
    if (isToolMarker(message) || message.messageKind === "node_completed") {
      out.push(message);
      continue;
    }
    if (message.role !== "assistant" && message.role !== "Assistant") {
      out.push(message);
      continue;
    }

    const { thoughts, remainder } = extractThinkContent(message.content);
    if (!thoughts && !remainder) {
      if (message.streaming) {
        out.push(message);
      }
      continue;
    }
    if (thoughts) {
      const thinkingMessage: ChatMessage = {
        role: "Thinking",
        content: thoughts,
      };
      if (message.id) thinkingMessage.id = `${message.id}-think`;
      if (message.streaming && !remainder) thinkingMessage.streaming = true;
      out.push(thinkingMessage);
    }
    if (remainder) {
      const prose: ChatMessage = {
        role: message.role,
        content: remainder,
      };
      if (message.id) prose.id = message.id;
      if (message.streaming && !thoughts) prose.streaming = true;
      out.push(prose);
    }
  }
  return out;
}

/** True when ConversationItemView would render nothing useful for this message. */
function isInvisibleNonTool(message: ChatMessage): boolean {
  if (message.messageKind === "node_completed") return false;
  if (isToolMarker(message)) return false;
  if (isApprovalSystemLine(message)) return true;
  if (message.streaming) return false;
  if (isProviderThinkingMessage(message)) {
    return message.content.trim().length === 0;
  }
  return displayChatContent(message.role, message.content).trim().length === 0;
}

/** Thinking between tools — fold into the active run instead of splitting stacks. */
function isStackBridge(message: ChatMessage): boolean {
  if (message.messageKind === "node_completed") return false;
  if (isToolMarker(message)) return false;
  return isProviderThinkingMessage(message);
}

function toolCount(run: ChatMessage[]): number {
  return run.reduce((count, message) => count + (isToolMarker(message) ? 1 : 0), 0);
}

function flushToolRun(
  run: ChatMessage[],
  out: GroupedConversationItem[],
  threshold: number,
): void {
  if (run.length === 0) return;
  if (toolCount(run) >= threshold) {
    out.push({ kind: "toolStack", messages: [...run] });
  } else {
    for (const message of run) {
      if (isToolMarker(message)) {
        out.push({ kind: "tool", message });
      } else {
        out.push({ kind: "message", message });
      }
    }
  }
  run.length = 0;
}

/**
 * Collapse tool-marker runs into stacks when tool count >= threshold.
 * Assistant `<think>` becomes Thinking bubbles; thinking before/between tools
 * folds into the stack. Summary is tools + optional "Thought for a while"/"Thinking".
 *
 * When `prev` is provided, reuses prior `toolStack` object identities so Solid
 * `<For>` does not remount open stacks on every live append (avoids enter-animation stutter).
 */
export function groupToolMessages(
  messages: ChatMessage[],
  threshold: number = DEFAULT_STACK_THRESHOLD,
  prev: GroupedConversationItem[] | null = null,
): GroupedConversationItem[] {
  const out: GroupedConversationItem[] = [];
  const run: ChatMessage[] = [];
  const pendingThinking: ChatMessage[] = [];

  const flushPendingThinkingAsMessages = () => {
    for (const message of pendingThinking) {
      out.push({ kind: "message", message });
    }
    pendingThinking.length = 0;
  };

  for (const message of expandThinkMessages(messages)) {
    if (isToolMarker(message)) {
      if (pendingThinking.length > 0) {
        run.push(...pendingThinking);
        pendingThinking.length = 0;
      }
      run.push(message);
      continue;
    }
    if (isInvisibleNonTool(message)) {
      continue;
    }
    if (isStackBridge(message)) {
      if (run.length > 0) {
        run.push(message);
      } else {
        pendingThinking.push(message);
      }
      continue;
    }
    flushPendingThinkingAsMessages();
    flushToolRun(run, out, threshold);
    out.push({ kind: "message", message });
  }
  flushPendingThinkingAsMessages();
  flushToolRun(run, out, threshold);
  return prev ? reuseGroupedItemIdentities(prev, out) : out;
}

function stackPersistKey(item: Extract<GroupedConversationItem, { kind: "toolStack" }>): string {
  return item.messages.find((message) => message.toolCallId)?.toolCallId ?? "";
}

/** Keep prior toolStack object refs when the first toolCallId matches. */
function reuseGroupedItemIdentities(
  prev: GroupedConversationItem[],
  next: GroupedConversationItem[],
): GroupedConversationItem[] {
  const prevStacks = new Map<string, Extract<GroupedConversationItem, { kind: "toolStack" }>>();
  for (const item of prev) {
    if (item.kind === "toolStack") {
      const key = stackPersistKey(item);
      if (key) prevStacks.set(key, item);
    }
  }
  return next.map((item) => {
    if (item.kind !== "toolStack") return item;
    const key = stackPersistKey(item);
    const prior = key ? prevStacks.get(key) : undefined;
    if (!prior) return item;
    prior.messages = item.messages;
    return prior;
  });
}
