import { createMemo, createResource, Show } from "solid-js";
import * as desktop from "../api";
import { useAppContext } from "../context/AppContext";
import { Spinner } from "@/components";
import { parseUnifiedDiff, formatDiffFileSummary } from "@/lib/diff";
import type { WorkflowRunState } from "@/lib/types";
import { GitDiffView } from "./GitDiffView";

function errorText(error: unknown): string {
  if (error instanceof Error) return error.message;
  return String(error);
}

/** Refetch when cwd or run file edits change — not runTrace (grows every token). */
function repoDiffSource(cwd: string | null, runState: WorkflowRunState | null): string | null {
  if (!cwd) return null;
  const fileEvents = runState?.changedFiles.length ?? 0;
  return `${cwd}\0${fileEvents}`;
}

export function GitPanel() {
  const ctx = useAppContext();
  const cwd = createMemo(() => ctx.executionCwdForActiveWorkflow());
  const diffSource = createMemo(() => repoDiffSource(cwd(), ctx.runState()));
  const [branch] = createResource(cwd, (dir) => (dir ? desktop.gitCurrentBranch(dir) : Promise.resolve("")));
  const [diff] = createResource(diffSource, (key) => {
    if (!key) return "";
    const projectCwd = key.split("\0")[0]!;
    return desktop.gitDiffRepo(projectCwd);
  });
  const files = () => (diff() ? parseUnifiedDiff(diff() as string) : []);
  const branchTitle = () => branch()?.trim() || "Git";

  return (
    <aside class="inspector-panel git-panel panel-enter">
      <div class="panel-header">
        <div class="panel-header-copy">
          <div class="eyebrow">Git</div>
          <div class="panel-header-title-row">
            <h3>{branchTitle()}</h3>
          </div>
          <Show when={!diff.loading && files().length > 0}>
            <p class="git-panel-subtitle">{formatDiffFileSummary(files())}</p>
          </Show>
        </div>
      </div>
      <div class="git-body">
        <Show
          when={!diff.loading}
          fallback={
            <div class="git-empty">
              <Spinner size="sm" /> Loading diff…
            </div>
          }
        >
          <Show when={!diff.error} fallback={<p class="git-empty">{errorText(diff.error)}</p>}>
            <Show when={files().length > 0} fallback={<p class="git-empty">No uncommitted changes.</p>}>
              <GitDiffView files={files()} />
            </Show>
          </Show>
        </Show>
      </div>
    </aside>
  );
}
