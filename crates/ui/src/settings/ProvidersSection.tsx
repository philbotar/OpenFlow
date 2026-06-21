import { createMemo, createSignal, For, Show } from "solid-js";
import { refreshBedrockModels } from "../api";
import { TextSelect } from "../components/TextSelect";
import { useAppContext } from "../context/AppContext";
import {
  activeProfile,
  defaultReasoningBudgetTokens,
  defaultReasoningEffort,
  reasoningEffortOptions,
} from "../lib/workflow";

export function ProvidersSection() {
  const ctx = useAppContext();
  const providerOptions = createMemo(() =>
    ctx.providerIdsMemo().map((providerId) => ({
      value: providerId,
      label: ctx.settings().providers[providerId]?.display_name ?? providerId,
    })),
  );
  const effortOptions = createMemo(() => reasoningEffortOptions(ctx.activeProfileMemo()));
  const selectedEffort = createMemo(() => defaultReasoningEffort(ctx.activeProfileMemo()) ?? "");
  const selectedEffortOption = createMemo(() =>
    effortOptions().find((option) => option.value === selectedEffort()),
  );
  const effortSelectOptions = createMemo(() => [
    { value: "", label: "None (provider default)" },
    ...effortOptions().map((option) => ({ value: option.value, label: option.label })),
  ]);
  const transportOptions = [
    { value: "responses", label: "Responses API" },
    { value: "chat_completions", label: "Chat Completions API" },
  ] as const;
  const profileEditable = () => ctx.activeProfileMemo().editable;
  const isBedrock = () => ctx.settings().active_provider === "bedrock";
  const [refreshingModels, setRefreshingModels] = createSignal(false);

  async function handleRefreshBedrockModels() {
    setRefreshingModels(true);
    try {
      const models = await refreshBedrockModels(ctx.settings());
      await ctx.updateSettings((draft) => {
        const profile = activeProfile(draft);
        profile.known_models = models;
        if (
          profile.default_model &&
          !models.includes(profile.default_model)
        ) {
          profile.default_model = models[0] ?? null;
        } else if (!profile.default_model) {
          profile.default_model = models[0] ?? null;
        }
      });
    } finally {
      setRefreshingModels(false);
    }
  }

  return (
    <div class="settings-section providers-section">
      <header class="providers-section-header">
        <div class="providers-section-intro">
          <div class="eyebrow">Providers</div>
          <h3>AI provider configuration</h3>
          <p>Choose a provider, authenticate, and manage models for workflow runs.</p>
        </div>
        <div class="readiness-chip" classList={{ ready: ctx.readiness()?.ready }}>
          <span class="status-dot" aria-hidden="true" />
          <span>{ctx.readiness()?.message ?? "Checking provider"}</span>
        </div>
      </header>

      <section class="settings-subsection" aria-labelledby="providers-active-heading">
        <h3 id="providers-active-heading" class="settings-subheading">
          Active provider
        </h3>
        <label>
          <span>Provider</span>
          <TextSelect
            value={ctx.settings().active_provider}
            options={providerOptions()}
            onChange={(event) =>
              void ctx.updateSettings((draft) => {
                draft.active_provider = event.currentTarget.value;
              })
            }
          />
        </label>
      </section>

      <section class="settings-subsection" aria-labelledby="providers-auth-heading">
        <h3 id="providers-auth-heading" class="settings-subheading">
          {isBedrock() ? "AWS credentials" : "API key"}
        </h3>
        <Show
          when={!isBedrock()}
          fallback={
            <p>
              Uses the AWS credential chain (env vars, shared config, SSO, instance role). Optionally
              set an AWS profile name below; otherwise <code>AWS_PROFILE</code> applies.
            </p>
          }
        >
          <p>
            Stored in plaintext in your local settings file for the selected provider. Protect this
            machine and settings file accordingly. Environment variables still act as fallback.
          </p>
        </Show>
        <input
          type={isBedrock() ? "text" : "password"}
          value={ctx.activeProviderKeyInput()}
          onInput={(event) => ctx.handleApiKeyInput(event.currentTarget.value)}
          placeholder={
            isBedrock()
              ? "AWS profile (optional)"
              : ctx.readiness()?.envVar || "optional local provider key"
          }
          class="text-input"
        />
      </section>

      <section class="settings-subsection" aria-labelledby="providers-connection-heading">
        <h3 id="providers-connection-heading" class="settings-subheading">
          Connection
        </h3>
        <Show when={!profileEditable()}>
          <p>Managed provider — connection settings are fixed.</p>
        </Show>
        <div class="field-grid">
          <label>
            <span>{isBedrock() ? "AWS region" : "Base URL"}</span>
            <input
              class="text-input"
              value={ctx.activeProfileMemo().base_url}
              disabled={!profileEditable() && !isBedrock()}
              onInput={(event) =>
                void ctx.updateSettings((draft) => {
                  activeProfile(draft).base_url = event.currentTarget.value;
                })
              }
            />
          </label>
          <Show when={!isBedrock()}>
            <label>
              <span>Transport</span>
              <TextSelect
                value={ctx.activeProfileMemo().transport}
                options={transportOptions}
                disabled={!profileEditable()}
                onChange={(event) =>
                  void ctx.updateSettings((draft) => {
                    activeProfile(draft).transport = event.currentTarget.value as
                      | "responses"
                      | "chat_completions";
                  })
                }
              />
            </label>
            <label>
              <span>Responses path</span>
              <input
                class="text-input"
                value={ctx.activeProfileMemo().responses_path}
                disabled={!profileEditable()}
                onInput={(event) =>
                  void ctx.updateSettings((draft) => {
                    activeProfile(draft).responses_path = event.currentTarget.value;
                  })
                }
              />
            </label>
            <label>
              <span>Chat completions path</span>
              <input
                class="text-input"
                value={ctx.activeProfileMemo().chat_completions_path}
                disabled={!profileEditable()}
                onInput={(event) =>
                  void ctx.updateSettings((draft) => {
                    activeProfile(draft).chat_completions_path = event.currentTarget.value;
                  })
                }
              />
            </label>
          </Show>
        </div>
      </section>

      <Show when={effortOptions().length > 0}>
        <section class="settings-subsection" aria-labelledby="providers-reasoning-heading">
          <h3 id="providers-reasoning-heading" class="settings-subheading">
            Reasoning defaults
          </h3>
          <p>
            Applied to agent nodes that do not set their own effort level. Saved per provider.
          </p>
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
        </section>
      </Show>

      <section class="settings-subsection" aria-labelledby="providers-models-heading">
        <h3 id="providers-models-heading" class="settings-subheading">
          Models
        </h3>
        <div class="chip-list">
          <For each={ctx.activeProfileMemo().known_models}>
            {(model) => (
              <button type="button" class="model-chip" onClick={() => ctx.handleRemoveKnownModel(model)}>
                {model}
                <span>×</span>
              </button>
            )}
          </For>
        </div>
        <div class="inline-form">
          <input
            class="text-input"
            placeholder="Add model"
            value={ctx.newModelInputByProvider()[ctx.settings().active_provider] ?? ""}
            onInput={(event) =>
              ctx.setNewModelInputByProvider((current) => ({
                ...current,
                [ctx.settings().active_provider]: event.currentTarget.value,
              }))
            }
          />
          <button type="button" class="secondary-button" onClick={ctx.handleAddKnownModel}>
            Add model
          </button>
          <Show when={isBedrock()}>
            <button
              type="button"
              class="secondary-button"
              disabled={refreshingModels()}
              onClick={() => void handleRefreshBedrockModels()}
            >
              {refreshingModels() ? "Refreshing…" : "Refresh from AWS"}
            </button>
          </Show>
        </div>
        <label>
          <span>Default model</span>
          <input
            class="text-input"
            list="known-models-settings"
            value={ctx.activeProfileMemo().default_model ?? ""}
            onInput={(event) =>
              void ctx.updateSettings((draft) => {
                activeProfile(draft).default_model = event.currentTarget.value || null;
              })
            }
          />
          <datalist id="known-models-settings">
            <For each={ctx.activeProfileMemo().known_models}>
              {(model) => <option value={model} />}
            </For>
          </datalist>
        </label>
      </section>

      <footer class="settings-save-bar">
        <p class="settings-save-hint">Saves API key and provider profile to local settings.</p>
        <button type="button" class="primary-button" onClick={() => void ctx.handleSaveSettings()}>
          Save settings
        </button>
      </footer>
    </div>
  );
}
