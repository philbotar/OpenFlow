import { TextSelect } from "../components/TextSelect";
import type { ApprovalMode, NodeToolConfig } from "../lib/types";

const APPROVAL_MODE_OPTIONS: { value: ApprovalMode; label: string }[] = [
  { value: "read_only", label: "Read only" },
  { value: "write", label: "Read auto-approve, write prompt" },
  { value: "always_ask", label: "Always ask" },
  { value: "yolo", label: "Auto-approve all" },
];

export function ToolConfigEditor(props: {
  config: NodeToolConfig;
  onApprovalModeChange: (value: ApprovalMode | null) => void;
}) {
  return (
    <div class="tool-config-body">
      <div class="field-grid tool-config-grid">
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
    </div>
  );
}
