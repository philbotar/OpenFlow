import { createMemo } from "solid-js";
import { TextSelect } from "../components/TextSelect";
import { useAppContext } from "../context/AppContext";
import { activeProfile } from "../lib/workflow";

export function ProviderSection() {
  const ctx = useAppContext();
  const providerOptions = createMemo(() =>
    ctx.providerIdsMemo().map((providerId) => ({
      value: providerId,
      label: ctx.settings().providers[providerId]?.display_name ?? providerId,
    })),
  );
  const transportOptions = [
    { value: "responses", label: "Responses API" },
    { value: "chat_completions", label: "Chat Completions API" },
  ] as const;

  return (
    <div class="settings-section">
      <div>
        <div class="eyebrow">Provider</div>
        <h3>Execution transport</h3>
      </div>
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
          <TextSelect
            value={ctx.activeProfileMemo().transport}
            options={transportOptions}
            disabled={!ctx.activeProfileMemo().editable}
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
