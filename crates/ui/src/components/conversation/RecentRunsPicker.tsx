import History from "lucide-solid/icons/history";
import Play from "lucide-solid/icons/play";
import RotateCcw from "lucide-solid/icons/rotate-ccw";
import { For, Show } from "solid-js";
import type { RunSummary } from "../../lib/types";

const RECENT_RUN_LIMIT = 3;

function formatRunTime(ms: number) {
  return new Date(ms).toLocaleString([], {
    month: "short",
    day: "numeric",
    hour: "numeric",
    minute: "2-digit",
  });
}

function canContinue(run: RunSummary) {
  return run.status === "paused" || run.status === "stopped" || run.status === "failed";
}

export function RecentRunsPicker(props: {
  runs: RunSummary[];
  currentRunId: string | null;
  onView: (runId: string) => void;
  onContinue: (runId: string) => void;
  onViewAll: () => void;
}) {
  const recentRuns = () =>
    props.runs
      .filter((run) => run.runId !== props.currentRunId)
      .slice(0, RECENT_RUN_LIMIT);

  return (
    <Show when={recentRuns().length > 0}>
      <section class="recent-runs-picker" aria-label="Previous runs">
        <div class="recent-runs-header">
          <span class="recent-runs-title">
            <History aria-hidden="true" width={14} height={14} />
            Previous runs
          </span>
          <button type="button" class="recent-runs-all" onClick={props.onViewAll}>
            All runs
          </button>
        </div>
        <div class="recent-runs-list">
          <For each={recentRuns()}>
            {(run) => (
              <div class="recent-run-item" data-run-id={run.runId}>
                <button
                  type="button"
                  class="recent-run-view"
                  aria-label={`View saved run ${run.runId}`}
                  onClick={() => props.onView(run.runId)}
                >
                  <Play aria-hidden="true" width={12} height={12} />
                  <span class="recent-run-name">{run.name}</span>
                  <time dateTime={new Date(run.updatedAtMs).toISOString()}>
                    {formatRunTime(run.updatedAtMs)}
                  </time>
                  <Show when={canContinue(run)}>
                    <span class={`recent-run-status status-${run.status}`}>
                      {run.status}
                    </span>
                  </Show>
                </button>
                <Show
                  when={canContinue(run)}
                  fallback={
                    <span
                      class={`recent-run-action recent-run-status status-${run.status}`}
                    >
                      {run.status}
                    </span>
                  }
                >
                  <button
                    type="button"
                    class="recent-run-action recent-run-continue"
                    aria-label={`Continue saved run ${run.runId}`}
                    onClick={() => props.onContinue(run.runId)}
                  >
                    <RotateCcw aria-hidden="true" width={12} height={12} />
                    Continue
                  </button>
                </Show>
              </div>
            )}
          </For>
        </div>
      </section>
    </Show>
  );
}
