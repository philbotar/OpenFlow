import { Show } from "solid-js";
import { useAppContext } from "../../context/AppContext";
import { prettyJson } from "../../lib/workflow";

export function ToolApprovalCard() {
  const ctx = useAppContext();

  return (
    <Show when={ctx.selectedPendingApproval()}>
      {(approval) => (
        <div class="inspector-card">
          <div class="eyebrow">Approval required</div>
          <h3>{approval().toolCall.name}</h3>
          <p>{approval().nodeLabel}</p>
          <pre>{prettyJson(approval().toolCall.arguments)}</pre>
          <div class="inspector-actions">
            <button
              class="secondary-button"
              onClick={() => void ctx.handleToolApproval(false)}
            >
              Deny
            </button>
            <button
              class="primary-button"
              onClick={() => void ctx.handleToolApproval(true)}
            >
              Approve
            </button>
          </div>
        </div>
      )}
    </Show>
  );
}
