import { createEffect, createMemo, createSignal, For, Show } from "solid-js";
import { Button } from "@/components";
import { GLOBAL_RUN_ENTRY_NODE_ID, isLiveTranscriptSegment } from "../../lib/workflow";
import { useAppContext } from "../../context/AppContext";
import { ConversationComposer } from "./ConversationComposer";
import { ConversationMessages } from "./ConversationMessages";
import { LiveSegmentFooter } from "./LiveSegmentFooter";
import { RecentRunsPicker } from "./RecentRunsPicker";
import { ToolApprovalCardBody } from "./ToolApprovalCard";

export function ChatPanel() {
  const ctx = useAppContext();
  const [recentRunsHidden, setRecentRunsHidden] = createSignal(false);
  const [replayOpening, setReplayOpening] = createSignal(false);
  let recentRunsWorkflowId: string | null | undefined;
  const inReplayMode = () => ctx.replayRunId() !== null;

  createEffect(() => {
    const workflowId = ctx.activeWorkflow()?.id ?? null;
    if (workflowId === recentRunsWorkflowId) {
      return;
    }
    recentRunsWorkflowId = workflowId;
    setRecentRunsHidden(false);
    if (workflowId) {
      void ctx.handleRefreshRunHistory();
    }
  });

  const inlineLiveSegment = createMemo(() =>
    ctx.chatLayout().settled.find((segment) =>
      isLiveTranscriptSegment(ctx.runState(), segment),
    ),
  );

  const showParallelLiveHint = createMemo(
    () =>
      ctx.replayRunId() === null &&
      ctx.chatFilterNodeId() === null &&
      ctx.pickedLiveNodeId() === null &&
      ctx.chatLayout().live.length > 1,
  );

  const parallelLiveCount = createMemo(() => ctx.chatLayout().live.length);

  const retryableNodes = createMemo(() => {
    const state = ctx.runState();
    if (!state?.active) {
      return [];
    }
    const nodes = ctx.activeWorkflow()?.nodes ?? [];
    return Object.entries(state.statusByNode)
      .filter(([, status]) => status === "failed" || status === "interrupted")
      .map(([nodeId, status]) => ({
        nodeId,
        status,
        label: nodes.find((node) => node.id === nodeId)?.label ?? nodeId,
      }));
  });
  const composerRetryNode = createMemo(() => {
    const nodes = retryableNodes();
    if (nodes.length === 0) {
      return null;
    }
    const focus = ctx.chatFilterNodeId() ?? ctx.pickedLiveNodeId();
    if (focus) {
      return nodes.find((node) => node.nodeId === focus) ?? null;
    }
    return nodes[0] ?? null;
  });
  const reviewingRun = createMemo(() => {
    const statuses = Object.values(ctx.runState()?.statusByNode ?? {});
    return statuses.length > 0 && statuses.every((status) => status === "completed");
  });

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
      <Show when={planModeStatus()}>
        {(status) => (
          <div class="chat-replay-banner" role="status">
            <span>
              <strong>Plan mode</strong> — {status().frozen
                ? `${status().sourceLabel} approved the plan. File edits are allowed.`
                : `Planning in progress. Only docs/**/*.md writes are allowed until ${status().sourceLabel} approves the plan.`}
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
      <Show
        when={
          !inReplayMode() &&
          !replayOpening() &&
          !ctx.runState()?.active &&
          !ctx.runState()?.runId &&
          !recentRunsHidden() &&
          !ctx.runHistoryLoading()
        }
      >
        <RecentRunsPicker
          runs={ctx.runHistory()}
          currentRunId={ctx.runState()?.runId ?? null}
          onView={(runId) => {
            setReplayOpening(true);
            void ctx.handleReplayRun(runId).finally(() => setReplayOpening(false));
          }}
          onContinue={(runId) => void ctx.handleResumeDurableRun(runId)}
          onViewAll={() => ctx.handleSelectBottomTab("history")}
        />
      </Show>
      <div class="chat-composer-bar" data-tour="workflow-composer">
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
        <Show when={ctx.chatLayout().live.length === 0 && !inlineLiveSegment()}>
          <Show
            when={ctx.runState()?.active && !ctx.startingRun()}
            fallback={
              <Show when={!ctx.runState()?.active}>
                <ConversationComposer
                  nodeId={GLOBAL_RUN_ENTRY_NODE_ID}
                  label={ctx.screen() === "chat" ? "chat" : "workflow"}
                  kickoff
                  onMessageSubmit={() => setRecentRunsHidden(true)}
                />
              </Show>
            }
          >
            <div class="chat-live-strip chat-live-strip--pending" aria-live="polite">
              <Show
                when={retryableNodes().length > 0}
                fallback={
                  <p class="chat-live-starting">
                    {reviewingRun() ? "Reviewing run…" : "Starting workflow…"}
                  </p>
                }
              >
                <div class="chat-retry-list">
                  <For each={retryableNodes()}>
                    {(node) => (
                      <div class="chat-retry-entry">
                        <div class="chat-retry-prompt" role="alert">
                          <p class="chat-live-starting">
                            <strong>
                              {node.label} {node.status === "failed" ? "failed" : "was interrupted"}
                            </strong>
                          </p>
                          <Button
                            variant="secondary"
                            size="small"
                            aria-label={`Retry ${node.label}`}
                            onClick={() => void ctx.handleRetryNode(node.nodeId)}
                          >
                            Retry
                          </Button>
                        </div>
                      </div>
                    )}
                  </For>
                  <Show when={composerRetryNode()}>
                    {(node) => (
                      <ConversationComposer nodeId={node().nodeId} label={node().label} />
                    )}
                  </Show>
                </div>
              </Show>
            </div>
          </Show>
        </Show>
      </div>
    </div>
  );
}
