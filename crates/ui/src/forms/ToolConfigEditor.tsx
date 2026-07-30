import { TextSelect } from "../components";
import type { ApprovalMode, NodeToolConfig } from "../lib/types";
import { APPROVAL_MODE_OPTIONS } from "./approvalModeOptions";

export function ToolConfigEditor(props: {
  config: NodeToolConfig;
  onApprovalModeChange: (value: ApprovalMode | null) => void;
}) {
  return (
    <div class="tool-config-body">
      <label>
        <span>Approval mode</span>
        <TextSelect
          value={props.config.approvalMode ?? "write"}
          options={APPROVAL_MODE_OPTIONS}
          onChange={(event) =>
            props.onApprovalModeChange(event.currentTarget.value as ApprovalMode)
          }
        />
      </label>
    </div>
  );
}
