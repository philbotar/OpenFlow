import { createSignal, For, Show } from "solid-js";
import type { AgentDefinition } from "../lib/types";

export function CallableAgentsEditor(props: {
  allowAll: boolean;
  selectedIds: string[];
  agents: AgentDefinition[];
  onAllowAllChange: (value: boolean) => void;
  onToggle: (agentId: string, enabled: boolean) => void;
  defaultOpen?: boolean;
}) {
  const [open, setOpen] = createSignal(props.defaultOpen ?? false);
  const selected = () => new Set(props.selectedIds);
  const agentChecked = (agentId: string) => props.allowAll || selected().has(agentId);

  return (
    <section class="tool-config-section callable-agents-section">
      <div class="tool-config-header">
        <div class="tool-config-header-copy">
          <div class="eyebrow">Callable agents</div>
          <p>
            Saved agents this node may invoke during a run via{" "}
            <code>openflow_call_subagent</code>.
          </p>
        </div>
        <button
          type="button"
          class="secondary-button tool-config-toggle"
          onClick={() => setOpen((value) => !value)}
          aria-expanded={open()}
        >
          {open() ? "Hide agents" : "Show agents"}
        </button>
      </div>
      <Show when={open()}>
        <div class="tool-config-body">
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
      </Show>
    </section>
  );
}
