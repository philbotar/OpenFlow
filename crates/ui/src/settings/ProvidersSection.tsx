import { createEffect, createMemo, createSignal, For, onCleanup, Show } from "solid-js";
import RefreshCw from "lucide-solid/icons/refresh-cw";
import ShieldCheck from "lucide-solid/icons/shield-check";
import {
  cancelCodexLogin,
  codexLoginStatus,
  disconnectCodex,
  refreshBedrockModels,
  refreshProviderModels,
  startCodexLogin,
  verifyBedrockCredentials,
} from "../api";
import {
  Button,
  InspectorSection,
  PanelEmptyState,
  SectionHeader,
  SettingsSection,
  SidebarIcon,
  SidebarIconButton,
  TextSelect,
} from "../components";
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
  const isCustomOpenAi = () =>
    ctx.settings().active_provider === "custom_openai_compatible";
  const supportsRemoteModelDiscovery = () => !isBedrock() && !isCodex();
  const bedrockRegion = createMemo(
    () => ctx.activeProfileMemo().aws_region ?? ctx.activeProfileMemo().base_url,
  );
  const profileModeLabel = () => (profileEditable() ? "Custom endpoint" : "Managed provider");
  const [refreshingModels, setRefreshingModels] = createSignal(false);
  const [remoteModelsByProvider, setRemoteModelsByProvider] = createSignal<
    Record<string, string[]>
  >({});
  const remoteModels = createMemo(
    () => remoteModelsByProvider()[ctx.settings().active_provider] ?? [],
  );
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
    const providerId = ctx.settings().active_provider;
    setRefreshingModels(true);
    try {
      const models = await refreshBedrockModels(ctx.settings());
      setRemoteModelsByProvider((current) => ({
        ...current,
        [providerId]: models,
      }));
    } catch (error) {
      ctx.showErrorToast(normalizeError(error), "Refresh Bedrock models");
    } finally {
      setRefreshingModels(false);
    }
  }

  async function handleRefreshProviderModels() {
    const providerId = ctx.settings().active_provider;
    setRefreshingModels(true);
    try {
      const models = await refreshProviderModels(
        ctx.settings(),
        ctx.activeProviderKeyInput() || null,
      );
      setRemoteModelsByProvider((current) => ({
        ...current,
        [providerId]: models,
      }));
      if (models.length === 0) {
        ctx.showErrorToast(
          "The provider returned no models.",
          "Fetch provider models",
        );
      }
    } catch (error) {
      ctx.showErrorToast(normalizeError(error), "Fetch provider models");
    } finally {
      setRefreshingModels(false);
    }
  }

  function setRemoteModelSelected(model: string, selected: boolean) {
    void ctx.updateSettings((draft) => {
      const profile = activeProfile(draft);
      const models = selected
        ? [...new Set([...profile.known_models, model])]
        : profile.known_models.filter((candidate) => candidate !== model);
      profile.known_models = models;
      if (!selected && profile.default_model === model) {
        profile.default_model = models[0] ?? null;
      }
      if (!selected && profile.model_transports) {
        const transports = { ...profile.model_transports };
        delete transports[model];
        profile.model_transports = transports;
      }
    });
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
      />

      <div class="providers-scroll-region">
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
                  {ctx.activeProfileMemo().display_name} is used for workflow
                  runs and agent chat.
                </p>
              </div>
              <div class="providers-panel-actions">
                <span class="provider-mode-pill">{profileModeLabel()}</span>
                <div
                  class="readiness-chip"
                  classList={{ ready: ctx.readiness()?.ready }}
                >
                  <span class="status-dot" aria-hidden="true" />
                  <span>{ctx.readiness()?.message ?? "Checking provider"}</span>
                </div>
              </div>
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
            <Show when={profileEditable()}>
              <label>
                <span>API base URL</span>
                <input
                  class="text-input"
                  value={ctx.activeProfileMemo().base_url}
                  placeholder="https://provider.internal/v1"
                  onInput={(event) =>
                    void ctx.updateSettings((draft) => {
                      activeProfile(draft).base_url =
                        event.currentTarget.value;
                    })
                  }
                />
              </label>
            </Show>
          </section>

          <section
            class="providers-panel providers-panel--auth"
            aria-labelledby="providers-auth-heading"
          >
            <div class="providers-panel-header">
              <div>
                <h3 id="providers-auth-heading" class="settings-subheading">
                  {isBedrock()
                    ? "AWS credentials"
                    : isCodex()
                      ? "ChatGPT account"
                      : isCustomOpenAi()
                        ? "API key (optional)"
                        : "API key"}
                </h3>
                <p class="providers-panel-copy">
                  {isBedrock()
                    ? "Use an AWS profile, region, or exported credentials command."
                    : isCodex()
                      ? "Use your ChatGPT subscription to run supported Codex models."
                      : isCustomOpenAi()
                        ? "Optional for endpoints reached through localhost, a VPN or trusted network. Add one only when the endpoint requires auth."
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
                                activeProfile(draft).aws_profile =
                                  event.currentTarget.value;
                              })
                            }
                          />
                        </label>
                        <label>
                          <span>Credential command (optional)</span>
                          <input
                            type="text"
                            class="text-input"
                            value={
                              ctx.activeProfileMemo().aws_credential_command ??
                              ""
                            }
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
                        {verifyingCredentials()
                          ? "Testing…"
                          : "Test AWS connection"}
                      </Button>
                    </div>
                  }
                >
                  <div class="providers-auth-stack">
                    <input
                      type="password"
                      value={ctx.activeProviderKeyInput()}
                      onInput={(event) =>
                        ctx.handleApiKeyInput(event.currentTarget.value)
                      }
                      placeholder={
                        ctx.readiness()?.envVar || "optional local provider key"
                      }
                      class="text-input providers-secret-input"
                      aria-label="Provider API key"
                    />
                  </div>
                </Show>
              }
            >
              <div
                class="providers-auth-stack providers-codex-auth"
                data-state={codexStatus().state}
              >
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
                    Finish signing in in your browser. OpenFlow is waiting for
                    the secure callback.
                  </p>
                </Show>
                <Show when={codexStatus().state === "starting"}>
                  <p class="providers-codex-message">
                    Starting secure ChatGPT sign-in…
                  </p>
                </Show>
                <Show when={codexDeviceStatus()}>
                  {(device) => (
                    <div class="providers-codex-device">
                      <p>
                        Enter this one-time code on the ChatGPT verification
                        page:
                      </p>
                      <code>{device().userCode}</code>
                      <a
                        href={device().verificationUrl}
                        target="_blank"
                        rel="noreferrer"
                      >
                        Open verification page
                      </a>
                    </div>
                  )}
                </Show>
                <Show when={codexFailedStatus()}>
                  {(failed) => (
                    <p class="providers-codex-error">{failed().message}</p>
                  )}
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
                            {codexStatus().state === "failed" ||
                            codexStatus().state === "cancelled"
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
                Set the default model. Custom endpoints can route each model
                through its required API.
              </p>
            </div>
          </div>
          <div class="providers-models-layout">
            <div class="providers-model-controls">
              <label>
                <span>Default model</span>
                <input
                  class="text-input"
                  list="known-models-settings"
                  value={ctx.activeProfileMemo().default_model ?? ""}
                  onInput={(event) =>
                    void ctx.updateSettings((draft) => {
                      activeProfile(draft).default_model =
                        event.currentTarget.value || null;
                    })
                  }
                />
                <datalist id="known-models-settings">
                  <For each={ctx.activeProfileMemo().known_models}>
                    {(model) => <option value={model} />}
                  </For>
                </datalist>
              </label>
              <div class="inline-form">
                <input
                  class="text-input"
                  placeholder="Add model"
                  value={
                    ctx.newModelInputByProvider()[
                      ctx.settings().active_provider
                    ] ?? ""
                  }
                  onInput={(event) =>
                    ctx.setNewModelInputByProvider((current) => ({
                      ...current,
                      [ctx.settings().active_provider]:
                        event.currentTarget.value,
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
                    <RefreshCw
                      aria-hidden="true"
                      absoluteStrokeWidth
                      strokeWidth={ICON_STROKE_WIDTH}
                    />
                    {refreshingModels() ? "Fetching…" : "Fetch from AWS"}
                  </Button>
                </Show>
                <Show when={supportsRemoteModelDiscovery()}>
                  <Button
                    variant="secondary"
                    class="providers-icon-button"
                    disabled={refreshingModels()}
                    onClick={() => void handleRefreshProviderModels()}
                  >
                    <RefreshCw
                      aria-hidden="true"
                      absoluteStrokeWidth
                      strokeWidth={ICON_STROKE_WIDTH}
                    />
                    {refreshingModels() ? "Fetching…" : "Fetch models"}
                  </Button>
                </Show>
              </div>
              <Show when={remoteModels().length > 0}>
                <fieldset class="provider-model-discovery">
                  <legend>Models returned by API</legend>
                  <p>Select every model you want available in OpenFlow.</p>
                  <div
                    class="provider-model-discovery-list"
                    role="group"
                    aria-label="Select provider models"
                  >
                    <For each={remoteModels()}>
                      {(model) => (
                        <label class="provider-model-discovery-option">
                          <input
                            type="checkbox"
                            aria-label={`Use ${model}`}
                            checked={ctx
                              .activeProfileMemo()
                              .known_models.includes(model)}
                            onChange={(event) =>
                              setRemoteModelSelected(
                                model,
                                event.currentTarget.checked,
                              )
                            }
                          />
                          <span>{model}</span>
                        </label>
                      )}
                    </For>
                  </div>
                </fieldset>
              </Show>
            </div>
            <div class="provider-model-catalog">
              <span class="provider-model-list-heading">Available models</span>
              <Show
                when={ctx.activeProfileMemo().known_models.length > 0}
                fallback={
                  <PanelEmptyState
                    title="No models configured"
                    description="Add a model to make it available for this provider."
                  />
                }
              >
                <div class="provider-model-list">
                  <For each={ctx.activeProfileMemo().known_models}>
                    {(model) => (
                      <div class="provider-model-row">
                        <span class="provider-model-name">{model}</span>
                        <Show when={profileEditable()}>
                          <TextSelect
                            class="provider-model-transport"
                            aria-label={`Transport for ${model}`}
                            value={
                              ctx.activeProfileMemo().model_transports?.[
                                model
                              ] ?? ""
                            }
                            options={modelTransportOptions}
                            onChange={(event) =>
                              updateModelTransport(
                                model,
                                event.currentTarget.value,
                              )
                            }
                          />
                        </Show>
                        <SidebarIconButton
                          icon="trash"
                          label={`Remove ${model}`}
                          class="provider-model-remove"
                          onClick={() => ctx.handleRemoveKnownModel(model)}
                        />
                      </div>
                    )}
                  </For>
                </div>
              </Show>
            </div>
          </div>
        </section>

        <div class="providers-advanced">
          <InspectorSection
            title="Advanced settings"
            summary="Connection and reasoning"
          >
            <div class="providers-detail-grid">
              <section
                class="providers-panel providers-panel--connection"
                aria-labelledby="providers-connection-heading"
              >
                <div class="providers-panel-header">
                  <div>
                    <h3
                      id="providers-connection-heading"
                      class="settings-subheading"
                    >
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
                  <Show when={!profileEditable()}>
                    <label>
                      <span>{isBedrock() ? "AWS region" : "Base URL"}</span>
                      <input
                        class="text-input"
                        value={
                          isBedrock()
                            ? bedrockRegion()
                            : ctx.activeProfileMemo().base_url
                        }
                        disabled={!isBedrock()}
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
                  </Show>
                  <Show when={!isBedrock()}>
                    <label>
                      <span>Transport</span>
                      <TextSelect
                        value={ctx.activeProfileMemo().transport}
                        options={transportOptions}
                        disabled={!profileEditable()}
                        onChange={(event) =>
                          void ctx.updateSettings((draft) => {
                            activeProfile(draft).transport = event.currentTarget
                              .value as "responses" | "chat_completions";
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
                            activeProfile(draft).responses_path =
                              event.currentTarget.value;
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
                            activeProfile(draft).chat_completions_path =
                              event.currentTarget.value;
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
                        value={
                          ctx.activeProfileMemo().request_timeout_secs ?? 300
                        }
                        onInput={(event) =>
                          void ctx.updateSettings((draft) => {
                            const parsed = Number.parseInt(
                              event.currentTarget.value,
                              10,
                            );
                            activeProfile(draft).request_timeout_secs =
                              Number.isFinite(parsed)
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
                    <h3
                      id="providers-reasoning-heading"
                      class="settings-subheading"
                    >
                      Reasoning defaults
                    </h3>
                    <p class="providers-panel-copy">
                      Effort options are sent as <code>reasoning_effort</code>.
                      Applied to agent nodes that do not set their own level.
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
                        onClick={() =>
                          ctx.handleRemoveReasoningEffortOption(option.value)
                        }
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
                    onInput={(event) =>
                      setNewEffortValue(event.currentTarget.value)
                    }
                  />
                  <input
                    class="text-input"
                    placeholder="Label (optional)"
                    value={newEffortLabel()}
                    onInput={(event) =>
                      setNewEffortLabel(event.currentTarget.value)
                    }
                  />
                  <label class="providers-effort-budget-toggle">
                    <input
                      type="checkbox"
                      checked={newEffortUsesBudget()}
                      onChange={(event) =>
                        setNewEffortUsesBudget(event.currentTarget.checked)
                      }
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
                            profile.default_reasoning_effort =
                              nextValue || null;
                          })
                        }
                      />
                    </label>
                    <Show when={selectedEffortOption()?.uses_budget_tokens}>
                      <label>
                        <span>
                          Budget tokens for {selectedEffortOption()?.label}
                        </span>
                        <input
                          class="text-input"
                          type="number"
                          min={1}
                          step={1}
                          value={
                            defaultReasoningBudgetTokens(
                              ctx.activeProfileMemo(),
                            )[selectedEffort()] ?? ""
                          }
                          onInput={(event) =>
                            void ctx.updateSettings((draft) => {
                              const profile = activeProfile(draft);
                              const effort = selectedEffort();
                              if (!effort) return;
                              const parsed = Number.parseInt(
                                event.currentTarget.value,
                                10,
                              );
                              if (!Number.isFinite(parsed) || parsed <= 0)
                                return;
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
          </InspectorSection>
        </div>
      </div>

      <footer class="settings-save-bar">
        <p class="settings-save-hint">
          {isBedrock()
            ? "AWS profile, region, and provider changes save automatically."
            : isCodex()
              ? "ChatGPT credentials are managed by Sign in and Disconnect; profile changes save automatically."
              : "Provider changes save automatically. API keys save only when you click Save API key."}
        </p>
        <Show when={!isBedrock() && !isCodex()}>
          <Button variant="primary" onClick={() => void ctx.handleSaveSettings()}>
            <SidebarIcon name="save" />
            Save API key
          </Button>
        </Show>
      </footer>
    </SettingsSection>
  );
}
