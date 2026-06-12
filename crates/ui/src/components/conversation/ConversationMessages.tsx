import MessageCircle from "lucide-solid/icons/message-circle";
import {
  createEffect,
  createMemo,
  createSignal,
  For,
  onCleanup,
  Show,
} from "solid-js";
import { labelForAgentStatus } from "../../lib/agentStatus";
import type { NodeId } from "../../lib/types";
import { isLiveTranscriptSegment } from "../../lib/workflow";
import { useAppContext } from "../../context/AppContext";
import { LiveSegmentFooter } from "./LiveSegmentFooter";
import {
  Conversation,
  ConversationContent,
  ConversationEmptyState,
  ConversationScrollButton,
} from "./Conversation";
import { ConversationSegmentMessages } from "./ConversationSegmentMessages";

export function ConversationMessages() {
  const ctx = useAppContext();
  const segmentRefs = new Map<NodeId, HTMLElement>();
  const [flashNodeId, setFlashNodeId] = createSignal<NodeId | null>(null);

  const visibleSettled = createMemo(() => {
    const filter = ctx.chatFilterNodeId();
    const segments = ctx.chatLayout().settled;
    if (!filter) {
      return segments;
    }
    return segments.filter((segment) => segment.nodeId === filter);
  });

  const showFilterChips = createMemo(() => ctx.chatLayout().settled.length > 1);

  createEffect(() => {
    const focus = ctx.chatFocusNode();
    if (!focus) {
      return;
    }
    const settledIds = new Set(ctx.chatLayout().settled.map((segment) => segment.nodeId));
    if (!settledIds.has(focus.nodeId)) {
      return;
    }
    const element = segmentRefs.get(focus.nodeId);
    if (element && typeof element.scrollIntoView === "function") {
      element.scrollIntoView({ behavior: "smooth", block: "nearest" });
    }
    setFlashNodeId(focus.nodeId);
    const timer = setTimeout(() => setFlashNodeId(null), 1500);
    onCleanup(() => clearTimeout(timer));
  });

  return (
    <div class="chat-settled">
      <Show when={showFilterChips()}>
        <div class="chat-filter-chips" role="toolbar" aria-label="Filter settled history">
          <button
            type="button"
            class="chat-filter-chip"
            classList={{ active: ctx.chatFilterNodeId() === null }}
            onClick={() => ctx.setChatFilterNodeId(null)}
          >
            All
          </button>
          <For each={ctx.chatLayout().settled}>
            {(segment) => (
              <button
                type="button"
                class="chat-filter-chip"
                classList={{ active: ctx.chatFilterNodeId() === segment.nodeId }}
                onClick={() => ctx.setChatFilterNodeId(segment.nodeId)}
              >
                <span class={`chat-filter-status-dot status-${segment.status}`} />
                {segment.label}
              </button>
            )}
          </For>
        </div>
      </Show>
      <Conversation class="chat-settled-conversation">
        {(conversation) => (
          <>
            <ConversationContent conversation={conversation}>
              <Show
                when={visibleSettled().length > 0}
                fallback={
                  <ConversationEmptyState
                    icon={
                      <MessageCircle
                        class="conversation-empty-icon-svg"
                        width={22}
                        height={22}
                      />
                    }
                  />
                }
              >
                <For each={visibleSettled()}>
                  {(segment) => (
                    <section
                      class="chat-segment"
                      classList={{ "is-focused": flashNodeId() === segment.nodeId }}
                      data-node-id={segment.nodeId}
                      ref={(element) => {
                        if (element) {
                          segmentRefs.set(segment.nodeId, element);
                        } else {
                          segmentRefs.delete(segment.nodeId);
                        }
                      }}
                    >
                      <header class="chat-segment-header">
                        <span class="eyebrow">{segment.label}</span>
                        <span class={`chat-segment-status status-${segment.status}`}>
                          {labelForAgentStatus(segment.status)}
                        </span>
                        <Show
                          when={
                            isLiveTranscriptSegment(ctx.runState(), segment) &&
                            ctx.composerBusyFor(segment.nodeId)
                          }
                        >
                          <span class="chat-live-streaming-dot" aria-label="Streaming" />
                        </Show>
                      </header>
                      <ConversationSegmentMessages
                        nodeId={segment.nodeId}
                        label={segment.label}
                        messages={segment.messages}
                        segmentHeaderShowsNode
                      />
                      <Show when={isLiveTranscriptSegment(ctx.runState(), segment)}>
                        <LiveSegmentFooter segment={segment} />
                      </Show>
                    </section>
                  )}
                </For>
              </Show>
            </ConversationContent>
            <ConversationScrollButton conversation={conversation} />
          </>
        )}
      </Conversation>
    </div>
  );
}
