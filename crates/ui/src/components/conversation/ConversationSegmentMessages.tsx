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

function MarkerToolBubble(props: { message: ChatMessage; nodeId: string }) {
  const ctx = useAppContext();
  const summary = () =>
    resolveToolSummary(props.nodeId, props.message.toolCallId!, ctx.runState());

  return (
    <ToolBubble
      toolName={summary()?.toolName ?? "Tool"}
      status={summary()?.status ?? "proposed"}
      output={summary()?.lastOutput}
      arguments={summary()?.arguments}
      intent={summary()?.intent}
      isError={summary()?.isError}
      streaming={summary()?.streaming ?? false}
    />
  );
}

function PlainMessage(props: {
  message: ChatMessage;
  label: string;
  segmentHeaderShowsNode: boolean;
}) {
  const content = createMemo(() =>
    displayChatContent(props.message.role, props.message.content),
  );
  const shouldRender = createMemo(
    () =>
      content().trim().length > 0 ||
      props.message.streaming ||
      props.message.role === "assistant",
  );
  return (
    <Show when={shouldRender()}>
      <Message
        from={chatRoleToMessageFrom(props.message.role)}
        label={messageLabel(props.message.role, props.label, {
          segmentHeaderShowsNode: props.segmentHeaderShowsNode,
        })}
        content={content()}
        streaming={props.message.streaming}
      />
    </Show>
  );
}

function ConversationItemView(props: {
  item: ConversationItem;
  nodeId: string;
  label: string;
  segmentHeaderShowsNode: boolean;
}) {
  if (isLegacyToolGroup(props.item)) {
    return <LegacyToolBubble group={props.item} />;
  }
  if (props.item.messageKind === "node_completed") {
    return <NodeCompletedBubble summary={props.item.content} />;
  }
  if (props.item.toolCallId) {
    return <MarkerToolBubble message={props.item} nodeId={props.nodeId} />;
  }
  if (isProviderThinkingMessage(props.item)) {
    return <ThinkingBubble message={props.item} />;
  }
  return (
    <PlainMessage
      message={props.item}
      label={props.label}
      segmentHeaderShowsNode={props.segmentHeaderShowsNode}
    />
  );
}

export function ConversationSegmentMessages(props: {
  nodeId: string;
  label: string;
  messages: ChatMessage[];
  emptyLabel?: string;
  segmentHeaderShowsNode?: boolean;
}) {
  const conversationItems = createMemo(() => groupLegacyToolMessages(props.messages));

  return (
    <Show
      when={props.messages.length > 0}
      fallback={
        props.emptyLabel !== undefined ? (
          <p class="chat-live-starting">{props.emptyLabel || "Starting…"}</p>
        ) : null
      }
    >
      <For each={conversationItems()}>
        {(item) => (
          <ConversationItemView
            item={item}
            nodeId={props.nodeId}
            label={props.label}
            segmentHeaderShowsNode={props.segmentHeaderShowsNode ?? false}
          />
        )}
      </For>
    </Show>
  );
}
