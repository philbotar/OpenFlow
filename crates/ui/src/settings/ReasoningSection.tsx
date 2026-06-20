import { createMemo, Show } from "solid-js";
import { TextSelect } from "../components/TextSelect";
import { useAppContext } from "../context/AppContext";
import {
  activeProfile,
  defaultReasoningBudgetTokens,
  defaultReasoningEffort,
  reasoningEffortOptions,
} from "../lib/workflow";

export function ReasoningSection() {
  const ctx = useAppContext();
  const effortOptions = createMemo(() => reasoningEffortOptions(ctx.activeProfileMemo()));
  const selectedEffort = createMemo(() => defaultReasoningEffort(ctx.activeProfileMemo()) ?? "");
  const selectedEffortOption = createMemo(() =>
    effortOptions().find((option) => option.value === selectedEffort()),
  );

  const effortSelectOptions = createMemo(() => [
    { value: "", label: "None (provider default)" },
    ...effortOptions().map((option) => ({ value: option.value, label: option.label })),
  ]);

  return (
    <Show when={effortOptions().length > 0}>
      <div class="settings-section">
        <div>
          <div class="eyebrow">Reasoning</div>
          <h3>Default reasoning effort</h3>
          <p>
            Applied to agent nodes that do not set their own effort level. Saved per provider.
          </p>
        </div>
        <label>
          <span>Reasoning effort</span>
          <TextSelect
            value={selectedEffort()}
            options={effortSelectOptions()}
            onChange={(event) =>
              void ctx.updateSettings((draft) => {
                const profile = activeProfile(draft);
                const nextValue = event.currentTarget.value;
                profile.default_reasoning_effort = nextValue || null;
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
                defaultReasoningBudgetTokens(ctx.activeProfileMemo())[selectedEffort()] ?? ""
              }
              onInput={(event) =>
                void ctx.updateSettings((draft) => {
                  const profile = activeProfile(draft);
                  const effort = selectedEffort();
                  if (!effort) return;
                  const parsed = Number.parseInt(event.currentTarget.value, 10);
                  if (!Number.isFinite(parsed) || parsed <= 0) return;
                  profile.default_reasoning_budget_tokens = {
                    ...defaultReasoningBudgetTokens(profile),
                    [effort]: parsed,
                  };
                })
              }
            />
          </label>
        </Show>
      </div>
    </Show>
  );
}
