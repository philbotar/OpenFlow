import {
  createEffect,
  createMemo,
  createSignal,
  For,
  onCleanup,
  onMount,
  Show,
} from "solid-js";
import { labelForAgentStatus } from "../../lib/agentStatus";
import type { TranscriptSegment } from "../../lib/workflow";
import { useAppContext } from "../../context/AppContext";
import { ConversationComposer } from "./ConversationComposer";
import { ConversationMessages } from "./ConversationMessages";
import { FileChangesPanel } from "./FileChangesPanel";
import { LiveNodeColumn } from "./LiveNodeColumn";

const WIDE_STRIP_THRESHOLD_PX = 720;

function partitionLiveColumns(live: TranscriptSegment[], maxColumns: number) {
  if (live.length <= maxColumns) {
    return { dedicated: live, tabbed: [] as TranscriptSegment[] };
  }
  return {
    dedicated: live.slice(0, maxColumns - 1),
    tabbed: live.slice(maxColumns - 1),
  };
}

function LiveTabbedColumn(props: {
  segments: TranscriptSegment[];
  activeNodeId: string;
  onSelect: (nodeId: string) => void;
}) {
  const activeSegment = createMemo(
    () =>
      props.segments.find((segment) => segment.nodeId === props.activeNodeId) ??
      props.segments[0],
  );

  return (
    <div class="chat-live-column chat-live-tabs-column">
      <div class="chat-live-tabs" role="tablist">
        <For each={props.segments}>
          {(segment) => (
            <button
              type="button"
              role="tab"
              class="chat-live-tab"
              classList={{
                active: segment.nodeId === activeSegment()?.nodeId,
                "has-activity":
                  segment.status === "started" ||
                  segment.status === "running_tool" ||
                  segment.status === "awaiting_input" ||
                  segment.status === "awaiting_tool_approval",
              }}
              aria-selected={segment.nodeId === activeSegment()?.nodeId}
              onClick={() => props.onSelect(segment.nodeId)}
            >
              <span class={`chat-filter-status-dot status-${segment.status}`} />
              {segment.label}
              <span class="chat-live-tab-status">{labelForAgentStatus(segment.status)}</span>
            </button>
          )}
        </For>
      </div>
      <Show when={activeSegment()}>
        {(segment) => <LiveNodeColumn segment={segment()} />}
      </Show>
    </div>
  );
}

export function ChatPanel() {
  const ctx = useAppContext();
  let stripRef: HTMLDivElement | undefined;
  const [stripWidth, setStripWidth] = createSignal(0);
  const [tabbedActiveNodeId, setTabbedActiveNodeId] = createSignal<string | null>(null);

  const maxColumns = createMemo(() => (stripWidth() >= WIDE_STRIP_THRESHOLD_PX ? 3 : 2));
  const livePartition = createMemo(() =>
    partitionLiveColumns(ctx.chatLayout().live, maxColumns()),
  );

  onMount(() => {
    if (!stripRef) {
      return;
    }
    const observer = new ResizeObserver((entries) => {
      const entry = entries[0];
      if (entry) {
        setStripWidth(entry.contentRect.width);
      }
    });
    observer.observe(stripRef);
    onCleanup(() => observer.disconnect());
  });

  createEffect(() => {
    const tabbed = livePartition().tabbed;
    if (tabbed.length === 0) {
      setTabbedActiveNodeId(null);
      return;
    }
    const focus = ctx.chatFocusNode();
    if (focus && tabbed.some((segment) => segment.nodeId === focus.nodeId)) {
      setTabbedActiveNodeId(focus.nodeId);
      return;
    }
    const awaiting = tabbed.find(
      (segment) =>
        segment.status === "awaiting_input" || segment.status === "awaiting_tool_approval",
    );
    if (awaiting) {
      setTabbedActiveNodeId(awaiting.nodeId);
      return;
    }
    if (!tabbed.some((segment) => segment.nodeId === tabbedActiveNodeId())) {
      setTabbedActiveNodeId(tabbed[0]?.nodeId ?? null);
    }
  });

  createEffect(() => {
    const focus = ctx.chatFocusNode();
    if (!focus) {
      return;
    }
    const liveIds = new Set(ctx.chatLayout().live.map((segment) => segment.nodeId));
    if (!liveIds.has(focus.nodeId)) {
      return;
    }
    const element = stripRef?.querySelector(`[data-node-id="${focus.nodeId}"]`);
    if (element instanceof HTMLElement && typeof element.scrollIntoView === "function") {
      element.scrollIntoView({ behavior: "smooth", block: "nearest", inline: "nearest" });
      element.classList.add("is-focused");
      const timer = setTimeout(() => element.classList.remove("is-focused"), 1500);
      onCleanup(() => clearTimeout(timer));
    }
  });

  return (
    <div class="chat-layout">
      <ConversationMessages />
      <div class="chat-side-panels">
        <FileChangesPanel />
      </div>
      <Show
        when={ctx.chatLayout().live.length > 0}
        fallback={
          <Show when={!ctx.runState()?.active}>
            <ConversationComposer
              nodeId={ctx.selectedNodeId() ?? "inactive"}
              label="workflow"
              disabled
            />
          </Show>
        }
      >
        <div class="chat-live-strip" ref={stripRef}>
          <For each={livePartition().dedicated}>
            {(segment) => <LiveNodeColumn segment={segment} />}
          </For>
          <Show when={livePartition().tabbed.length > 0}>
            <LiveTabbedColumn
              segments={livePartition().tabbed}
              activeNodeId={tabbedActiveNodeId() ?? livePartition().tabbed[0]?.nodeId ?? ""}
              onSelect={setTabbedActiveNodeId}
            />
          </Show>
        </div>
      </Show>
    </div>
  );
}
