import { createResource, For, Show } from "solid-js";
import { createUiDesktopOutboundAdapter } from "../../port";
import { useAppContext } from "../../context/AppContext";
import { prettyJson } from "../../lib/workflow";
import { isFileEditTool } from "./FileChangesPanel";

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

export function ToolApprovalCard() {
  const ctx = useAppContext();

  const [preview] = createResource(
    () => {
      const approval = ctx.selectedPendingApproval();
      if (!approval || !isFileEditTool(approval.toolCall.name)) {
        return null;
      }
      return {
        approvalId: approval.approvalId,
        toolName: approval.toolCall.name,
        arguments: approval.toolCall.arguments,
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
    const approval = ctx.selectedPendingApproval();
    if (!approval || !isFileEditTool(approval.toolCall.name)) {
      return true;
    }
    return !preview.loading;
  };

  const previewWarning = () => {
    const approval = ctx.selectedPendingApproval();
    if (!approval || !isFileEditTool(approval.toolCall.name) || preview.loading) {
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

  const pendingApprovals = () => ctx.runState()?.pendingApprovals ?? [];

  return (
    <Show when={pendingApprovals().length > 0}>
      <div class="tool-approval-stack">
        <Show when={pendingApprovals().length > 1}>
          <div class="tool-approval-queue">
            <div class="eyebrow">
              {pendingApprovals().length} approvals pending
            </div>
            <For each={pendingApprovals()}>
              {(item) => (
                <button
                  type="button"
                  class="tool-approval-queue-item"
                  classList={{
                    active: item.approvalId === ctx.selectedPendingApproval()?.approvalId,
                  }}
                  onClick={() => ctx.handleSelectNode(item.nodeId)}
                >
                  <span>{item.nodeLabel}</span>
                  <span>{item.toolCall.name}</span>
                </button>
              )}
            </For>
          </div>
        </Show>
        <Show when={ctx.selectedPendingApproval()}>
      {(approval) => (
        <div class="tool-approval-card">
          <div class="eyebrow">Approval required</div>
          <h3>{approval().toolCall.name}</h3>
          <p class="tool-approval-node">{approval().nodeLabel}</p>
          <Show when={approval().toolCall.intent}>
            {(intent) => <p class="tool-approval-intent">{intent()}</p>}
          </Show>

          <Show
            when={isFileEditTool(approval().toolCall.name)}
            fallback={
              <pre class="tool-approval-args">{prettyJson(approval().toolCall.arguments)}</pre>
            }
          >
            <div class="file-edit-preview">
              <div class="eyebrow">Preview</div>
              <Show when={preview.loading}>
                <p class="file-edit-preview-status">Computing diff…</p>
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
            {(message) => (
              <p class="file-edit-preview-warning">{message()}</p>
            )}
          </Show>

          <div class="tool-approval-actions">
            <button
              class="secondary-button"
              onClick={() => void ctx.handleToolApproval(false)}
            >
              Deny
            </button>
            <button
              class="primary-button"
              disabled={!canApproveFileEdit()}
              onClick={() => void ctx.handleToolApproval(true)}
            >
              Approve
            </button>
          </div>
        </div>
      )}
        </Show>
      </div>
    </Show>
  );
}
