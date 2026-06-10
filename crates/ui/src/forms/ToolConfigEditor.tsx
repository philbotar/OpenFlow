import { createMemo, For } from "solid-js";
import { SUPPORTED_NODE_TOOLS } from "../lib/workflow";
import type { ApprovalMode, NodeToolConfig } from "../lib/types";

export function ToolConfigEditor(props: {
  config: NodeToolConfig;
  onToolEnabledChange: (toolName: string, enabled: boolean) => void;
  onApprovalModeChange: (value: ApprovalMode | null) => void;
}) {
  const enabledTools = createMemo(
    () => new Set(props.config.catalog.tools.map((tool) => tool.name)),
  );

  return (
    <div class="tool-config-body">
      <p class="field-help">Safe retrieval tools are enabled by default for this node.</p>
      <div class="tool-config-list" role="group" aria-label="Enabled node tools">
        <For each={SUPPORTED_NODE_TOOLS}>
          {(tool) => (
            <label class="tool-config-option">
              <span class="tool-config-option-copy">
                <span class="tool-config-option-title">{tool.name}</span>
                <span class="tool-config-option-description">{tool.description}</span>
              </span>
              <input
                type="checkbox"
                checked={enabledTools().has(tool.name)}
                onChange={(event) =>
                  props.onToolEnabledChange(tool.name, event.currentTarget.checked)
                }
              />
            </label>
          )}
        </For>
      </div>
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
            <option value="always_ask">Always ask</option>
            <option value="write">Read tools auto-approve</option>
            <option value="yolo">Read and write auto-approve</option>
          </select>
        </label>
      </div>
    </div>
  );
}
