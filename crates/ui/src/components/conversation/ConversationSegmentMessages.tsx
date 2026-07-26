import { createMemo, For, Show } from "solid-js";
import { displayChatContent } from "../../lib/stripToolCallMarkup";
import { useAppContext } from "../../context/AppContext";
import type { ChatMessage, ToolCallSummary } from "../../lib/types";
import { isProviderThinkingMessage } from "./providerThinking";
import { chatRoleToMessageFrom, messageLabel } from "./chatRole";
import { Message } from "./Message";
import { NodeCompletedBubble } from "./NodeCompletedBubble";
import { ThinkingBubble } from "./ThinkingBubble";
import {
  TodoChecklist,
  todoItemsFromArguments,
  UPDATE_TODO_LIST_TOOL,
} from "./TodoChecklist";
import { ToolBubble } from "./ToolBubble";
import { ToolStackBubble } from "./ToolStackBubble";
import { groupToolMessages, type GroupedConversationItem } from "./groupToolMessages";
import { resolveToolSummary, toolStackSummaryWithThinking } from "./toolBubbleState";

function todoItemsForSummary(summary: ToolCallSummary | undefined) {
  if (
    summary?.toolName !== UPDATE_TODO_LIST_TOOL ||
    summary.isError ||
    summary.status === "failed" ||
    summary.status === "blocked" ||
    summary.status === "aborted"
  ) {
    return null;
  }
  return todoItemsFromArguments(summary.arguments);
}

function MarkerToolBubble(props: { message: ChatMessage; nodeId: string }) {
  const ctx = useAppContext();
  const summary = () =>
    resolveToolSummary(props.nodeId, props.message.toolCallId!, ctx.runState());
  const todoItems = () => todoItemsForSummary(summary());

  return (
    <Show
      when={todoItems()}
      fallback={
        <ToolBubble
          toolName={summary()?.toolName ?? "Tool"}
          status={summary()?.status ?? "proposed"}
          output={summary()?.lastOutput}
          arguments={summary()?.arguments}
          intent={summary()?.intent}
          isError={summary()?.isError}
          streaming={summary()?.streaming ?? false}
          cwd={ctx.executionCwdForActiveWorkflow()}
        />
      }
    >
      {(items) => (
        <TodoChecklist
          items={items()}
          toolStatus={summary()?.status ?? "proposed"}
        />
      )}
    </Show>
  );
}

function ToolStackView(props: {
  messages: ChatMessage[];
  nodeId: string;
  label: string;
  segmentHeaderShowsNode: boolean;
}) {
  const ctx = useAppContext();
  const summaryText = () =>
    toolStackSummaryWithThinking(
      props.messages
        .filter((message) => Boolean(message.toolCallId))
        .map((message) => {
          const summary = resolveToolSummary(
            props.nodeId,
            message.toolCallId!,
            ctx.runState(),
          );
          return {
            toolName: summary?.toolName ?? "Tool",
            status: summary?.status ?? "proposed",
          };
        }),
      props.messages,
    );

  const persistKey = () => {
    const firstToolId = props.messages.find((message) => message.toolCallId)?.toolCallId;
    return `${props.nodeId}:${firstToolId ?? "stack"}`;
  };

  return (
    <ToolStackBubble summaryText={summaryText()} persistKey={persistKey()}>
      <For each={props.messages}>
        {(message) =>
          message.toolCallId ? (
            <MarkerToolBubble message={message} nodeId={props.nodeId} />
          ) : (
            <ConversationItemView
              message={message}
              nodeId={props.nodeId}
              label={props.label}
              segmentHeaderShowsNode={props.segmentHeaderShowsNode}
            />
          )
        }
      </For>
    </ToolStackBubble>
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
    () => content().trim().length > 0 || props.message.streaming,
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
  message: ChatMessage;
  nodeId: string;
  label: string;
  segmentHeaderShowsNode: boolean;
}) {
  if (props.message.messageKind === "node_completed") {
    return <NodeCompletedBubble summary={props.message.content} />;
  }
  if (props.message.toolCallId) {
    return <MarkerToolBubble message={props.message} nodeId={props.nodeId} />;
  }
  if (isProviderThinkingMessage(props.message)) {
    return <ThinkingBubble message={props.message} />;
  }
  return (
    <PlainMessage
      message={props.message}
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
  const ctx = useAppContext();
  const todoSummaries = createMemo(() => {
    return (ctx.runState()?.toolCallsByNode[props.nodeId] ?? []).filter(
      (summary) => summary.toolName === UPDATE_TODO_LIST_TOOL,
    );
  });
  const latestTodoCallId = createMemo(() => {
    let latest: string | null = null;
    for (const summary of todoSummaries()) {
      if (todoItemsForSummary(summary)) {
        latest = summary.toolCallId;
      }
    }
    return latest;
  });
  const visibleMessages = createMemo(() => {
    const todoById = new Map(
      todoSummaries().map((summary) => [summary.toolCallId, summary]),
    );
    const latest = latestTodoCallId();
    return props.messages.filter((message) => {
      if (!message.toolCallId) return true;
      const summary = todoById.get(message.toolCallId);
      if (!summary) return true;
      return message.toolCallId === latest || todoItemsForSummary(summary) === null;
    });
  });
  const standaloneToolCallIds = createMemo(() => {
    const todoIds = new Set(todoSummaries().map((summary) => summary.toolCallId));
    return new Set(
      visibleMessages()
        .map((message) => message.toolCallId)
        .filter(
          (id): id is string => typeof id === "string" && todoIds.has(id),
        ),
    );
  });
  const items = createMemo((prev: GroupedConversationItem[] | undefined) =>
    groupToolMessages(
      visibleMessages(),
      undefined,
      prev ?? null,
      standaloneToolCallIds(),
    ),
  );

  return (
    <Show
      when={props.messages.length > 0}
      fallback={
        props.emptyLabel !== undefined ? (
          <p class="chat-live-starting">{props.emptyLabel || "Starting…"}</p>
        ) : null
      }
    >
      <div class="chat-segment-body">
        <For each={items()}>
          {(item) => {
            if (item.kind === "toolStack") {
              return (
                <ToolStackView
                  messages={item.messages}
                  nodeId={props.nodeId}
                  label={props.label}
                  segmentHeaderShowsNode={props.segmentHeaderShowsNode ?? false}
                />
              );
            }
            return (
              <ConversationItemView
                message={item.message}
                nodeId={props.nodeId}
                label={props.label}
                segmentHeaderShowsNode={props.segmentHeaderShowsNode ?? false}
              />
            );
          }}
        </For>
      </div>
    </Show>
  );
}
