import { createEffect, For, Show } from "solid-js";
import Play from "lucide-solid/icons/play";
import RotateCcw from "lucide-solid/icons/rotate-ccw";
import RefreshCw from "lucide-solid/icons/refresh-cw";
import { useAppContext } from "../context/AppContext";
import type { RunSummary } from "../lib/types";

function formatRunTime(ms: number) {
  return new Date(ms).toLocaleString();
}

function canResume(run: RunSummary) {
  return run.status === "paused" || run.status === "stopped" || run.status === "failed";
}

export function RunHistoryPanel() {
  const ctx = useAppContext();

  createEffect(() => {
    if (ctx.bottomTab() === "runs") {
      void ctx.handleRefreshRunHistory();
    }
  });

  return (
    <div class="run-history-panel">
      <header class="run-history-header">
        <div>
          <div class="eyebrow">Runs</div>
          <h3>History</h3>
        </div>
        <button
          type="button"
          class="dock-icon-action"
          title="Refresh runs"
          aria-label="Refresh runs"
          onClick={() => void ctx.handleRefreshRunHistory()}
        >
          <RefreshCw width={15} height={15} />
        </button>
      </header>

      <Show
        when={!ctx.runHistoryLoading()}
        fallback={<div class="empty-panel">Loading runs.</div>}
      >
        <Show
          when={ctx.runHistory().length > 0}
          fallback={<div class="empty-panel">No saved runs for this workflow.</div>}
        >
          <div class="run-history-list">
            <For each={ctx.runHistory()}>
              {(run) => (
                <div class="run-history-row" classList={{ active: ctx.replayRunId() === run.runId }}>
                  <div class="run-history-main">
                    <span class={`run-history-status status-${run.status}`}>{run.status}</span>
                    <strong>{run.workflowName}</strong>
                    <span>{formatRunTime(run.updatedAtMs)}</span>
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
