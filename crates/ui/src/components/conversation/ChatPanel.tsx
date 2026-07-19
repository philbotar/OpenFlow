import { createMemo, Show } from "solid-js";
import RotateCcw from "lucide-solid/icons/rotate-ccw";
import { Button } from "@/components";
import { GLOBAL_RUN_ENTRY_NODE_ID, isLiveTranscriptSegment } from "../../lib/workflow";
import { useAppContext } from "../../context/AppContext";
import { ConversationComposer } from "./ConversationComposer";
import { ConversationMessages } from "./ConversationMessages";
import { LiveSegmentFooter } from "./LiveSegmentFooter";
import { ToolApprovalCardBody } from "./ToolApprovalCard";

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

  const showParallelLiveHint = createMemo(
    () =>
      ctx.replayRunId() === null &&
      ctx.chatFilterNodeId() === null &&
      ctx.pickedLiveNodeId() === null &&
      ctx.chatLayout().live.length > 1,
  );

  const parallelLiveCount = createMemo(() => ctx.chatLayout().live.length);

  const waitingToRetry = createMemo(() =>
    Object.values(ctx.runState()?.statusByNode ?? {}).some(
      (status) => status === "failed" || status === "interrupted",
    ),
  );

  // Surface approval outside the parallel-live picker — otherwise the card only
  // appears after the user picks (or the sibling finishes and folds inline).
  const pendingApproval = createMemo(() => ctx.runState()?.pendingApprovals[0] ?? null);

  const planModeStatus = createMemo(() => {
    const workflow = ctx.activeWorkflow();
    const runState = ctx.runState();
    const sourceNodeId =
      runState?.planMode?.evidenceSourceNodeId ??
      workflow?.settings?.planMode?.evidenceSourceNodeId;
    if (!sourceNodeId || !runState) {
      return null;
    }
    const source = workflow?.nodes.find((node) => node.id === sourceNodeId);
    const frozen =
      runState.planMode?.phase === "execution" ||
      runState.statusByNode[sourceNodeId] === "completed";
    return {
      sourceLabel: source?.label ?? sourceNodeId,
      frozen,
    };
  });

  return (
    <div class="chat-layout">
      <Show when={inReplayMode()}>
        <div class="chat-replay-banner" role="status">
          <span>
            Viewing saved run
            {replayRunSummary() ? ` (${replayRunSummary()!.status})` : ""} — read-only.
          </span>
          <div class="chat-replay-banner-actions">
            <Show when={replayRunSummary() && replayRunSummary()!.status !== "completed"}>
              <Button variant="secondary" size="small" onClick={() => void ctx.handleResumeDurableRun(ctx.replayRunId()!)}>
                <RotateCcw width={14} height={14} />
                Resume run
              </Button>
            </Show>
            <Button variant="secondary" size="small" onClick={() => void ctx.handleExitReplay()}>
              Exit replay
            </Button>
          </div>
        </div>
      </Show>
      <Show when={planModeStatus()}>
        {(status) => (
          <div class="chat-replay-banner" role="status">
            <span>
              <strong>Plan mode</strong> — {status().frozen
                ? `${status().sourceLabel} approved the plan. File edits are allowed.`
                : `Planning in progress. File edits stay blocked until ${status().sourceLabel} approves the plan.`}
            </span>
          </div>
        )}
      </Show>
      <ConversationMessages />
      <Show when={showParallelLiveHint()}>
        <div class="chat-parallel-hint" role="status" aria-live="polite">
          <span>
            <strong>{parallelLiveCount()}</strong> agents are running in parallel.
          </span>
          <span>Select a node above to view and reply.</span>
        </div>
      </Show>
      <div class="chat-composer-bar">
        <Show when={pendingApproval()}>
          {(approval) => (
            <ToolApprovalCardBody
              approval={approval()}
              onApprove={(allow) =>
                void ctx.handleToolApproval(approval().approvalId, allow)
              }
            />
          )}
        </Show>
        <Show when={inlineLiveSegment()}>
          {(segment) => <LiveSegmentFooter segment={segment()} />}
        </Show>
        <Show when={!inReplayMode() && ctx.chatLayout().live.length === 0 && !inlineLiveSegment()}>
          <Show
            when={ctx.runState()?.active && !ctx.startingRun()}
            fallback={
              <Show when={!ctx.runState()?.active}>
                <ConversationComposer
                  nodeId={GLOBAL_RUN_ENTRY_NODE_ID}
                  label="workflow"
                  kickoff
                />
              </Show>
            }
          >
            <div class="chat-live-strip chat-live-strip--pending" aria-live="polite">
              <p class="chat-live-starting">
                {waitingToRetry() ? "Waiting to retry…" : "Starting workflow…"}
              </p>
            </div>
          </Show>
        </Show>
      </div>
    </div>
  );
}
