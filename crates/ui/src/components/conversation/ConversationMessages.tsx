import MessageCircle from "lucide-solid/icons/message-circle";
import { createMemo, For, Show } from "solid-js";
import { displayChatContent } from "../../lib/stripToolCallMarkup";
import { useAppContext } from "../../context/AppContext";
import type { ChatMessage } from "../../lib/types";
import {
  groupLegacyToolMessages,
  isLegacyToolGroup,
  isProviderThinkingMessage,
  type ConversationItem,
  type LegacyToolGroup,
} from "../../lib/parseLegacyToolMessages";
import { chatRoleToMessageFrom, messageLabel } from "./chatRole";
import {
  Conversation,
  ConversationContent,
  ConversationEmptyState,
  ConversationScrollButton,
} from "./Conversation";
import { Message } from "./Message";
import { NodeCompletedBubble } from "./NodeCompletedBubble";
import { ThinkingBubble } from "./ThinkingBubble";
import { ToolBubble } from "./ToolBubble";
import { resolveToolSummary } from "./toolBubbleState";

function parseLegacyArguments(argumentsText: string | null): unknown {
  if (!argumentsText?.trim()) return undefined;
  try {
    return JSON.parse(argumentsText) as unknown;
  } catch {
    return argumentsText;
  }
}

function LegacyToolBubble(props: { group: LegacyToolGroup }) {
  return (
    <ToolBubble
      toolName={props.group.toolName}
      status={props.group.status}
      output={props.group.output}
      arguments={parseLegacyArguments(props.group.argumentsText)}
      isError={props.group.isError}
    />
  );
}

function MarkerToolBubble(props: { message: ChatMessage }) {
  const ctx = useAppContext();
  const summary = () =>
    resolveToolSummary(
      ctx.selectedNodeId(),
      props.message.toolCallId!,
      ctx.runState(),
    );

  return (
    <ToolBubble
      toolName={summary()?.toolName ?? "Tool"}
      status={summary()?.status ?? "proposed"}
      output={summary()?.lastOutput}
      arguments={summary()?.arguments}
      isError={summary()?.isError}
    />
  );
}

function PlainMessage(props: { message: ChatMessage }) {
  const ctx = useAppContext();
  const content = createMemo(() =>
    displayChatContent(props.message.role, props.message.content),
  );
  return (
    <Show when={content().trim()}>
      <Message
        from={chatRoleToMessageFrom(props.message.role)}
        label={messageLabel(props.message.role, ctx.currentNode()?.label)}
        content={content()}
        streaming={props.message.streaming}
      />
    </Show>
  );
}

function ConversationItemView(props: { item: ConversationItem }) {
  if (isLegacyToolGroup(props.item)) {
    return <LegacyToolBubble group={props.item} />;
  }
  if (props.item.messageKind === "node_completed") {
    return <NodeCompletedBubble summary={props.item.content} />;
  }
  if (props.item.toolCallId) {
    return <MarkerToolBubble message={props.item} />;
  }
  if (isProviderThinkingMessage(props.item)) {
    return <ThinkingBubble message={props.item} />;
  }
  return <PlainMessage message={props.item} />;
}

export function ConversationMessages() {
  const ctx = useAppContext();
  const conversationItems = createMemo(() => groupLegacyToolMessages(ctx.chatMessages()));

  return (
    <Conversation>
      {(conversation) => (
        <>
          <ConversationContent conversation={conversation}>
            <Show
              when={ctx.chatMessages().length > 0}
              fallback={
                <ConversationEmptyState
                  icon={<MessageCircle class="conversation-empty-icon-svg" width={22} height={22} />}
                />
              }
            >
              <For each={conversationItems()}>
                {(item) => <ConversationItemView item={item} />}
              </For>
            </Show>
          </ConversationContent>
          <ConversationScrollButton conversation={conversation} />
        </>
      )}
    </Conversation>
  );
}
