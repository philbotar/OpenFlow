import { createEffect, createMemo, createSignal, For, onCleanup, Show } from "solid-js";
import RefreshCw from "lucide-solid/icons/refresh-cw";
import ShieldCheck from "lucide-solid/icons/shield-check";
import {
  cancelCodexLogin,
  codexLoginStatus,
  disconnectCodex,
  refreshBedrockModels,
  startCodexLogin,
  verifyBedrockCredentials,
} from "../api";
import { Button, SectionHeader, SettingsSection, SidebarIcon, TextSelect } from "../components";
import { useAppContext } from "../context/AppContext";
import { ICON_STROKE_WIDTH, normalizeError } from "../lib/utils";
import type { CodexLoginStatus, ModelTransport } from "../lib/types";
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
  const modelTransportOptions = [
    { value: "", label: "Provider default" },
    ...transportOptions,
    { value: "anthropic_messages", label: "Anthropic Messages API" },
  ] as const;
  const profileEditable = () => ctx.activeProfileMemo().editable;
  const isBedrock = () => ctx.settings().active_provider === "bedrock";
  const isCodex = () => ctx.settings().active_provider === "openai-codex";
  const bedrockRegion = createMemo(
    () => ctx.activeProfileMemo().aws_region ?? ctx.activeProfileMemo().base_url,
  );
  const profileModeLabel = () => (profileEditable() ? "Custom endpoint" : "Managed provider");
  const credentialLabel = () =>
    isBedrock() ? "AWS credentials" : isCodex() ? "ChatGPT account" : "Local API key";
  const [refreshingModels, setRefreshingModels] = createSignal(false);
  const [verifyingCredentials, setVerifyingCredentials] = createSignal(false);
  const [newEffortValue, setNewEffortValue] = createSignal("");
  const [newEffortLabel, setNewEffortLabel] = createSignal("");
  const [newEffortUsesBudget, setNewEffortUsesBudget] = createSignal(false);
  const [codexStatus, setCodexStatus] = createSignal<CodexLoginStatus>({
    state: "disconnected",
  });
  const [codexActionPending, setCodexActionPending] = createSignal(false);
  const codexDeviceStatus = createMemo(() => {
    const status = codexStatus();
    return status.state === "awaitingDevice" ? status : null;
  });
  const codexConnectedStatus = createMemo(() => {
    const status = codexStatus();
    return status.state === "connected" ? status : null;
  });
  const codexFailedStatus = createMemo(() => {
    const status = codexStatus();
    return status.state === "failed" ? status : null;
  });
  const codexLoginPending = createMemo(() =>
    ["starting", "awaitingBrowser", "awaitingDevice"].includes(codexStatus().state),
  );

  async function refreshCodexStatus() {
    try {
      const nextStatus = await codexLoginStatus();
      setCodexStatus(nextStatus);
      if (nextStatus.state === "connected" || nextStatus.state === "disconnected") {
        await ctx.refreshReadiness();
      }
    } catch (error) {
      ctx.showErrorToast(normalizeError(error), "ChatGPT sign-in status");
    }
  }

  async function handleStartCodexLogin() {
    setCodexActionPending(true);
    try {
      setCodexStatus(await startCodexLogin());
    } catch (error) {
      ctx.showErrorToast(normalizeError(error), "Sign in with ChatGPT");
    } finally {
      setCodexActionPending(false);
    }
  }

  async function handleCancelCodexLogin() {
    setCodexActionPending(true);
    try {
      setCodexStatus(await cancelCodexLogin());
      await ctx.refreshReadiness();
    } catch (error) {
      ctx.showErrorToast(normalizeError(error), "Cancel ChatGPT sign-in");
    } finally {
      setCodexActionPending(false);
    }
  }

  async function handleDisconnectCodex() {
    setCodexActionPending(true);
    try {
      setCodexStatus(await disconnectCodex());
      await ctx.refreshReadiness();
      ctx.showSuccessToast("ChatGPT account disconnected.");
    } catch (error) {
      ctx.showErrorToast(normalizeError(error), "Disconnect ChatGPT");
    } finally {
      setCodexActionPending(false);
    }
  }

  createEffect(() => {
    if (!isCodex()) {
      setCodexStatus({ state: "disconnected" });
      return;
    }
    void refreshCodexStatus();
  });

  createEffect(() => {
    if (!isCodex() || !codexLoginPending()) return;
    const timer = window.setInterval(() => void refreshCodexStatus(), 1_000);
    onCleanup(() => window.clearInterval(timer));
  });

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

  function updateModelTransport(model: string, transport: string) {
    void ctx.updateSettings((draft) => {
      const profile = activeProfile(draft);
      const modelTransports = { ...(profile.model_transports ?? {}) };
      if (transport === "") {
        delete modelTransports[model];
      } else {
        modelTransports[model] = transport as ModelTransport;
      }
      profile.model_transports = modelTransports;
    });
  }

  return (
    <SettingsSection sectionClass="providers-section">
      <SectionHeader
        eyebrow="Providers"
        title="AI provider configuration"
        description="Choose a provider, authenticate, and manage models for workflow runs."
        actions={
          <div class="readiness-chip" classList={{ ready: ctx.readiness()?.ready }}>
            <span class="status-dot" aria-hidden="true" />
            <span>{ctx.readiness()?.message ?? "Checking provider"}</span>
          </div>
        }
      />

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
                {isBedrock() ? "AWS credentials" : isCodex() ? "ChatGPT account" : "API key"}
              </h3>
              <p class="providers-panel-copy">
                {isBedrock()
                  ? "Use an AWS profile, region, or exported credentials command."
                  : isCodex()
                    ? "Use your ChatGPT subscription to run supported Codex models."
                    : "Use a stored local key, with environment variables as fallback."}
              </p>
            </div>
          </div>
          <Show
            when={isCodex()}
            fallback={
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
                              activeProfile(draft).aws_credential_command =
                                event.currentTarget.value;
                            })
                          }
                        />
                      </label>
                    </div>
                    <Button
                      variant="secondary"
                      class="providers-icon-button"
                      disabled={verifyingCredentials()}
                      onClick={() => void handleVerifyBedrockCredentials()}
                    >
                      <ShieldCheck
                        aria-hidden="true"
                        absoluteStrokeWidth
                        strokeWidth={ICON_STROKE_WIDTH}
                      />
                      {verifyingCredentials() ? "Testing…" : "Test AWS connection"}
                    </Button>
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
            }
          >
            <div class="providers-auth-stack providers-codex-auth" data-state={codexStatus().state}>
              <Show when={codexConnectedStatus()}>
                {(connected) => (
                  <div class="providers-codex-status providers-codex-status--connected">
                    <span class="status-dot" aria-hidden="true" />
                    <div>
                      <strong>Connected to ChatGPT</strong>
                      <Show when={connected().email}>
                        {(email) => <span>{email()}</span>}
                      </Show>
                    </div>
                  </div>
                )}
              </Show>
              <Show when={codexStatus().state === "awaitingBrowser"}>
                <p class="providers-codex-message">
                  Finish signing in in your browser. OpenFlow is waiting for the secure callback.
                </p>
              </Show>
              <Show when={codexStatus().state === "starting"}>
                <p class="providers-codex-message">Starting secure ChatGPT sign-in…</p>
              </Show>
              <Show when={codexDeviceStatus()}>
                {(device) => (
                  <div class="providers-codex-device">
                    <p>Enter this one-time code on the ChatGPT verification page:</p>
                    <code>{device().userCode}</code>
                    <a href={device().verificationUrl} target="_blank" rel="noreferrer">
                      Open verification page
                    </a>
                  </div>
                )}
              </Show>
              <Show when={codexFailedStatus()}>
                {(failed) => <p class="providers-codex-error">{failed().message}</p>}
              </Show>
              <Show when={codexStatus().state === "cancelled"}>
                <p class="providers-codex-message">Sign-in cancelled.</p>
              </Show>
              <div class="providers-codex-actions">
                <Show
                  when={codexConnectedStatus()}
                  fallback={
                    <Show
                      when={codexLoginPending()}
                      fallback={
                        <Button
                          variant="primary"
                          disabled={codexActionPending()}
                          onClick={() => void handleStartCodexLogin()}
                        >
                          {codexStatus().state === "failed" || codexStatus().state === "cancelled"
                            ? "Retry ChatGPT sign-in"
                            : "Sign in with ChatGPT"}
                        </Button>
                      }
                    >
                      <Button
                        variant="secondary"
                        disabled={codexActionPending()}
                        onClick={() => void handleCancelCodexLogin()}
                      >
                        Cancel sign-in
                      </Button>
                    </Show>
                  }
                >
                  <Button
                    variant="secondary"
                    disabled={codexActionPending()}
                    onClick={() => void handleDisconnectCodex()}
                  >
                    Disconnect
                  </Button>
                </Show>
              </div>
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
            <Button
              variant="secondary"
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
            </Button>
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
              Set the default model. Custom endpoints can route each model through its required API.
            </p>
          </div>
        </div>
        <div class="provider-model-list">
          <For each={ctx.activeProfileMemo().known_models}>
            {(model) => (
              <div class="provider-model-row">
                <button
                  type="button"
                  class="model-chip"
                  aria-label={`Remove ${model}`}
                  onClick={() => ctx.handleRemoveKnownModel(model)}
                >
                  {model}
                  <span>×</span>
                </button>
                <Show when={profileEditable()}>
                  <TextSelect
                    class="provider-model-transport"
                    aria-label={`Transport for ${model}`}
                    value={ctx.activeProfileMemo().model_transports?.[model] ?? ""}
                    options={modelTransportOptions}
                    onChange={(event) => updateModelTransport(model, event.currentTarget.value)}
                  />
                </Show>
              </div>
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
          <Button variant="secondary" onClick={ctx.handleAddKnownModel}>
            <SidebarIcon name="plus" />
            Add model
          </Button>
          <Show when={isBedrock()}>
            <Button
              variant="secondary"
              class="providers-icon-button"
              disabled={refreshingModels()}
              onClick={() => void handleRefreshBedrockModels()}
            >
              <RefreshCw aria-hidden="true" absoluteStrokeWidth strokeWidth={ICON_STROKE_WIDTH} />
              {refreshingModels() ? "Refreshing…" : "Refresh from AWS"}
            </Button>
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
            : isCodex()
              ? "ChatGPT credentials are managed by Sign in and Disconnect; profile changes save locally."
              : "Saves API key and provider profile to local settings."}
        </p>
        <Button variant="primary" onClick={() => void ctx.handleSaveSettings()}>
          <SidebarIcon name="save" />
          Save settings
        </Button>
      </footer>
    </SettingsSection>
  );
}
