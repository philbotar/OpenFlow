import { createMemo, Show } from "solid-js";
import { AnimatedPanel, TextSelect } from "@/components";
import { useAppContext } from "../context/AppContext";
import {
  defaultReasoningBudgetTokens,
  defaultReasoningEffort,
  reasoningBudgetForEffort,
  reasoningEffortOptions,
  workflowReasoningBudgetTokens,
  workflowReasoningEffort,
} from "../lib/workflow";

export function WorkflowSettingsPanel() {
  const ctx = useAppContext();
  const effortOptions = createMemo(() => reasoningEffortOptions(ctx.activeProfileMemo()));
  const selectedEffort = createMemo(
    () => workflowReasoningEffort(ctx.activeWorkflow()?.settings ?? { shared_context: "" }) ?? "",
  );
  const selectedEffortOption = createMemo(() =>
    effortOptions().find((option) => option.value === selectedEffort()),
  );
  const providerDefaultLabel = createMemo(() => {
    const effort = defaultReasoningEffort(ctx.activeProfileMemo());
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

  return (
    <AnimatedPanel class="inspector-panel workflow-settings-panel">
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
                  reasoningBudgetForEffort(ctx.activeProfileMemo(), nextValue) ?? null;
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
                defaultReasoningBudgetTokens(ctx.activeProfileMemo())[selectedEffort()] ??
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

      <div class="settings-section workflow-settings-danger">
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
          <button
            type="button"
            class="danger-button"
            onClick={() => void ctx.handleDeleteActiveWorkflow()}
          >
            Delete workflow
          </button>
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
    </AnimatedPanel>
  );
}
