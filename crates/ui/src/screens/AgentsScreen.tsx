import { For, Show } from "solid-js";
import { useAppContext } from "../context/AppContext";
import { AgentConfigForm } from "../forms/AgentConfigForm";
import { ToolConfigEditor } from "../forms/ToolConfigEditor";
import { SidebarIcon } from "../components/SidebarIcon";
import { prettyJson } from "../lib/workflow";

export function AgentsScreen() {
  const ctx = useAppContext();

  return (
    <section class="agents-screen">
      <div class="agents-layout">
        <aside class="agents-sidebar-panel">
          <div class="agents-sidebar-header">
            <div>
              <h3>Agents</h3>
            </div>
            <button
              class="sidebar-icon-button"
              aria-label="New agent"
              onClick={() => void ctx.handleCreateAgent()}
            >
              <SidebarIcon name="plus" />
            </button>
          </div>
          <div class="agent-definition-list">
            <Show
              when={ctx.agents().length > 0}
              fallback={
                <div class="empty-panel agents-empty-panel">No saved agents yet.</div>
              }
            >
              <For each={ctx.agents()}>
                {(agent) => (
                  <button
                    class="agent-list-row"
                    classList={{ active: agent.id === ctx.selectedAgentId() }}
                    onClick={() => ctx.setSelectedAgentId(agent.id)}
                  >
                    <span class="agent-list-row-title">
                      {agent.name || "Untitled agent"}
                    </span>
                  </button>
                )}
              </For>
            </Show>
          </div>
        </aside>

        <section class="agents-detail-panel">
          <Show
            when={ctx.selectedAgent()}
            fallback={
              <div class="empty-panel agents-detail-empty">
                Select an agent to edit its prompts, schema, and model.
              </div>
            }
          >
            {(agent) => (
              <div class="settings-section">
                <label>
                  <span>Name</span>
                  <input
                    class="text-input"
                    value={agent().name}
                    onInput={(event) =>
                      ctx.updateSelectedAgent((draft) => {
                        draft.name = event.currentTarget.value;
                      })
                    }
                  />
                </label>

                <AgentConfigForm
                  model={agent().model}
                  onModelChange={(value) =>
                    ctx.updateSelectedAgent((draft) => {
                      draft.model = value;
                    })
                  }
                  autoStart={agent().auto_start}
                  onAutoStartChange={(value) =>
                    ctx.updateSelectedAgent((draft) => {
                      draft.auto_start = value;
                    })
                  }
                  systemPrompt={agent().system_prompt}
                  onSystemPromptChange={(value) =>
                    ctx.updateSelectedAgent((draft) => {
                      draft.system_prompt = value;
                    })
                  }
                  taskPrompt={agent().task_prompt}
                  onTaskPromptChange={(value) =>
                    ctx.updateSelectedAgent((draft) => {
                      draft.task_prompt = value;
                    })
                  }
                  schemaJson={ctx.agentSchemaDraft()}
                  onSchemaChange={(value) => ctx.handleAgentSchemaInput(value)}
                  knownModels={ctx.activeProfileMemo().known_models}
                  defaultModel={ctx.activeProfileMemo().default_model}
                  listId="agent-model-list"
                />
                <ToolConfigEditor
                  config={agent().tools}
                  onToolEnabledChange={(toolName, enabled) =>
                    ctx.updateSelectedAgent((draft) => {
                      ctx.setToolEnabled(draft.tools, toolName, enabled);
                    })
                  }
                  onApprovalModeChange={(value) =>
                    ctx.updateSelectedAgent((draft) => {
                      draft.tools.approvalMode = value;
                    })
                  }
                  onMaxToolRoundsChange={(value) =>
                    ctx.updateSelectedAgent((draft) => {
                      draft.tools.maxToolRounds = Math.min(32, Math.max(1, value));
                    })
                  }
                />
                <div class="button-row end">
                  <button
                    class="primary-button"
                    onClick={() => void ctx.handleSaveAgents()}
                  >
                    Save
                  </button>
                </div>
              </div>
            )}
          </Show>
        </section>
      </div>
    </section>
  );
}
