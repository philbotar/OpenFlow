import MessageCircle from "lucide-solid/icons/message-circle";
import {
  createEffect,
  createMemo,
  createSignal,
  For,
  onCleanup,
  Show,
} from "solid-js";
import { labelForAgentStatus } from "../../lib/agentStatusLabels";
import type { NodeId } from "../../lib/types";
import { isLiveTranscriptSegment, sortTranscriptSegmentsByNodeOrder } from "../../lib/workflow";
import { useAppContext } from "../../context/AppContext";
import { PanelEmptyState } from "../PanelEmptyState";
import {
  Conversation,
  ConversationContent,
  ConversationScrollButton,
} from "./Conversation";
import { ConversationSegmentMessages } from "./ConversationSegmentMessages";

export function ConversationMessages() {
  const ctx = useAppContext();
  const segmentRefs = new Map<NodeId, HTMLElement>();
  const [flashNodeId, setFlashNodeId] = createSignal<NodeId | null>(null);

  const visibleSettled = createMemo(() => {
    const filter = ctx.chatFilterNodeId() ?? ctx.pickedLiveNodeId();
    const segments = ctx.chatLayout().settled;
    if (!filter) {
      return segments;
    }
    return segments.filter((segment) => segment.nodeId === filter);
  });

  const filterChips = createMemo(() => {
    const layout = ctx.chatLayout();
    const segments = [...layout.settled, ...layout.live];
    return sortTranscriptSegmentsByNodeOrder(segments, ctx.chatSegmentOrder());
  });

  const showFilterChips = createMemo(() => filterChips().length > 1);

  const liveNodeIds = createMemo(
    () => new Set(ctx.chatLayout().live.map((segment) => segment.nodeId)),
  );

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
        <div
          class="chat-filter-chips chat-filter-strip"
          role="toolbar"
          aria-label="Filter conversation by node"
        >
          <button
            type="button"
            class="chat-filter-chip"
            classList={{
              active:
                ctx.chatFilterNodeId() === null && ctx.pickedLiveNodeId() === null,
            }}
            onClick={() => {
              ctx.setChatFilterNodeId(null);
              ctx.setPickedLiveNodeId(null);
            }}
          >
            All
          </button>
          <For each={filterChips()}>
            {(segment) => (
              <button
                type="button"
                class="chat-filter-chip"
                classList={{
                  active:
                    ctx.pickedLiveNodeId() === segment.nodeId ||
                    (ctx.pickedLiveNodeId() === null &&
                      ctx.chatFilterNodeId() === segment.nodeId),
                  "has-activity":
                    segment.status === "awaiting_input" ||
                    segment.status === "awaiting_tool_approval",
                }}
                onClick={() => {
                  const isLive = liveNodeIds().has(segment.nodeId);
                  if (isLive) {
                    ctx.setPickedLiveNodeId(segment.nodeId);
                    ctx.setChatFilterNodeId(null);
                    return;
                  }
                  ctx.setChatFilterNodeId(segment.nodeId);
                  ctx.setPickedLiveNodeId(null);
                }}
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
            <ConversationContent conversation={conversation} class="chat-transcript-scroll">
              <div class="chat-transcript-lane">
                <Show
                  when={visibleSettled().length > 0}
                  fallback={
                    <PanelEmptyState
                      title="No messages yet"
                      description="Send a message to start the workflow."
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
                      </section>
                    )}
                  </For>
                </Show>
              </div>
            </ConversationContent>
            <ConversationScrollButton conversation={conversation} />
          </>
        )}
      </Conversation>
    </div>
  );
}
