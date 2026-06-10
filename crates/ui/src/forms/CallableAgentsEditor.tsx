import { For, Show } from "solid-js";
import type { AgentDefinition } from "../lib/types";

export function CallableAgentsEditor(props: {
  allowAll: boolean;
  selectedIds: string[];
  agents: AgentDefinition[];
  onAllowAllChange: (value: boolean) => void;
  onToggle: (agentId: string, enabled: boolean) => void;
}) {
  const selected = () => new Set(props.selectedIds);
  const agentChecked = (agentId: string) => props.allowAll || selected().has(agentId);

  return (
    <div class="tool-config-body callable-agents-body">
      <p class="field-help">
        Saved agents this node may invoke during a run via{" "}
        <code>openflow_call_subagent</code>.
      </p>
      <label class="tool-config-option callable-agents-allow-all">
        <span class="tool-config-option-copy">
          <span class="tool-config-option-title">Allow all agents</span>
          <span class="tool-config-option-description">
            Snapshot every saved agent at run start. Individual selection is ignored.
          </span>
        </span>
        <input
          type="checkbox"
          checked={props.allowAll}
          onChange={(event) => props.onAllowAllChange(event.currentTarget.checked)}
        />
      </label>
      <Show
        when={props.agents.length > 0}
        fallback={
          <div class="empty-panel callable-agents-empty">
            No saved agents yet. Create one in the Agents screen.
          </div>
        }
      >
        <div
          class="tool-config-list"
          classList={{ "callable-agents-list-disabled": props.allowAll }}
          role="group"
          aria-label="Callable saved agents"
        >
          <For each={props.agents}>
            {(agent) => (
              <label class="tool-config-option">
                <span class="tool-config-option-copy">
                  <span class="tool-config-option-title">
                    {agent.name || "Untitled agent"}
                  </span>
                  <span class="tool-config-option-description">
                    {agent.model || "No model selected"}
                  </span>
                </span>
                <input
                  type="checkbox"
                  checked={agentChecked(agent.id)}
                  disabled={props.allowAll}
                  onChange={(event) =>
                    props.onToggle(agent.id, event.currentTarget.checked)
                  }
                />
              </label>
            )}
          </For>
        </div>
      </Show>
    </div>
  );
}
