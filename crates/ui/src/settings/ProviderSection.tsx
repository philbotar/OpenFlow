import { For } from "solid-js";
import { useAppContext } from "../context/AppContext";
import { activeProfile } from "../lib/workflow";

export function ProviderSection() {
  const ctx = useAppContext();

  return (
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
  );
}
