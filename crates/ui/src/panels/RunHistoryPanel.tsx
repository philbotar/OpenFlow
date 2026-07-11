import { createEffect, For, Show } from "solid-js";
import History from "lucide-solid/icons/history";
import Play from "lucide-solid/icons/play";
import RotateCcw from "lucide-solid/icons/rotate-ccw";
import { PanelEmptyState } from "@/components";
import { useAppContext } from "../context/AppContext";
import type { RunSummary } from "@/lib/types";

function formatRunTime(ms: number) {
  return new Date(ms).toLocaleString();
}

function canResume(run: RunSummary) {
  return run.status === "paused" || run.status === "stopped" || run.status === "failed";
}

export function RunHistoryPanel() {
  const ctx = useAppContext();

  createEffect(() => {
    if (ctx.bottomTab() === "history") {
      void ctx.handleRefreshRunHistory();
    }
  });

  return (
    <div class="run-history-panel">
      <Show
        when={!ctx.runHistoryLoading()}
        fallback={<PanelEmptyState title="Loading runs..." />}
      >
        <Show
          when={ctx.runHistory().length > 0}
          fallback={
            <PanelEmptyState
              icon={<History width={22} height={22} />}
              title="No saved runs yet"
              description="Completed and paused runs for this workflow appear here."
            />
          }
        >
          <div class="run-history-list">
            <For each={ctx.runHistory()}>
              {(run) => (
                <div class="run-history-row" classList={{ active: ctx.replayRunId() === run.runId }}>
                  <span class={`trace-pill ${run.status}`}>{run.status}</span>
                  <div>
                    <strong>{run.workflowName}</strong>
                    <div>{formatRunTime(run.updatedAtMs)}</div>
                  </div>
                  <div class="run-history-actions">
                    <button
                      type="button"
                      class="dock-icon-action"
                      title="Open replay"
                      aria-label="Open replay"
                      onClick={() => void ctx.handleReplayRun(run.runId)}
                    >
                      <Play width={15} height={15} />
                    </button>
                    <Show when={canResume(run)}>
                      <button
                        type="button"
                        class="dock-icon-action"
                        title="Resume run"
                        aria-label="Resume run"
                        onClick={() => void ctx.handleResumeDurableRun(run.runId)}
                      >
                        <RotateCcw width={15} height={15} />
                      </button>
                    </Show>
                  </div>
                </div>
              )}
            </For>
          </div>
        </Show>
      </Show>
    </div>
  );
}
