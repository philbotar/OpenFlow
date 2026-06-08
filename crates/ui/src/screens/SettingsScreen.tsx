import { For } from "solid-js";
import { useAppContext } from "../context/AppContext";
import { activeProfile } from "../lib/workflow";

export function SettingsScreen() {
  const ctx = useAppContext();

  return (
    <section class="settings-screen">
      <div class="settings-panel">
        <div class="settings-section">
          <div>
            <div class="eyebrow">Authentication</div>
            <h3>Provider API key</h3>
            <p>
              Saved in the OS credential store for the selected provider. Environment
              variables still act as fallback.
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
