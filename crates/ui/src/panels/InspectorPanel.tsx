import { createEffect, Show } from "solid-js";
import { useAppContext } from "../context/AppContext";
import { AgentConfigForm } from "../forms/AgentConfigForm";
import {
  agentReasoningBudgetTokens,
  agentReasoningEffort,
  defaultReasoningBudgetTokens,
  defaultReasoningEffort,
  reasoningEffortOptions,
  workflowReasoningEffort,
} from "@/lib/workflow";
import { CallableAgentsEditor } from "../forms/CallableAgentsEditor";
import { ToolConfigEditor } from "../forms/ToolConfigEditor";
import { InspectorSection, SidebarIcon } from "@/components";

export function InspectorPanel() {
  const ctx = useAppContext();
  let labelInput: HTMLInputElement | undefined;

  createEffect(() => {
    const nodeId = ctx.editingNodeId();
    if (!nodeId) return;
    queueMicrotask(() => {
      if (ctx.editingNodeId() !== nodeId || !labelInput) return;
      labelInput.focus();
      labelInput.setSelectionRange(0, labelInput.value.length);
    });
  });

  return (
    <aside class="inspector-panel panel-enter">
      <Show when={ctx.currentNode()}>
        {(node) => (
          <>
            <div class="panel-header">
              <div class="panel-header-copy">
                <div class="eyebrow">Inspector</div>
                <div class="panel-header-title-row">
                  <Show
                    when={ctx.editingNodeId() === node().id}
                    fallback={<h3>{node().label}</h3>}
                  >
                    <input
                      ref={labelInput}
                      class="text-input inspector-title-input"
                      value={ctx.nodeLabelDraft()}
                      onInput={(event) => ctx.setNodeLabelDraft(event.currentTarget.value)}
                      onBlur={ctx.handleCommitNodeLabel}
                      onKeyDown={(event) => {
                        if (event.key === "Enter") {
                          ctx.handleCommitNodeLabel();
                          return;
                        }
                        if (event.key === "Escape") {
                          ctx.handleCancelNodeLabelEdit();
                        }
                      }}
                      aria-label="Node label"
                    />
                  </Show>
                  <div class="panel-header-actions">
                    <button
                      class="inspector-action-button"
                      onClick={() =>
                        ctx.handleStartNodeLabelEdit(node().id, node().label)
                      }
                      title="Rename node"
                      aria-label={`Rename ${node().label}`}
                    >
                      <SidebarIcon name="edit" />
                    </button>
                    <button
                      class="inspector-delete-button"
                      onClick={ctx.handleDeleteSelectedNode}
                      title="Delete node"
                      aria-label={`Delete ${node().label}`}
                    >
                      <SidebarIcon name="trash" />
                    </button>
                  </div>
                </div>
              </div>
            </div>

            <InspectorSection title="Agent" defaultOpen>
              <AgentConfigForm
                model={node().agent.model}
                onModelChange={(value) =>
                  ctx.updateCurrentNode((nextNode) => {
                    nextNode.agent.model = value;
                  })
                }
                autoStart={node().agent.auto_start}
                onAutoStartChange={(value) =>
                  ctx.updateCurrentNode((nextNode) => {
                    nextNode.agent.auto_start = value;
                  })
                }
                systemPrompt={node().agent.system_prompt}
                onSystemPromptChange={(value) =>
                  ctx.updateCurrentNode((nextNode) => {
                    nextNode.agent.system_prompt = value;
                  })
                }
                taskPrompt={node().agent.task_prompt}
                onTaskPromptChange={(value) =>
                  ctx.updateCurrentNode((nextNode) => {
                    nextNode.agent.task_prompt = value;
                  })
                }
                schemaJson={ctx.schemaText()}
                onSchemaChange={(value) => ctx.setSchemaText(value)}
                knownModels={() => ctx.activeProfileMemo().known_models}
                defaultModel={() => ctx.activeProfileMemo().default_model}
                reasoningEffortOptions={reasoningEffortOptions(ctx.activeProfileMemo())}
                workflowDefaultReasoningEffort={workflowReasoningEffort(
                  ctx.activeWorkflow()?.settings ?? { shared_context: "" },
                )}
                providerDefaultReasoningEffort={defaultReasoningEffort(ctx.activeProfileMemo())}
                defaultReasoningBudgetTokens={defaultReasoningBudgetTokens(ctx.activeProfileMemo())}
                reasoningEffort={agentReasoningEffort(node().agent)}
                reasoningBudgetTokens={agentReasoningBudgetTokens(node().agent)}
                onReasoningEffortChange={(value) =>
                  ctx.updateCurrentNode((nextNode) => {
                    nextNode.agent.reasoning_effort = value;
                    nextNode.agent.reasoningEffort = value;
                    if (!value) {
                      nextNode.agent.reasoning_budget_tokens = null;
                      nextNode.agent.reasoningBudgetTokens = null;
                    }
                  })
                }
                onReasoningBudgetTokensChange={(value) =>
                  ctx.updateCurrentNode((nextNode) => {
                    nextNode.agent.reasoning_budget_tokens = value;
                    nextNode.agent.reasoningBudgetTokens = value;
                  })
                }
                systemPromptRows={8}
                taskPromptRows={5}
                showSchema={false}
              />
            </InspectorSection>

            <InspectorSection title="Output schema">
              <label>
                <span>JSON output schema</span>
                <textarea
                  class="text-area code"
                  rows={14}
                  value={ctx.schemaText()}
                  onInput={(event) => ctx.setSchemaText(event.currentTarget.value)}
                />
              </label>
              <div class="button-row">
                <button class="secondary-button" onClick={ctx.applySchemaEditor}>
                  Apply schema
                </button>
              </div>
            </InspectorSection>

            <InspectorSection title="Tools">
              <ToolConfigEditor
                config={node().agent.tools}
                onApprovalModeChange={(value) =>
                  ctx.updateCurrentNodeToolConfig((tools) => {
                    tools.approvalMode = value;
                  })
                }
              />
            </InspectorSection>

            <InspectorSection title="Callable agents">
              <CallableAgentsEditor
                allowAll={node().agent.allow_all_callable_agents ?? false}
                selectedIds={node().agent.callable_agents ?? []}
                agents={ctx.agents()}
                onAllowAllChange={(value) =>
                  ctx.updateCurrentNode((nextNode) => {
                    nextNode.agent.allow_all_callable_agents = value;
                    if (value) {
                      nextNode.agent.callable_agents = [];
                    }
                  })
                }
                onToggle={(agentId, enabled) =>
                  ctx.updateCurrentNode((nextNode) => {
                    if (nextNode.agent.allow_all_callable_agents) {
                      return;
                    }
                    const ids = new Set(nextNode.agent.callable_agents ?? []);
                    if (enabled) {
                      ids.add(agentId);
                    } else {
                      ids.delete(agentId);
                    }
                    nextNode.agent.callable_agents = [...ids];
                  })
                }
              />
            </InspectorSection>
          </>
        )}
      </Show>
    </aside>
  );
}
