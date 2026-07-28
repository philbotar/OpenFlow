import { For, Show, createSignal } from "solid-js";
import {
  Button,
  ButtonRow,
  PanelEmptyState,
  SettingsSection,
  SidebarList,
  SidebarListRow,
  SidebarNavButton,
  Spinner,
} from "@/components";
import { useAppContext } from "../context/AppContext";
import { AgentConfigForm } from "../forms/AgentConfigForm";
import { ToolConfigEditor } from "../forms/ToolConfigEditor";

export function AgentsScreen() {
  const ctx = useAppContext();
  const [creatingWithAi, setCreatingWithAi] = createSignal(false);
  const [aiDescription, setAiDescription] = createSignal("");
  const [aiRequestPending, setAiRequestPending] = createSignal(false);

  const openAiCreator = () => {
    if (aiRequestPending()) return;
    setCreatingWithAi(true);
    ctx.setSelectedAgentId(null);
  };

  const selectAgent = (agentId: string) => {
    setCreatingWithAi(false);
    ctx.setSelectedAgentId(agentId);
  };

  const createAgentWithAi = async () => {
    const description = aiDescription().trim();
    if (!description || aiRequestPending() || ctx.readiness()?.ready !== true) return;
    setAiRequestPending(true);
    try {
      const created = await ctx.handleCreateAgentWithAi(description);
      if (created) {
        setAiDescription("");
        setCreatingWithAi(false);
      }
    } finally {
      setAiRequestPending(false);
    }
  };

  return (
    <section class="agents-screen">
      <div class="agents-layout">
        <aside class="agents-sidebar-panel">
          <SidebarList>
            <SidebarNavButton
              icon="plus"
              label="New agent"
              onClick={() => {
                setCreatingWithAi(false);
                void ctx.handleCreateAgent();
              }}
            />
            <SidebarNavButton
              icon="sparkles"
              label="Create with AI"
              active={creatingWithAi()}
              onClick={openAiCreator}
            />
            <Show
              when={ctx.agents().length > 0}
              fallback={
                <PanelEmptyState
                  title="No saved agents yet"
                  description="Use an option above to create your first reusable config."
                />
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
                      onSelect={() => selectAgent(agent.id)}
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
            when={!creatingWithAi()}
            fallback={
              <SettingsSection sectionClass="agent-ai-create">
                <h3>Create an agent with AI</h3>
                <p class="field-help">
                  Describe the reusable role, task, and result you need. AI generates the prompts
                  and output schema; you can edit everything after creation.
                </p>
                <label>
                  <span>What should this agent do?</span>
                  <textarea
                    class="text-area"
                    rows={8}
                    value={aiDescription()}
                    placeholder="Example: Review research notes, challenge unsupported claims, and return prioritized findings with evidence."
                    disabled={aiRequestPending()}
                    onInput={(event) => setAiDescription(event.currentTarget.value)}
                    onKeyDown={(event) => {
                      if ((event.metaKey || event.ctrlKey) && event.key === "Enter") {
                        event.preventDefault();
                        void createAgentWithAi();
                      }
                    }}
                  />
                </label>
                <Show when={ctx.readiness()?.ready === false ? ctx.readiness() : undefined}>
                  {(readiness) => (
                    <p class="agent-ai-create-warning" role="status">
                      {readiness().message}
                    </p>
                  )}
                </Show>
                <ButtonRow align="end">
                  <Button
                    variant="primary"
                    disabled={
                      aiDescription().trim().length === 0 ||
                      aiRequestPending() ||
                      ctx.readiness()?.ready !== true
                    }
                    onClick={() => void createAgentWithAi()}
                  >
                    <Show when={aiRequestPending()}>
                      <Spinner size="sm" />
                    </Show>
                    {aiRequestPending() ? "Creating…" : "Create Agent"}
                  </Button>
                </ButtonRow>
              </SettingsSection>
            }
          >
            <Show
              when={ctx.selectedAgent()}
              fallback={
                <PanelEmptyState
                  title="Select an agent"
                  description="Pick an agent from the list to edit prompts, schema, and model."
                />
              }
            >
              {(agent) => (
                <SettingsSection>
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
                    skills={ctx.availableSkills()}
                    schemaJson={ctx.agentSchemaDraft()}
                    onSchemaChange={(value) => ctx.handleAgentSchemaInput(value)}
                    knownModels={() => ctx.activeProfileMemo().known_models}
                    defaultModel={() => ctx.activeProfileMemo().default_model}
                  />
                  <ToolConfigEditor
                    config={agent().tools}
                    onApprovalModeChange={(value) =>
                      ctx.updateSelectedAgent((draft) => {
                        draft.tools.approvalMode = value;
                      })
                    }
                  />
                  <ButtonRow align="end">
                    <Button
                      variant="primary"
                      size="compact"
                      onClick={() => void ctx.handleSaveAgents()}
                    >
                      Save
                    </Button>
                  </ButtonRow>
                </SettingsSection>
              )}
            </Show>
          </Show>
        </section>
      </div>
    </section>
  );
}
