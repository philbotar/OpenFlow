import { createEffect, createMemo, For, Show } from "solid-js";
import type { ReasoningEffortOption } from "../lib/types";

export function AgentConfigForm(props: {
  model: string;
  onModelChange: (value: string) => void;
  autoStart: boolean;
  onAutoStartChange: (value: boolean) => void;
  systemPrompt: string;
  onSystemPromptChange: (value: string) => void;
  taskPrompt: string;
  onTaskPromptChange: (value: string) => void;
  schemaJson: string;
  onSchemaChange: (value: string) => void;
  knownModels: readonly string[];
  defaultModel: string | null;
  listId: string;
  systemPromptRows?: number;
  taskPromptRows?: number;
  schemaRows?: number;
  reasoningEffortOptions?: readonly ReasoningEffortOption[];
  providerDefaultReasoningEffort?: string | null;
  defaultReasoningBudgetTokens?: Record<string, number>;
  reasoningEffort?: string | null;
  reasoningBudgetTokens?: number | null;
  onReasoningEffortChange?: (value: string | null) => void;
  onReasoningBudgetTokensChange?: (value: number | null) => void;
  showSchema?: boolean;
}) {
  const effectiveModel = () => props.model || props.defaultModel || "";
  const effortOptions = () => props.reasoningEffortOptions ?? [];
  const selectedEffort = () => props.reasoningEffort ?? "";
  const selectedEffortOption = createMemo(() =>
    effortOptions().find((option) => option.value === selectedEffort()),
  );
  const providerDefaultLabel = createMemo(() => {
    const effort = props.providerDefaultReasoningEffort ?? null;
    if (!effort) {
      return "Use provider default";
    }
    const option = effortOptions().find((entry) => entry.value === effort);
    return option ? `Use provider default (${option.label})` : `Use provider default (${effort})`;
  });

  createEffect(() => {
    if (!props.model && props.defaultModel) {
      props.onModelChange(props.defaultModel);
    }
  });

  return (
    <>
      <label>
        <span>Model</span>
        <input
          class="text-input"
          value={effectiveModel()}
          list={props.listId}
          onInput={(event) => props.onModelChange(event.currentTarget.value)}
        />
        <datalist id={props.listId}>
          <For each={props.knownModels}>{(model) => <option value={model} />}</For>
        </datalist>
      </label>
      <Show when={effortOptions().length > 0 && props.onReasoningEffortChange}>
        <label>
          <span>Reasoning effort</span>
          <select
            class="text-input"
            value={selectedEffort()}
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
          >
            <option value="">{providerDefaultLabel()}</option>
            <For each={effortOptions()}>
              {(option) => <option value={option.value}>{option.label}</option>}
            </For>
          </select>
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
          checked={!props.autoStart}
          onChange={(event) => props.onAutoStartChange(!event.currentTarget.checked)}
        />
        <span>Request user input</span>
      </label>
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
