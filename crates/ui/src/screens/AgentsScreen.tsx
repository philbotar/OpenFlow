import { For, Show } from "solid-js";
import { useAppContext } from "../context/AppContext";
import { AgentConfigForm } from "../forms/AgentConfigForm";
import { ToolConfigEditor } from "../forms/ToolConfigEditor";
import { SidebarList, SidebarListRow, SidebarNavButton } from "../components/sidebar";

export function AgentsScreen() {
  const ctx = useAppContext();

  return (
    <section class="agents-screen">
      <div class="agents-layout">
        <aside class="agents-sidebar-panel">
          <SidebarList>
            <SidebarNavButton
              icon="plus"
              label="New agent"
              onClick={() => void ctx.handleCreateAgent()}
            />
            <Show
              when={ctx.agents().length > 0}
              fallback={
                <div class="empty-panel agents-empty-panel">No saved agents yet.</div>
              }
            >
              <For each={ctx.agents()}>
                {(agent) => {
                  const displayName = () => agent.name || "Untitled agent";
                  const editing = () => agent.id === ctx.editingAgentId();
                  return (
                    <SidebarListRow
                      title={displayName()}
                      active={agent.id === ctx.selectedAgentId()}
                      editing={editing()}
                      onSelect={() => ctx.setSelectedAgentId(agent.id)}
                      onRename={() =>
                        ctx.handleStartAgentNameEdit(agent.id, agent.name || "Untitled agent")
                      }
                      editSlot={
                        <input
                          ref={(el) => ctx.setAgentNameInputRef(el)}
                          value={ctx.agentNameDraft()}
                          onInput={(event) =>
                            ctx.setAgentNameDraft(event.currentTarget.value)
                          }
                          onBlur={ctx.handleAgentNameCommit}
                          onKeyDown={ctx.handleAgentNameKeyDown}
                          class="workflow-row-input"
                          aria-label={`Agent name for ${displayName()}`}
                        />
                      }
                    />
                  );
                }}
              </For>
            </Show>
          </SidebarList>
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
                  onApprovalModeChange={(value) =>
                    ctx.updateSelectedAgent((draft) => {
                      draft.tools.approvalMode = value;
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
