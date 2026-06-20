import type { ApprovalMode, NodeToolConfig } from "../lib/types";

export function ToolConfigEditor(props: {
  config: NodeToolConfig;
  onApprovalModeChange: (value: ApprovalMode | null) => void;
}) {
  return (
    <div class="tool-config-body">
      <div class="field-grid tool-config-grid">
        <label>
          <span>Approval mode</span>
          <select
            class="text-input"
            value={props.config.approvalMode ?? "write"}
            onChange={(event) =>
              props.onApprovalModeChange(event.currentTarget.value as ApprovalMode)
            }
          >
            <option value="read_only">Read only</option>
            <option value="write">Read auto-approve, write prompt</option>
            <option value="always_ask">Always ask</option>
            <option value="yolo">Auto-approve all</option>
          </select>
        </label>
      </div>
    </div>
  );
}
