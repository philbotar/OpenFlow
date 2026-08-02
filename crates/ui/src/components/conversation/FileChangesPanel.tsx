import AlertTriangle from "lucide-solid/icons/triangle-alert";
import ChevronRight from "lucide-solid/icons/chevron-right";
import { createEffect, createMemo, createSignal, For, Show } from "solid-js";
import * as desktop from "../../api";
import { useAppContext } from "../../context/AppContext";
import type { FileChangeRecord, NodeId } from "../../lib/types";
import { effectiveChangePath, nodeChangedFiles } from "../../lib/workflow";
import { Spinner } from "../Spinner";
import { formatToolDisplayName } from "./toolBubbleState";

const EDIT_TOOLS = new Set(["write", "edit", "apply_patch"]);
const expandedPanelKeys = new Set<string>();

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

function FileChangeRow(props: {
  record: FileChangeRecord;
  runId: string | null;
}) {
  const [diff, setDiff] = createSignal<string | null>(null);
  const [diffOpen, setDiffOpen] = createSignal(false);
  const [loading, setLoading] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);
  const hasDiff = () => Boolean(props.record.diffArtifactId || props.record.diffSummary);
  const shownDiff = () => diff() ?? props.record.diffSummary ?? "";

  async function loadDiff() {
    if (diffOpen()) {
      setDiffOpen(false);
      return;
    }
    if (diff() !== null || !props.record.diffArtifactId) {
      setDiffOpen(true);
      return;
    }
    if (!props.runId) {
      setError("Run id unavailable. Reopen this run, then retry.");
      return;
    }
    setLoading(true);
    setError(null);
    try {
      const exactDiff = await desktop.loadFileChangeDiff(
        props.runId,
        props.record.diffArtifactId,
      );
      setDiff(exactDiff || "(empty diff)");
      setDiffOpen(true);
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : String(loadError));
    } finally {
      setLoading(false);
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
        <Show when={props.record.toolName}>
          {(toolName) => (
            <span class="file-change-tool">
              via {formatToolDisplayName(toolName())}
            </span>
          )}
        </Show>
        <div class="file-change-actions">
          <Show when={!props.record.diffArtifactId && props.record.diffSummary}>
            <span class="file-change-summary-only">Summary only</span>
          </Show>
          <Show when={hasDiff()}>
            <button
              type="button"
              class="file-change-action"
              disabled={loading()}
              onClick={() => void loadDiff()}
            >
              <Show
                when={loading()}
                fallback={diffOpen() ? "Hide diff" : "View diff"}
              >
                <span class="loading-inline">
                  <Spinner size="sm" />
                  Loading…
                </span>
              </Show>
            </button>
          </Show>
        </div>
      </div>
      <Show when={error()}>
        {(message) => (
          <p class="file-change-error">
            {message()}{" "}
            <button
              type="button"
              class="file-change-retry"
              disabled={loading()}
              onClick={() => void loadDiff()}
            >
              Retry
            </button>
          </p>
        )}
      </Show>
      <Show when={diffOpen() && shownDiff()}>
        <pre class="file-edit-diff">{shownDiff()}</pre>
      </Show>
    </div>
  );
}

function panelSummaryLabel(fileCount: number, editCount: number): string {
  return `${fileCount} file${fileCount === 1 ? "" : "s"} changed · ${editCount} edit${
    editCount === 1 ? "" : "s"
  }`;
}

export function FileChangesPanel(props: { nodeId: NodeId }) {
  const ctx = useAppContext();
  const panelKey = () => `${ctx.runState()?.runId ?? "no-run"}:${props.nodeId}`;
  let currentPanelKey = panelKey();
  const [expanded, setExpanded] = createSignal(
    expandedPanelKeys.has(currentPanelKey),
  );
  const changedFiles = createMemo(() =>
    nodeChangedFiles(ctx.runState(), props.nodeId)
      .map((record, index) => ({ record, index }))
      .sort(
        (left, right) =>
          left.record.timestampMs - right.record.timestampMs ||
          left.index - right.index,
      )
      .map(({ record }) => record),
  );
  const fileCount = createMemo(
    () => new Set(changedFiles().map(effectiveChangePath)).size,
  );
  const usedBash = createMemo(() =>
    (ctx.runState()?.toolCallsByNode?.[props.nodeId] ?? []).some(
      (call) => call.toolName === "bash",
    ),
  );
  const runId = () => ctx.runState()?.runId ?? null;

  createEffect(() => {
    const nextPanelKey = panelKey();
    if (nextPanelKey === currentPanelKey) return;
    currentPanelKey = nextPanelKey;
    setExpanded(expandedPanelKeys.has(nextPanelKey));
  });

  function toggleExpanded() {
    const nextExpanded = !expanded();
    if (nextExpanded) {
      expandedPanelKeys.add(panelKey());
    } else {
      expandedPanelKeys.delete(panelKey());
    }
    setExpanded(nextExpanded);
  }

  return (
    <Show when={changedFiles().length > 0 || usedBash()}>
      <div class="file-changes-panel is-node-output" classList={{ "is-collapsed": !expanded() }}>
        <button
          type="button"
          class="file-changes-panel-header"
          aria-expanded={expanded()}
          onClick={toggleExpanded}
        >
          <ChevronRight
            class="file-changes-chevron"
            classList={{ expanded: expanded() }}
            aria-hidden="true"
            size={14}
          />
          <span class="file-changes-panel-title">
            {panelSummaryLabel(fileCount(), changedFiles().length)}
          </span>
        </button>
        <Show when={expanded()}>
          <div class="file-changes-panel-body">
            <Show when={usedBash()}>
              <p class="file-changes-attribution-warning">
                <AlertTriangle size={14} aria-hidden="true" />
                Bash ran in this node. Shell, external tool, or MCP file writes may not appear
                here.
              </p>
            </Show>
            <Show when={changedFiles().length > 0}>
              <div class="file-changes-list">
                <For each={changedFiles()}>
                  {(record) => <FileChangeRow record={record} runId={runId()} />}
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

export function resetFileChangesPanelExpandStateForTests(): void {
  expandedPanelKeys.clear();
}
