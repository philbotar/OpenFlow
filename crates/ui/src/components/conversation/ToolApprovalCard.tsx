import { createResource, For, Show } from "solid-js";
import { createUiDesktopOutboundAdapter } from "../../port";
import type { PendingToolApproval } from "../../lib/types";
import { Spinner } from "../Spinner";
import { isFileEditTool } from "./FileChangesPanel";
import { formatToolDisplayName } from "./toolBubbleState";

const desktop = createUiDesktopOutboundAdapter();

function opLabel(op: string): string {
  switch (op) {
    case "create":
      return "Create";
    case "update":
      return "Update";
    case "delete":
      return "Delete";
    case "rename":
      return "Rename";
    default:
      return op;
  }
}

export function ToolApprovalCardBody(props: {
  approval: PendingToolApproval;
  onApprove: (allow: boolean) => void;
}) {
  const [preview] = createResource(
    () => {
      if (!isFileEditTool(props.approval.toolCall.name)) {
        return null;
      }
      return {
        approvalId: props.approval.approvalId,
        toolName: props.approval.toolCall.name,
        arguments: props.approval.toolCall.arguments,
      };
    },
    async (input) => {
      if (!input) {
        return null;
      }
      try {
        return await desktop.previewFileEdit(
          input.approvalId,
          input.toolName,
          input.arguments,
        );
      } catch (error) {
        return {
          entries: [],
          error: error instanceof Error ? error.message : String(error),
        };
      }
    },
  );

  const canApproveFileEdit = () => {
    if (!isFileEditTool(props.approval.toolCall.name)) {
      return true;
    }
    return !preview.loading;
  };

  const previewWarning = () => {
    if (!isFileEditTool(props.approval.toolCall.name) || preview.loading) {
      return null;
    }
    if (preview.error) {
      return String(preview.error);
    }
    const result = preview();
    if (result?.error) {
      return result.error;
    }
    if ((result?.entries?.length ?? 0) === 0) {
      return "Preview returned no diff. You can still approve, but review the tool arguments first.";
    }
    return null;
  };

  return (
    <div class="tool-approval-card">
      <div class="eyebrow">Approval required</div>
      <h3>{formatToolDisplayName(props.approval.toolCall.name)}</h3>
      <p class="tool-approval-node">{props.approval.nodeLabel}</p>

      <Show
        when={isFileEditTool(props.approval.toolCall.name)}
        fallback={
          <pre class="tool-approval-args">{JSON.stringify(props.approval.toolCall.arguments, null, 2)}</pre>
        }
      >
        <div class="file-edit-preview">
          <div class="eyebrow">Preview</div>
          <Show when={preview.loading}>
            <p class="file-edit-preview-status loading-inline">
              <Spinner size="sm" />
              Computing diff…
            </p>
          </Show>
          <Show when={preview.error}>
            <p class="file-edit-preview-error">{String(preview.error)}</p>
          </Show>
          <Show when={preview()?.error}>
            {(message) => <p class="file-edit-preview-error">{message()}</p>}
          </Show>
          <Show when={preview()?.entries?.length}>
            <For each={preview()!.entries}>
              {(entry) => (
                <div class="file-edit-preview-entry">
                  <div class="file-edit-preview-header">
                    <span class="file-change-op">{opLabel(entry.op)}</span>
                    <span class="file-change-path">{entry.path}</span>
                    <Show when={entry.renameTo}>
                      {(renameTo) => (
                        <span class="file-change-rename">→ {renameTo()}</span>
                      )}
                    </Show>
                  </div>
                  <pre class="file-edit-diff">{entry.diff}</pre>
                </div>
              )}
            </For>
          </Show>
        </div>
      </Show>

      <Show when={previewWarning()}>
        {(message) => <p class="file-edit-preview-warning">{message()}</p>}
      </Show>

      <div class="tool-approval-actions">
        <button
          class="secondary-button"
          onClick={() => props.onApprove(false)}
        >
          Deny
        </button>
        <button
          class="primary-button"
          disabled={!canApproveFileEdit()}
          onClick={() => props.onApprove(true)}
        >
          Approve
        </button>
      </div>
    </div>
  );
}
