import { createMemo, createSignal, For, Show } from "solid-js";
import RefreshCw from "lucide-solid/icons/refresh-cw";
import ShieldCheck from "lucide-solid/icons/shield-check";
import { refreshBedrockModels, verifyBedrockCredentials } from "../api";
import { SidebarIcon, TextSelect } from "../components";
import { useAppContext } from "../context/AppContext";
import { ICON_STROKE_WIDTH, normalizeError } from "../lib/utils";
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
  const bedrockRegion = createMemo(
    () => ctx.activeProfileMemo().aws_region ?? ctx.activeProfileMemo().base_url,
  );
  const profileModeLabel = () => (profileEditable() ? "Custom endpoint" : "Managed provider");
  const credentialLabel = () => (isBedrock() ? "AWS credentials" : "Local API key");
  const [refreshingModels, setRefreshingModels] = createSignal(false);
  const [verifyingCredentials, setVerifyingCredentials] = createSignal(false);
  const [newEffortValue, setNewEffortValue] = createSignal("");
  const [newEffortLabel, setNewEffortLabel] = createSignal("");
  const [newEffortUsesBudget, setNewEffortUsesBudget] = createSignal(false);

  async function handleVerifyBedrockCredentials() {
    setVerifyingCredentials(true);
    try {
      const message = await verifyBedrockCredentials(ctx.settings());
      ctx.showSuccessToast(message);
    } catch (error) {
      ctx.showErrorToast(normalizeError(error), "Test AWS connection");
    } finally {
      setVerifyingCredentials(false);
    }
  }

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
    } catch (error) {
      ctx.showErrorToast(normalizeError(error), "Refresh Bedrock models");
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

      <div class="providers-summary-grid">
        <section
          class="providers-panel providers-panel--active"
          aria-labelledby="providers-active-heading"
        >
          <div class="providers-panel-header">
            <div>
              <h3 id="providers-active-heading" class="settings-subheading">
                Active provider
              </h3>
              <p class="providers-panel-copy">
                {ctx.activeProfileMemo().display_name} is used for workflow runs and agent chat.
              </p>
            </div>
            <span class="provider-mode-pill">{profileModeLabel()}</span>
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
          <div class="provider-facts" aria-label="Selected provider details">
            <span>{credentialLabel()}</span>
            <span>{ctx.activeProfileMemo().default_model ?? "No default model"}</span>
          </div>
        </section>

        <section
          class="providers-panel providers-panel--auth"
          aria-labelledby="providers-auth-heading"
        >
          <div class="providers-panel-header">
            <div>
              <h3 id="providers-auth-heading" class="settings-subheading">
                {isBedrock() ? "AWS credentials" : "API key"}
              </h3>
              <p class="providers-panel-copy">
                {isBedrock()
                  ? "Use an AWS profile, region, or exported credentials command."
                  : "Use a stored local key, with environment variables as fallback."}
              </p>
            </div>
          </div>
          <Show
            when={!isBedrock()}
            fallback={
              <div class="providers-auth-stack">
                <div class="field-grid providers-auth-fields">
                  <label>
                    <span>AWS profile</span>
                    <input
                      type="text"
                      class="text-input"
                      value={ctx.activeProfileMemo().aws_profile ?? ""}
                      placeholder="e.g. bedrock"
                      onInput={(event) =>
                        void ctx.updateSettings((draft) => {
                          activeProfile(draft).aws_profile = event.currentTarget.value;
                        })
                      }
                    />
                  </label>
                  <label>
                    <span>Credential command (optional)</span>
                    <input
                      type="text"
                      class="text-input"
                      value={ctx.activeProfileMemo().aws_credential_command ?? ""}
                      placeholder="e.g. aws configure export-credentials --profile bedrock"
                      onInput={(event) =>
                        void ctx.updateSettings((draft) => {
                          activeProfile(draft).aws_credential_command = event.currentTarget.value;
                        })
                      }
                    />
                  </label>
                </div>
                <button
                  type="button"
                  class="secondary-button providers-icon-button"
                  disabled={verifyingCredentials()}
                  onClick={() => void handleVerifyBedrockCredentials()}
                >
                  <ShieldCheck aria-hidden="true" absoluteStrokeWidth strokeWidth={ICON_STROKE_WIDTH} />
                  {verifyingCredentials() ? "Testing…" : "Test AWS connection"}
                </button>
              </div>
            }
          >
            <div class="providers-auth-stack">
              <input
                type="password"
                value={ctx.activeProviderKeyInput()}
                onInput={(event) => ctx.handleApiKeyInput(event.currentTarget.value)}
                placeholder={ctx.readiness()?.envVar || "optional local provider key"}
                class="text-input providers-secret-input"
                aria-label="Provider API key"
              />
            </div>
          </Show>
        </section>
      </div>

      <div class="providers-detail-grid">
        <section
          class="providers-panel providers-panel--connection"
          aria-labelledby="providers-connection-heading"
        >
          <div class="providers-panel-header">
            <div>
              <h3 id="providers-connection-heading" class="settings-subheading">
                Connection
              </h3>
              <p class="providers-panel-copy">
                {profileEditable()
                  ? "Endpoint settings for this provider profile."
                  : "Managed provider connection settings are fixed."}
              </p>
            </div>
          </div>
          <div class="field-grid">
            <label>
              <span>{isBedrock() ? "AWS region" : "Base URL"}</span>
              <input
                class="text-input"
                value={isBedrock() ? bedrockRegion() : ctx.activeProfileMemo().base_url}
                disabled={!profileEditable() && !isBedrock()}
                onInput={(event) =>
                  void ctx.updateSettings((draft) => {
                    const profile = activeProfile(draft);
                    if (isBedrock()) {
                      profile.aws_region = event.currentTarget.value;
                    } else {
                      profile.base_url = event.currentTarget.value;
                    }
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
              <label>
                <span>Model timeout (seconds)</span>
                <input
                  class="text-input"
                  type="number"
                  min="1"
                  max="3600"
                  value={ctx.activeProfileMemo().request_timeout_secs ?? 300}
                  onInput={(event) =>
                    void ctx.updateSettings((draft) => {
                      const parsed = Number.parseInt(event.currentTarget.value, 10);
                      activeProfile(draft).request_timeout_secs = Number.isFinite(parsed)
                        ? Math.min(3600, Math.max(1, parsed))
                        : 300;
                    })
                  }
                />
              </label>
            </Show>
          </div>
        </section>

        <section
          class="providers-panel providers-panel--reasoning"
          aria-labelledby="providers-reasoning-heading"
        >
          <div class="providers-panel-header">
            <div>
              <h3 id="providers-reasoning-heading" class="settings-subheading">
                Reasoning defaults
              </h3>
              <p class="providers-panel-copy">
                Effort options are sent as <code>reasoning_effort</code> (e.g. Fast →{" "}
                <code>none</code> for Grok). Applied to agent nodes that do not set their own
                level.
              </p>
            </div>
          </div>
          <div class="chip-list">
            <For each={effortOptions()}>
              {(option) => (
                <button
                  type="button"
                  class="model-chip"
                  data-effort-value={option.value}
                  onClick={() => ctx.handleRemoveReasoningEffortOption(option.value)}
                >
                  {option.label === option.value
                    ? option.label
                    : `${option.label} (${option.value})`}
                  <span>×</span>
                </button>
              )}
            </For>
          </div>
          <div class="inline-form providers-effort-add">
            <input
              class="text-input"
              placeholder="Value (e.g. none)"
              value={newEffortValue()}
              onInput={(event) => setNewEffortValue(event.currentTarget.value)}
            />
            <input
              class="text-input"
              placeholder="Label (optional)"
              value={newEffortLabel()}
              onInput={(event) => setNewEffortLabel(event.currentTarget.value)}
            />
            <label class="providers-effort-budget-toggle">
              <input
                type="checkbox"
                checked={newEffortUsesBudget()}
                onChange={(event) => setNewEffortUsesBudget(event.currentTarget.checked)}
              />
              <span>Budget tokens</span>
            </label>
            <button
              type="button"
              class="secondary-button"
              onClick={() => {
                const value = newEffortValue().trim();
                if (!value) return;
                ctx.handleAddReasoningEffortOption({
                  value,
                  label: newEffortLabel().trim() || value,
                  uses_budget_tokens: newEffortUsesBudget(),
                });
                setNewEffortValue("");
                setNewEffortLabel("");
                setNewEffortUsesBudget(false);
              }}
            >
              <SidebarIcon name="plus" />
              Add effort
            </button>
          </div>
          <Show when={effortOptions().length > 0}>
            <div class="field-grid providers-reasoning-fields">
              <label>
                <span>Default reasoning effort</span>
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
        </section>
      </div>

      <section
        class="providers-panel providers-panel--models"
        aria-labelledby="providers-models-heading"
      >
        <div class="providers-panel-header">
          <div>
            <h3 id="providers-models-heading" class="settings-subheading">
              Models
            </h3>
            <p class="providers-panel-copy">
              Keep the model list short and set the default used by new workflow nodes.
            </p>
          </div>
        </div>
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
            <SidebarIcon name="plus" />
            Add model
          </button>
          <Show when={isBedrock()}>
            <button
              type="button"
              class="secondary-button providers-icon-button"
              disabled={refreshingModels()}
              onClick={() => void handleRefreshBedrockModels()}
            >
              <RefreshCw aria-hidden="true" absoluteStrokeWidth strokeWidth={ICON_STROKE_WIDTH} />
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
        <p class="settings-save-hint">
          {isBedrock()
            ? "Saves AWS profile, region, and provider profile to local settings."
            : "Saves API key and provider profile to local settings."}
        </p>
        <button type="button" class="primary-button" onClick={() => void ctx.handleSaveSettings()}>
          <SidebarIcon name="save" />
          Save settings
        </button>
      </footer>
    </div>
  );
}
