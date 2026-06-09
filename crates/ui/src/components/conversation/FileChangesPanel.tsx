import { createSignal, For, Show } from "solid-js";
import { useAppContext } from "../../context/AppContext";
import type { FileChangeRecord } from "../../lib/types";

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

function effectiveChangePath(record: FileChangeRecord): string {
  if (record.op === "rename" && record.renameTo) {
    return record.renameTo;
  }
  return record.path;
}

function latestChangesByPath(records: FileChangeRecord[]): FileChangeRecord[] {
  const byPath = new Map<string, FileChangeRecord>();
  for (const record of records) {
    if (record.op === "rename") {
      const stale = byPath.get(record.path);
      if (!stale || record.timestampMs >= stale.timestampMs) {
        byPath.delete(record.path);
      }
    }
    const key = effectiveChangePath(record);
    const existing = byPath.get(key);
    if (!existing || record.timestampMs >= existing.timestampMs) {
      byPath.set(key, record);
    }
  }
  return [...byPath.values()];
}

function FileChangeRow(props: { record: FileChangeRecord }) {
  const [open, setOpen] = createSignal(Boolean(props.record.diffSummary));

  return (
    <div class="file-change-row">
      <button
        type="button"
        class="file-change-row-header"
        disabled={!props.record.diffSummary}
        onClick={() => setOpen((value) => !value)}
      >
        <span class="file-change-op">{opLabel(props.record.op)}</span>
        <span class="file-change-path">{props.record.path}</span>
        <Show when={props.record.renameTo}>
          {(renameTo) => <span class="file-change-rename">→ {renameTo()}</span>}
        </Show>
        <Show when={props.record.diffSummary}>
          <span class="file-change-toggle">{open() ? "Hide diff" : "Show diff"}</span>
        </Show>
      </button>
      <Show when={open() && props.record.diffSummary}>
        <pre class="file-edit-diff">{props.record.diffSummary}</pre>
      </Show>
    </div>
  );
}

export function FileChangesPanel() {
  const ctx = useAppContext();
  const changedFiles = () => latestChangesByPath(ctx.runState()?.changedFiles ?? []);

  return (
    <Show when={changedFiles().length > 0}>
      <div class="file-changes-panel">
        <div class="eyebrow">Changed files</div>
        <div class="file-changes-list">
          <For each={changedFiles()}>{(record) => <FileChangeRow record={record} />}</For>
        </div>
      </div>
    </Show>
  );
}

export function isFileEditTool(name: string): boolean {
  return EDIT_TOOLS.has(name);
}
