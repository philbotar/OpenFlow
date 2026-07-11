import { createSignal, For, onMount, Show } from "solid-js";
import { SidebarIcon } from "../components";
import { deleteSearchApiKey, loadSearchApiKey, saveSearchApiKey } from "../api";
import { useAppContext } from "../context/AppContext";
import { normalizeError } from "../lib/utils";

export const SEARCH_KEY_PROVIDERS: ReadonlyArray<{ id: string; label: string }> = [
  { id: "brave", label: "Brave" },
  { id: "serper", label: "Serper" },
  { id: "exa", label: "Exa" },
  { id: "jina", label: "Jina" },
  { id: "linkup", label: "Linkup" },
  { id: "firecrawl", label: "Firecrawl" },
  { id: "tavily", label: "Tavily" },
  { id: "perplexity", label: "Perplexity" },
  { id: "serpapi", label: "SerpApi" },
  { id: "browserless", label: "Browserless" },
  { id: "xai", label: "xAI" },
  { id: "parallel", label: "Parallel" },
];

export function SearchSection() {
  const ctx = useAppContext();
  const [drafts, setDrafts] = createSignal<Record<string, string>>({});
  const [saved, setSaved] = createSignal<Record<string, boolean>>({});

  onMount(() => {
    void Promise.all(
      SEARCH_KEY_PROVIDERS.map(async (provider) => {
        try {
          const key = await loadSearchApiKey(provider.id);
          if (key) {
            setSaved((prev) => ({ ...prev, [provider.id]: true }));
          }
        } catch {
          // Missing key state is non-fatal; row just shows as unset.
        }
      }),
    );
  });

  async function handleSave(providerId: string) {
    const value = (drafts()[providerId] ?? "").trim();
    if (!value) {
      return;
    }
    try {
      await saveSearchApiKey(providerId, value);
      setSaved((prev) => ({ ...prev, [providerId]: true }));
      setDrafts((prev) => ({ ...prev, [providerId]: "" }));
      ctx.showSuccessToast(`Saved ${providerId} search key`);
    } catch (error) {
      ctx.showErrorToast(normalizeError(error), "Save search key");
    }
  }

  async function handleDelete(providerId: string) {
    try {
      await deleteSearchApiKey(providerId);
      setSaved((prev) => ({ ...prev, [providerId]: false }));
      ctx.showSuccessToast(`Removed ${providerId} search key`);
    } catch (error) {
      ctx.showErrorToast(normalizeError(error), "Remove search key");
    }
  }

  return (
    <div class="settings-section search-section">
      <header class="providers-section-header">
        <div class="providers-section-intro">
          <div class="eyebrow">Search</div>
          <h3>Web search configuration</h3>
          <p>
            Web search uses search-cli, bundled with OpenFlow. Keys are stored
            locally and passed to the binary as environment variables when a
            workflow runs; keys already exported in your shell environment also
            work.
          </p>
        </div>
      </header>

      <section
        class="providers-panel providers-panel--connection"
        aria-labelledby="search-setup-heading"
      >
        <div class="providers-panel-header">
          <div>
            <h3 id="search-setup-heading" class="settings-subheading">
              Setup
            </h3>
            <p class="providers-panel-copy">
              Enable the web_search tool. Override the binary path only if needed.
            </p>
          </div>
        </div>
        <label class="checkbox-row">
          <input
            type="checkbox"
            checked={ctx.settings().search?.enabled ?? true}
            onChange={(event) =>
              void ctx.updateSettings((draft) => {
                draft.search = {
                  ...(draft.search ?? {}),
                  enabled: event.currentTarget.checked,
                };
              })
            }
          />
          <span>Enable the web_search tool in workflow runs</span>
        </label>
        <label>
          <span>Binary path (optional)</span>
          <input
            type="text"
            class="text-input"
            placeholder="Bundled with OpenFlow; override only if needed"
            value={ctx.settings().search?.binaryPath ?? ""}
            onInput={(event) =>
              void ctx.updateSettings((draft) => {
                draft.search = {
                  ...(draft.search ?? {}),
                  binaryPath: event.currentTarget.value.trim(),
                };
              })
            }
          />
        </label>
      </section>

      <section
        class="providers-panel providers-panel--auth"
        aria-labelledby="search-keys-heading"
      >
        <div class="providers-panel-header">
          <div>
            <h3 id="search-keys-heading" class="settings-subheading">
              Provider API keys
            </h3>
            <p class="providers-panel-copy">
              Save at least one key to expose web_search during workflow runs.
            </p>
          </div>
        </div>
        <div class="mcp-server-list">
          <For each={SEARCH_KEY_PROVIDERS}>
            {(provider) => (
              <div class="mcp-server-row">
                <label class="mcp-server-row-main">
                  <span class="mcp-server-name">{provider.label}</span>
                  <div class="inline-form">
                    <input
                      type="password"
                      class="text-input providers-secret-input"
                      data-provider={provider.id}
                      placeholder={saved()[provider.id] ? "Key saved" : "Not set"}
                      value={drafts()[provider.id] ?? ""}
                      onInput={(event) => {
                        const value = event.currentTarget.value;
                        setDrafts((prev) => ({ ...prev, [provider.id]: value }));
                      }}
                    />
                    <button
                      type="button"
                      class="secondary-button"
                      data-save-provider={provider.id}
                      onClick={() => void handleSave(provider.id)}
                    >
                      Save
                    </button>
                    <Show when={saved()[provider.id]}>
                      <button
                        type="button"
                        class="secondary-button ghost"
                        data-remove-provider={provider.id}
                        onClick={() => void handleDelete(provider.id)}
                      >
                        Remove
                      </button>
                    </Show>
                  </div>
                </label>
              </div>
            )}
          </For>
        </div>
      </section>

      <footer class="settings-save-bar">
        <p class="settings-save-hint">
          Saves enable flag and binary path to local settings. Provider keys save per row.
        </p>
        <button type="button" class="primary-button" onClick={() => void ctx.handleSaveSettings()}>
          <SidebarIcon name="save" />
          Save settings
        </button>
      </footer>
    </div>
  );
}
