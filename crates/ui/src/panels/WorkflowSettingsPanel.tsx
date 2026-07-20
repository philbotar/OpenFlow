import { createMemo, For, Show } from "solid-js";
import { Button, TextSelect } from "@/components";
import { useAppContext } from "../context/AppContext";
import {
  defaultReasoningBudgetTokens,
  defaultReasoningEffort,
  reasoningBudgetForEffort,
  reasoningEffortOptions,
  workflowProviderProfile,
  workflowReasoningBudgetTokens,
  workflowReasoningEffort,
} from "../lib/workflow";

export function WorkflowSettingsPanel() {
  const ctx = useAppContext();
  const workflowProfile = createMemo(() =>
    workflowProviderProfile(ctx.settings(), ctx.activeWorkflow()?.settings),
  );
  const effortOptions = createMemo(() => reasoningEffortOptions(workflowProfile()));
  const interactiveNodes = createMemo(() =>
    (ctx.activeWorkflow()?.nodes ?? []).filter((node) => node.agent.requestUserInput),
  );
  const planModeEnabled = createMemo(
    () => ctx.activeWorkflow()?.settings.planMode != null,
  );
  const selectedEffort = createMemo(
    () => workflowReasoningEffort(ctx.activeWorkflow()?.settings ?? { shared_context: "" }) ?? "",
  );
  const selectedEffortOption = createMemo(() =>
    effortOptions().find((option) => option.value === selectedEffort()),
  );
  const providerDefaultLabel = createMemo(() => {
    const effort = defaultReasoningEffort(workflowProfile());
    if (!effort) {
      return "Use provider default";
    }
    const option = effortOptions().find((entry) => entry.value === effort);
    return option ? `Use provider default (${option.label})` : `Use provider default (${effort})`;
  });

  const effortSelectOptions = createMemo(() => [
    { value: "", label: providerDefaultLabel() },
    ...effortOptions().map((option) => ({ value: option.value, label: option.label })),
  ]);

  const selectedOverseerModel = createMemo(
    () => ctx.activeWorkflow()?.settings.outputRepairModel ?? "",
  );
  const overseerModelOptions = createMemo(() => {
    const models = workflowProfile().known_models ?? [];
    const options = [
      { value: "", label: "Use worker model" },
      ...models.map((model) => ({ value: model, label: model })),
    ];
    const current = selectedOverseerModel();
    if (current && !models.includes(current)) {
      options.splice(1, 0, { value: current, label: current });
    }
    return options;
  });

  return (
    <aside class="inspector-panel workflow-settings-panel panel-enter">
      <div class="panel-header">
        <div class="panel-header-copy">
          <div class="eyebrow">Workflow</div>
          <h3>Settings</h3>
        </div>
      </div>

      <label>
        <span>Shared context</span>
        <p class="field-help">
          Shared context is injected into every node&apos;s system prompt at run time.
        </p>
        <textarea
          class="text-input"
          rows={12}
          value={ctx.activeWorkflow()?.settings.shared_context ?? ""}
          onInput={(event) =>
            ctx.updateActiveWorkflowSettings((settings) => {
              settings.shared_context = event.currentTarget.value;
            })
          }
        />
      </label>

      <section class="workflow-settings-section">
        <span>Plan → Execute</span>
        <p class="field-help">
          Block file edits while agents plan. Execution unlocks after the review node approves
          the plan.
        </p>
        <label class="checkbox-row">
          <input
            type="checkbox"
            checked={planModeEnabled()}
            disabled={interactiveNodes().length === 0}
            onChange={(event) =>
              ctx.updateActiveWorkflowSettings((settings) => {
                if (!event.currentTarget.checked) {
                  settings.planMode = null;
                  return;
                }
                settings.planMode = {
                  evidenceSourceNodeId: interactiveNodes()[0]?.id ?? "",
                };
              })
            }
          />
          <span>Require an approved plan before execution</span>
        </label>
        <Show when={interactiveNodes().length === 0}>
          <p class="field-help">
            Turn on <strong>Allow follow-up questions</strong> on a review node first.
          </p>
        </Show>
        <Show when={planModeEnabled()}>
          <label>
            <span>Review and freeze node</span>
            <select
              class="text-input"
              value={ctx.activeWorkflow()?.settings.planMode?.evidenceSourceNodeId ?? ""}
              onChange={(event) =>
                ctx.updateActiveWorkflowSettings((settings) => {
                  if (settings.planMode) {
                    settings.planMode.evidenceSourceNodeId = event.currentTarget.value;
                  }
                })
              }
            >
              <For each={interactiveNodes()}>
                {(node) => <option value={node.id}>{node.label}</option>}
              </For>
            </select>
          </label>
        </Show>
      </section>

      <Show when={effortOptions().length > 0}>
        <label>
          <span>Default reasoning effort</span>
          <p class="field-help">
            Applied to agent nodes that do not set their own effort level. Saved on this workflow.
          </p>
          <TextSelect
            value={selectedEffort()}
            options={effortSelectOptions()}
            onChange={(event) =>
              ctx.updateActiveWorkflowSettings((settings) => {
                const nextValue = event.currentTarget.value;
                settings.reasoning_effort = nextValue || null;
                settings.reasoningEffort = nextValue || null;
                if (!nextValue) {
                  settings.reasoning_budget_tokens = null;
                  settings.reasoningBudgetTokens = null;
                  return;
                }
                const option = effortOptions().find((entry) => entry.value === nextValue);
                if (!option?.uses_budget_tokens) {
                  settings.reasoning_budget_tokens = null;
                  settings.reasoningBudgetTokens = null;
                  return;
                }
                const existing = workflowReasoningBudgetTokens(settings);
                if (existing != null) {
                  return;
                }
                const budget =
                  reasoningBudgetForEffort(workflowProfile(), nextValue) ?? null;
                settings.reasoning_budget_tokens = budget;
                settings.reasoningBudgetTokens = budget;
              })
            }
          />
        </label>
        <Show when={selectedEffortOption()?.uses_budget_tokens}>
          <label>
            <span>Budget tokens for {selectedEffortOption()?.label}</span>
            <input
              class="text-input"
              type="number"
              min={1}
              step={1}
              value={
                workflowReasoningBudgetTokens(ctx.activeWorkflow()?.settings ?? {
                  shared_context: "",
                }) ??
                defaultReasoningBudgetTokens(workflowProfile())[selectedEffort()] ??
                ""
              }
              onInput={(event) => {
                const parsed = Number.parseInt(event.currentTarget.value, 10);
                ctx.updateActiveWorkflowSettings((settings) => {
                  if (!Number.isFinite(parsed) || parsed <= 0) {
                    settings.reasoning_budget_tokens = null;
                    settings.reasoningBudgetTokens = null;
                    return;
                  }
                  settings.reasoning_budget_tokens = parsed;
                  settings.reasoningBudgetTokens = parsed;
                });
              }}
            />
          </label>
        </Show>
      </Show>

      <label>
        <span>Overseer model</span>
        <p class="field-help">
          Optional model on this workflow&apos;s provider that repairs malformed final output before
          the normal retry path. Not a workflow node. Blank uses each worker request&apos;s model.
        </p>
        <TextSelect
          value={selectedOverseerModel()}
          options={overseerModelOptions()}
          onChange={(event) =>
            ctx.updateActiveWorkflowSettings((settings) => {
              const nextValue = event.currentTarget.value.trim();
              settings.outputRepairModel = nextValue || null;
            })
          }
        />
      </label>

      <label>
        <span>Max retry attempts</span>
        <p class="field-help">
          Automatic retries for transient model failures (rate limits, timeouts). 0 disables
          auto-retry.
        </p>
        <input
          class="text-input"
          type="number"
          min={0}
          max={10}
          step={1}
          value={ctx.activeWorkflow()?.settings.retry_policy?.max_attempts ?? 3}
          onInput={(event) => {
            const parsed = Number.parseInt(event.currentTarget.value, 10);
            ctx.updateActiveWorkflowSettings((settings) => {
              settings.retry_policy = {
                ...(settings.retry_policy ?? { max_attempts: 3, backoff_ms: 1_000 }),
                max_attempts: Number.isFinite(parsed)
                  ? Math.min(10, Math.max(0, parsed))
                  : 0,
              };
            });
          }}
        />
      </label>

      <label>
        <span>Retry backoff (ms)</span>
        <p class="field-help">
          Base delay before the first retry. Doubles each attempt, capped at 30 seconds.
        </p>
        <input
          class="text-input"
          type="number"
          min={0}
          step={100}
          value={ctx.activeWorkflow()?.settings.retry_policy?.backoff_ms ?? 1_000}
          onInput={(event) => {
            const parsed = Number.parseInt(event.currentTarget.value, 10);
            ctx.updateActiveWorkflowSettings((settings) => {
              settings.retry_policy = {
                ...(settings.retry_policy ?? { max_attempts: 3, backoff_ms: 1_000 }),
                backoff_ms: Number.isFinite(parsed) ? Math.max(0, parsed) : 0,
              };
            });
          }}
        />
      </label>

      <div class="workflow-settings-danger">
        <span>Danger zone</span>
        <p class="field-help">Permanently delete this workflow and its settings.</p>
        <Show
          when={
            !(
              ctx.runState()?.active &&
              ctx.backendRunWorkflowId() === ctx.activeWorkflow()?.id
            )
          }
        >
          <Button variant="danger" onClick={() => void ctx.handleDeleteActiveWorkflow()}>
            Delete workflow
          </Button>
        </Show>
        <Show
          when={
            ctx.runState()?.active &&
            ctx.backendRunWorkflowId() === ctx.activeWorkflow()?.id
          }
        >
          <p class="field-help">Stop the active run before deleting this workflow.</p>
        </Show>
      </div>
    </aside>
  );
}
