import ChevronRight from "lucide-solid/icons/chevron-right";
import { createSignal, For, Show } from "solid-js";
import { createUiDesktopOutboundAdapter } from "../../port";
import { useAppContext } from "../../context/AppContext";
import { Spinner } from "../Spinner";
import type { EditBatch, FileChangeRecord } from "../../lib/types";
import { effectiveChangePath, latestChangesByPath, nodeChangedFiles, nodeEditBatches } from "../../lib/workflow";
import { formatToolDisplayName } from "./toolBubbleState";

const desktop = createUiDesktopOutboundAdapter();

const EDIT_TOOLS = new Set(["write", "edit", "apply_patch"]);

function opLabel(op: FileChangeRecord["op"]): string {
  switch (op) {
    case "create":
      return "Created";
    case "update":
      return "Updated";
    case "delete":
      return "Deleted";
    case "rename":
      return "Renamed";
    default:
      return op;
  }
}

function FileChangeRow(props: { record: FileChangeRecord }) {
  const [toolDiffOpen, setToolDiffOpen] = createSignal(Boolean(props.record.diffSummary));
  const [gitDiff, setGitDiff] = createSignal<string | null>(null);
  const [gitDiffOpen, setGitDiffOpen] = createSignal(false);
  const [gitLoading, setGitLoading] = createSignal(false);
  const [gitError, setGitError] = createSignal<string | null>(null);

  async function loadGitDiff() {
    if (gitDiffOpen()) {
      setGitDiffOpen(false);
      return;
    }
    if (gitDiff()) {
      setGitDiffOpen(true);
      return;
    }
    setGitLoading(true);
    setGitError(null);
    try {
      const diff = await desktop.gitDiffFile(effectiveChangePath(props.record));
      setGitDiff(diff.trim() || "(no changes)");
      setGitDiffOpen(true);
    } catch (error) {
      setGitError(error instanceof Error ? error.message : String(error));
    } finally {
      setGitLoading(false);
    }
  }

  return (
    <div class="file-change-row">
      <div class="file-change-row-header">
        <span class="file-change-op">{opLabel(props.record.op)}</span>
        <span class="file-change-path">{props.record.path}</span>
        <Show when={props.record.renameTo}>
          {(renameTo) => <span class="file-change-rename">→ {renameTo()}</span>}
        </Show>
        <div class="file-change-actions">
          <Show when={props.record.diffSummary}>
            <button
              type="button"
              class="file-change-action"
              onClick={() => setToolDiffOpen((value) => !value)}
            >
              {toolDiffOpen() ? "Hide tool diff" : "Tool diff"}
            </button>
          </Show>
          <button
            type="button"
            class="file-change-action"
            disabled={gitLoading()}
            onClick={() => void loadGitDiff()}
          >
            <Show
              when={gitLoading()}
              fallback={gitDiffOpen() ? "Hide git diff" : "Git diff"}
            >
              <span class="loading-inline">
                <Spinner size="sm" />
                Loading…
              </span>
            </Show>
          </button>
        </div>
      </div>
      <Show when={gitError()}>
        <p class="file-change-error">{gitError()}</p>
      </Show>
      <Show when={toolDiffOpen() && props.record.diffSummary}>
        <pre class="file-edit-diff">{props.record.diffSummary}</pre>
      </Show>
      <Show when={gitDiffOpen() && gitDiff()}>
        {(diff) => <pre class="file-edit-diff file-git-diff">{diff()}</pre>}
      </Show>
    </div>
  );
}

function EditBatchRow(props: { batch: EditBatch }) {
  const [busy, setBusy] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);

  async function revert() {
    setBusy(true);
    setError(null);
    try {
      await desktop.revertEditBatch(props.batch.batchId);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div class="edit-batch-row">
      <div class="edit-batch-summary">
        <span class="edit-batch-tool">{formatToolDisplayName(props.batch.toolName)}</span>
        <span class="edit-batch-meta">
          {props.batch.snapshots.length} file
          {props.batch.snapshots.length === 1 ? "" : "s"}
        </span>
      </div>
      <button
        type="button"
        class="secondary-button edit-batch-revert"
        disabled={busy()}
        onClick={() => void revert()}
      >
        {busy() ? "Reverting…" : "Revert batch"}
      </button>
      <Show when={error()}>
        <p class="file-change-error">{error()}</p>
      </Show>
    </div>
  );
}

function panelSummaryLabel(fileCount: number, batchCount: number): string {
  const filePart =
    fileCount > 0 ? `${fileCount} file${fileCount === 1 ? "" : "s"}` : null;
  const batchPart =
    batchCount > 0
      ? `${batchCount} revertible batch${batchCount === 1 ? "" : "es"}`
      : null;
  return [filePart, batchPart].filter(Boolean).join(", ");
}

export function FileChangesPanel() {
  const ctx = useAppContext();
  const [expanded, setExpanded] = createSignal(true);
  const changedFiles = () =>
    latestChangesByPath(nodeChangedFiles(ctx.runState(), ctx.selectedNodeId()));
  const editBatches = () => nodeEditBatches(ctx.runState(), ctx.selectedNodeId());
  const fileCount = () => changedFiles().length;
  const batchCount = () => editBatches().length;

  return (
    <Show when={fileCount() > 0 || batchCount() > 0}>
      <div
        class="file-changes-panel"
        classList={{ "is-collapsed": !expanded() }}
      >
        <button
          type="button"
          class="file-changes-panel-header"
          aria-expanded={expanded()}
          onClick={() => setExpanded((value) => !value)}
        >
          <ChevronRight
            class="file-changes-chevron"
            classList={{ expanded: expanded() }}
            aria-hidden="true"
            size={14}
          />
          <span class="file-changes-panel-title">
            {panelSummaryLabel(fileCount(), batchCount())}
          </span>
        </button>
        <Show when={expanded()}>
          <div class="file-changes-panel-body">
            <Show when={batchCount() > 0}>
              <div class="edit-batches-section">
                <div class="edit-batches-label">Revertible batches</div>
                <For each={editBatches()}>
                  {(batch) => <EditBatchRow batch={batch} />}
                </For>
              </div>
            </Show>
            <Show when={fileCount() > 0}>
              <div class="file-changes-list">
                <For each={changedFiles()}>
                  {(record) => <FileChangeRow record={record} />}
                </For>
              </div>
            </Show>
          </div>
        </Show>
      </div>
    </Show>
  );
}

export function isFileEditTool(name: string): boolean {
  return EDIT_TOOLS.has(name);
}
