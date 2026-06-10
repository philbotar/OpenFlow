import { createMemo, For, Show } from "solid-js";
import { useAppContext } from "../context/AppContext";
import {
  activeProfile,
  defaultReasoningBudgetTokens,
  defaultReasoningEffort,
  reasoningEffortOptions,
} from "../lib/workflow";

export function SettingsScreen() {
  const ctx = useAppContext();
  const effortOptions = createMemo(() => reasoningEffortOptions(ctx.activeProfileMemo()));
  const selectedEffort = createMemo(() => defaultReasoningEffort(ctx.activeProfileMemo()) ?? "");
  const selectedEffortOption = createMemo(() =>
    effortOptions().find((option) => option.value === selectedEffort()),
  );

  return (
    <section class="settings-screen">
      <div class="settings-panel">
        <div class="settings-section">
          <div>
            <div class="eyebrow">Appearance</div>
            <h3>Theme</h3>
            <p>Choose light, dark, or match your system setting.</p>
          </div>
          <div class="theme-segment" role="group" aria-label="Theme preference">
            <button
              type="button"
              classList={{ "is-active": ctx.themePreference() === "system" }}
              onClick={() => ctx.handleSetThemePreference("system")}
            >
              System
            </button>
            <button
              type="button"
              classList={{ "is-active": ctx.themePreference() === "light" }}
              onClick={() => ctx.handleSetThemePreference("light")}
            >
              Light
            </button>
            <button
              type="button"
              classList={{ "is-active": ctx.themePreference() === "dark" }}
              onClick={() => ctx.handleSetThemePreference("dark")}
            >
              Dark
            </button>
          </div>
        </div>

        <div class="settings-section">
          <div>
            <div class="eyebrow">Authentication</div>
            <h3>Provider API key</h3>
            <p>
              Stored in plaintext in your local settings file for the selected provider.
              Protect this machine and settings file accordingly. Environment variables
              still act as fallback.
            </p>
          </div>
          <input
            type="password"
            value={ctx.activeProviderKeyInput()}
            onInput={(event) => ctx.handleApiKeyInput(event.currentTarget.value)}
            placeholder={ctx.readiness()?.envVar || "optional local provider key"}
            class="text-input"
          />
        </div>

        <div class="settings-section">
          <div>
            <div class="eyebrow">Provider</div>
            <h3>Execution transport</h3>
          </div>
          <label>
            <span>Provider</span>
            <select
              class="text-input"
              value={ctx.settings().active_provider}
              onChange={(event) =>
                void ctx.updateSettings((draft) => {
                  draft.active_provider = event.currentTarget.value;
                })
              }
            >
              <For each={ctx.providerIdsMemo()}>
                {(providerId) => (
                  <option value={providerId}>
                    {ctx.settings().providers[providerId]?.display_name ?? providerId}
                  </option>
                )}
              </For>
            </select>
          </label>
          <div class="field-grid">
            <label>
              <span>Base URL</span>
              <input
                class="text-input"
                value={ctx.activeProfileMemo().base_url}
                disabled={!ctx.activeProfileMemo().editable}
                onInput={(event) =>
                  void ctx.updateSettings((draft) => {
                    activeProfile(draft).base_url = event.currentTarget.value;
                  })
                }
              />
            </label>
            <label>
              <span>Transport</span>
              <select
                class="text-input"
                value={ctx.activeProfileMemo().transport}
                disabled={!ctx.activeProfileMemo().editable}
                onChange={(event) =>
                  void ctx.updateSettings((draft) => {
                    activeProfile(draft).transport = event.currentTarget.value as
                      | "responses"
                      | "chat_completions";
                  })
                }
              >
                <option value="responses">Responses API</option>
                <option value="chat_completions">Chat Completions API</option>
              </select>
            </label>
            <label>
              <span>Responses path</span>
              <input
                class="text-input"
                value={ctx.activeProfileMemo().responses_path}
                disabled={!ctx.activeProfileMemo().editable}
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
                disabled={!ctx.activeProfileMemo().editable}
                onInput={(event) =>
                  void ctx.updateSettings((draft) => {
                    activeProfile(draft).chat_completions_path = event.currentTarget.value;
                  })
                }
              />
            </label>
          </div>
        </div>

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
              <select
                class="text-input"
                value={selectedEffort()}
                onChange={(event) =>
                  void ctx.updateSettings((draft) => {
                    const profile = activeProfile(draft);
                    const nextValue = event.currentTarget.value;
                    profile.default_reasoning_effort = nextValue || null;
                  })
                }
              >
                <option value="">None (provider default)</option>
                <For each={effortOptions()}>
                  {(option) => <option value={option.value}>{option.label}</option>}
                </For>
              </select>
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

        <div class="settings-section">
          <div>
            <div class="eyebrow">Models</div>
            <h3>Known models for the active provider</h3>
          </div>
          <div class="chip-list">
            <For each={ctx.activeProfileMemo().known_models}>
              {(model) => (
                <button class="model-chip" onClick={() => ctx.handleRemoveKnownModel(model)}>
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
            <button class="secondary-button" onClick={ctx.handleAddKnownModel}>
              Add model
            </button>
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
          <button class="primary-button" onClick={() => void ctx.handleSaveSettings()}>
            Save settings
          </button>
        </div>
      </div>
    </section>
  );
}
