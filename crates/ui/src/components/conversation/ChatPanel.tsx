import { For, Show } from "solid-js";
import { labelForAgentStatus } from "../../lib/agentStatus";
import { isLiveTranscriptSegment } from "../../lib/workflow";
import { useAppContext } from "../../context/AppContext";
import { ConversationComposer } from "./ConversationComposer";
import { ConversationMessages } from "./ConversationMessages";
import { FileChangesPanel } from "./FileChangesPanel";

/**
 * When parallel nodes run, the global chat blocks until the user picks one to
 * talk to. The picked node streams inline in the conversation; the remaining
 * live nodes stay here until each completes in turn.
 */
function LiveNodePicker() {
  const ctx = useAppContext();
  const locked = () => ctx.pickedLiveNodeId() !== null;

  return (
    <div class="chat-live-picker" role="group" aria-label="Pick a running node to talk to">
      <p class="chat-live-picker-hint">
        <Show
          when={!locked()}
          fallback={`${ctx.chatLayout().live.length} more nodes running — finish the current one first`}
        >
          {ctx.chatLayout().live.length} nodes running in parallel — pick one to talk to
        </Show>
      </p>
      <div class="chat-live-picker-options">
        <For each={ctx.chatLayout().live}>
          {(segment) => (
            <button
              type="button"
              class="chat-live-picker-option"
              classList={{
                "has-activity":
                  segment.status === "awaiting_input" ||
                  segment.status === "awaiting_tool_approval",
              }}
              disabled={locked()}
              onClick={() => ctx.setPickedLiveNodeId(segment.nodeId)}
            >
              <span class={`chat-filter-status-dot status-${segment.status}`} />
              {segment.label}
              <span class="chat-live-picker-status">
                {labelForAgentStatus(segment.status)}
              </span>
            </button>
          )}
        </For>
      </div>
    </div>
  );
}

export function ChatPanel() {
  const ctx = useAppContext();

  const hasInlineLiveSegment = () =>
    ctx.chatLayout().settled.some((segment) =>
      isLiveTranscriptSegment(ctx.runState(), segment),
    );

  return (
    <div class="chat-layout">
      <ConversationMessages />
      <div class="chat-side-panels">
        <FileChangesPanel />
      </div>
      <Show
        when={ctx.chatLayout().live.length > 0}
        fallback={
          <Show
            when={ctx.runState()?.active && !hasInlineLiveSegment()}
            fallback={
              <Show when={!hasInlineLiveSegment()}>
                <ConversationComposer
                  nodeId={ctx.selectedNodeId() ?? "inactive"}
                  label="workflow"
                  disabled
                />
              </Show>
            }
          >
            <div class="chat-live-strip chat-live-strip--pending" aria-live="polite">
              <p class="chat-live-starting">Starting workflow…</p>
            </div>
          </Show>
        }
      >
        <LiveNodePicker />
      </Show>
    </div>
  );
}
