import { For } from "solid-js";
import { useAppContext } from "../context/AppContext";
import { activeProfile } from "../lib/workflow";

export function ModelsSection() {
  const ctx = useAppContext();

  return (
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
  );
}
