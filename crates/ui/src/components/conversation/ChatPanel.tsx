import { createMemo, Show } from "solid-js";
import RotateCcw from "lucide-solid/icons/rotate-ccw";
import { GLOBAL_RUN_ENTRY_NODE_ID, isLiveTranscriptSegment } from "../../lib/workflow";
import { useAppContext } from "../../context/AppContext";
import { ConversationComposer } from "./ConversationComposer";
import { ConversationMessages } from "./ConversationMessages";
import { LiveSegmentFooter } from "./LiveSegmentFooter";

export function ChatPanel() {
  const ctx = useAppContext();
  const inReplayMode = () => ctx.replayRunId() !== null;

  const inlineLiveSegment = createMemo(() =>
    ctx.chatLayout().settled.find((segment) =>
      isLiveTranscriptSegment(ctx.runState(), segment),
    ),
  );

  const replayRunSummary = createMemo(() => {
    const runId = ctx.replayRunId();
    if (!runId) {
      return null;
    }
    return ctx.runHistory().find((run) => run.runId === runId) ?? null;
  });

  return (
    <div class="chat-layout">
      <Show when={inReplayMode()}>
        <div class="chat-replay-banner" role="status">
          <span>
            Viewing saved run
            {replayRunSummary() ? ` (${replayRunSummary()!.status})` : ""} — read-only.
          </span>
          <Show when={replayRunSummary() && replayRunSummary()!.status !== "completed"}>
            <button
              type="button"
              class="secondary-button small"
              onClick={() => void ctx.handleResumeDurableRun(ctx.replayRunId()!)}
            >
              <RotateCcw width={14} height={14} />
              Resume run
            </button>
          </Show>
        </div>
      </Show>
      <ConversationMessages />
      <div class="chat-composer-bar">
        <Show when={inlineLiveSegment()}>
          {(segment) => <LiveSegmentFooter segment={segment()} />}
        </Show>
        <Show when={!inReplayMode() && ctx.chatLayout().live.length === 0 && !inlineLiveSegment()}>
          <Show
            when={ctx.runState()?.active}
            fallback={
              <ConversationComposer
                nodeId={GLOBAL_RUN_ENTRY_NODE_ID}
                label="workflow"
                kickoff
              />
            }
          >
            <div class="chat-live-strip chat-live-strip--pending" aria-live="polite">
              <p class="chat-live-starting">Starting workflow…</p>
            </div>
          </Show>
        </Show>
      </div>
    </div>
  );
}
