import { createMemo, Show } from "solid-js";
import { TextSelect } from "@/components";
import { useAppContext } from "../../context/AppContext";
import { APPROVAL_MODE_OPTIONS } from "../../forms/approvalModeOptions";
import {
  agentReasoningBudgetTokens,
  agentReasoningEffort,
  defaultReasoningBudgetTokens,
  defaultReasoningEffort,
  reasoningEffortOptions,
  workflowReasoningEffort,
} from "@/lib/workflow";
import type { NodeId } from "../../lib/types";

export function ComposerRuntimeControls(props: { nodeId: NodeId; disabled?: boolean }) {
  const ctx = useAppContext();
  const node = createMemo(() =>
    ctx.activeWorkflow()?.nodes.find((entry) => entry.id === props.nodeId),
  );
  const effortOptions = createMemo(() => reasoningEffortOptions(ctx.activeProfileMemo()));
  const selectedEffort = createMemo(() => {
    const current = node();
    return current ? agentReasoningEffort(current.agent) ?? "" : "";
  });
  const selectedEffortOption = createMemo(() =>
    effortOptions().find((option) => option.value === selectedEffort()),
  );
  const inheritedDefaultLabel = createMemo(() => {
    const workflowEffort = workflowReasoningEffort(
      ctx.activeWorkflow()?.settings ?? { shared_context: "" },
    );
    if (workflowEffort) {
      const option = effortOptions().find((entry) => entry.value === workflowEffort);
      return option ? option.label : workflowEffort;
    }
    const effort = defaultReasoningEffort(ctx.activeProfileMemo());
    if (!effort) {
      return "Default";
    }
    const option = effortOptions().find((entry) => entry.value === effort);
    return option ? option.label : effort;
  });
  const effortSelectOptions = createMemo(() => [
    { value: "", label: inheritedDefaultLabel() },
    ...effortOptions().map((option) => ({ value: option.value, label: option.label })),
  ]);
  const controlsDisabled = () =>
    props.disabled || !!ctx.replayRunId() || !ctx.runState()?.active;

  return (
    <Show when={node()}>
      {(currentNode) => (
        <div class="composer-runtime-controls" aria-label="Node runtime settings">
          <TextSelect
            class="composer-runtime-select"
            menuPlacement="above"
            value={currentNode().agent.tools.approvalMode ?? "write"}
            options={APPROVAL_MODE_OPTIONS}
            disabled={controlsDisabled()}
            aria-label="Tool approval mode"
            onChange={(event) => {
              void ctx.handleUpdateNodeRuntimeConfig(props.nodeId, {
                approvalMode: event.currentTarget.value as typeof APPROVAL_MODE_OPTIONS[number]["value"],
              });
            }}
          />
          <Show when={effortOptions().length > 0}>
            <TextSelect
              class="composer-runtime-select"
              menuPlacement="above"
              value={selectedEffort()}
              options={effortSelectOptions()}
              disabled={controlsDisabled()}
              aria-label="Reasoning effort"
              onChange={(event) => {
                const nextValue = event.currentTarget.value;
                if (!nextValue) {
                  void ctx.handleUpdateNodeRuntimeConfig(props.nodeId, {
                    reasoningEffort: null,
                    reasoningBudgetTokens: null,
                  });
                  return;
                }
                const option = effortOptions().find((entry) => entry.value === nextValue);
                const defaultBudget =
                  defaultReasoningBudgetTokens(ctx.activeProfileMemo())[nextValue] ?? null;
                void ctx.handleUpdateNodeRuntimeConfig(props.nodeId, {
                  reasoningEffort: nextValue,
                  reasoningBudgetTokens: option?.uses_budget_tokens
                    ? agentReasoningBudgetTokens(currentNode().agent) ?? defaultBudget
                    : null,
                });
              }}
            />
            <Show when={selectedEffortOption()?.uses_budget_tokens}>
              <input
                class="composer-runtime-budget"
                type="number"
                min={1}
                step={1}
                disabled={controlsDisabled()}
                aria-label="Reasoning budget tokens"
                value={agentReasoningBudgetTokens(currentNode().agent) ?? ""}
                onInput={(event) => {
                  const parsed = Number.parseInt(event.currentTarget.value, 10);
                  void ctx.handleUpdateNodeRuntimeConfig(props.nodeId, {
                    reasoningBudgetTokens:
                      Number.isFinite(parsed) && parsed > 0 ? parsed : null,
                  });
                }}
              />
            </Show>
          </Show>
        </div>
      )}
    </Show>
  );
}
