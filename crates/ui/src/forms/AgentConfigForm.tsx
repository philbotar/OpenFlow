import { createEffect, createMemo, Show, type Accessor } from "solid-js";
import { TextSelect } from "@/components";
import type { ReasoningEffortOption } from "@/lib/types";

export function AgentConfigForm(props: {
  model: string;
  onModelChange: (value: string) => void;
  autoStart: boolean;
  onAutoStartChange: (value: boolean) => void;
  requestUserInput?: boolean;
  onRequestUserInputChange?: (value: boolean) => void;
  systemPrompt: string;
  onSystemPromptChange: (value: string) => void;
  taskPrompt: string;
  onTaskPromptChange: (value: string) => void;
  schemaJson: string;
  onSchemaChange: (value: string) => void;
  knownModels: Accessor<readonly string[]>;
  defaultModel: Accessor<string | null>;
  systemPromptRows?: number;
  taskPromptRows?: number;
  schemaRows?: number;
  reasoningEffortOptions?: readonly ReasoningEffortOption[];
  workflowDefaultReasoningEffort?: string | null;
  providerDefaultReasoningEffort?: string | null;
  defaultReasoningBudgetTokens?: Record<string, number>;
  reasoningEffort?: string | null;
  reasoningBudgetTokens?: number | null;
  onReasoningEffortChange?: (value: string | null) => void;
  onReasoningBudgetTokensChange?: (value: number | null) => void;
  showSchema?: boolean;
}) {
  const effectiveModel = () => props.model || props.defaultModel() || "";
  const modelSelectOptions = createMemo(() => {
    const models = props.knownModels();
    const options = models.map((model) => ({ value: model, label: model }));
    const current = props.model;
    if (current && !models.includes(current)) {
      options.unshift({ value: current, label: current });
    }
    return options;
  });
  const effortOptions = () => props.reasoningEffortOptions ?? [];
  const selectedEffort = () => props.reasoningEffort ?? "";
  const selectedEffortOption = createMemo(() =>
    effortOptions().find((option) => option.value === selectedEffort()),
  );
  const inheritedDefaultLabel = createMemo(() => {
    const workflowEffort = props.workflowDefaultReasoningEffort ?? null;
    if (workflowEffort) {
      const option = effortOptions().find((entry) => entry.value === workflowEffort);
      return option
        ? `Use workflow default (${option.label})`
        : `Use workflow default (${workflowEffort})`;
    }
    const effort = props.providerDefaultReasoningEffort ?? null;
    if (!effort) {
      return "None (provider default)";
    }
    const option = effortOptions().find((entry) => entry.value === effort);
    return option ? `Use provider default (${option.label})` : `Use provider default (${effort})`;
  });

  const effortSelectOptions = createMemo(() => [
    { value: "", label: inheritedDefaultLabel() },
    ...effortOptions().map((option) => ({ value: option.value, label: option.label })),
  ]);

  createEffect(() => {
    const defaultModel = props.defaultModel();
    if (!props.model && defaultModel) {
      props.onModelChange(defaultModel);
    }
  });

  return (
    <>
      <label>
        <span>Model</span>
        <TextSelect
          value={effectiveModel()}
          options={modelSelectOptions()}
          onChange={(event) => props.onModelChange(event.currentTarget.value)}
        />
      </label>
      <Show when={effortOptions().length > 0 && props.onReasoningEffortChange}>
        <label>
          <span>Reasoning effort</span>
          <TextSelect
            value={selectedEffort()}
            options={effortSelectOptions()}
            onChange={(event) => {
              const nextValue = event.currentTarget.value;
              if (!nextValue) {
                props.onReasoningEffortChange?.(null);
                props.onReasoningBudgetTokensChange?.(null);
                return;
              }
              props.onReasoningEffortChange?.(nextValue);
              const option = effortOptions().find((entry) => entry.value === nextValue);
              if (!option?.uses_budget_tokens) {
                props.onReasoningBudgetTokensChange?.(null);
                return;
              }
              if (props.reasoningBudgetTokens != null) {
                return;
              }
              const defaultBudget = props.defaultReasoningBudgetTokens?.[nextValue] ?? null;
              props.onReasoningBudgetTokensChange?.(defaultBudget);
            }}
          />
        </label>
        <Show when={selectedEffortOption()?.uses_budget_tokens}>
          <label>
            <span>Budget tokens</span>
            <input
              class="text-input"
              type="number"
              min={1}
              step={1}
              value={props.reasoningBudgetTokens ?? ""}
              onInput={(event) => {
                const parsed = Number.parseInt(event.currentTarget.value, 10);
                if (!Number.isFinite(parsed) || parsed <= 0) {
                  props.onReasoningBudgetTokensChange?.(null);
                  return;
                }
                props.onReasoningBudgetTokensChange?.(parsed);
              }}
            />
          </label>
        </Show>
      </Show>
      <label class="checkbox-row">
        <input
          type="checkbox"
          checked={props.autoStart}
          onChange={(event) => props.onAutoStartChange(event.currentTarget.checked)}
        />
        <span>Start automatically</span>
      </label>
      <Show when={props.onRequestUserInputChange}>
        <label class="checkbox-row">
          <input
            type="checkbox"
            checked={props.requestUserInput ?? false}
            onChange={(event) => props.onRequestUserInputChange?.(event.currentTarget.checked)}
          />
          <span>Allow follow-up questions</span>
        </label>
      </Show>
      <label>
        <span>System prompt</span>
        <textarea
          class="text-area"
          rows={props.systemPromptRows ?? 4}
          value={props.systemPrompt}
          onInput={(event) => props.onSystemPromptChange(event.currentTarget.value)}
        />
      </label>
      <label>
        <span>Task prompt</span>
        <textarea
          class="text-area"
          rows={props.taskPromptRows ?? 3}
          value={props.taskPrompt}
          onInput={(event) => props.onTaskPromptChange(event.currentTarget.value)}
        />
      </label>
      <Show when={props.showSchema !== false}>
        <label>
          <span>JSON output schema</span>
          <textarea
            class="text-area code"
            rows={props.schemaRows ?? 8}
            value={props.schemaJson}
            onInput={(event) => props.onSchemaChange(event.currentTarget.value)}
          />
        </label>
      </Show>
    </>
  );
}
