import { createEffect, createMemo, createSignal } from "solid-js";
import type { Accessor, Setter } from "solid-js";
import * as desktop from "../../api";
import { EMPTY_SETTINGS } from "../../constants/providers";
import {
  activeProfile,
  cloneSettings,
  providerDisplayOrder,
} from "../../lib/workflow";
import { normalizeError } from "../../lib/utils";
import type {
  AiProviderKind,
  AppSettings,
  McpDiscoveryRow,
  ProviderReadiness,
  ReasoningEffortOption,
} from "../../lib/types";
import { defaultReasoningBudgetTokens, reasoningEffortOptions } from "../../lib/workflow";

type ToastHandler = (message: string, context?: string) => void;

interface UseSettingsParams {
  showErrorToast: ToastHandler;
  showSuccessToast: ToastHandler;
}

export function useSettings(params: UseSettingsParams) {
  const [settings, setSettings] = createSignal<AppSettings>(cloneSettings(EMPTY_SETTINGS));
  const [discoveredMcp, setDiscoveredMcp] = createSignal<McpDiscoveryRow[]>([]);
  const [readiness, setReadiness] = createSignal<ProviderReadiness | null>(null);
  const [newModelInputByProvider, setNewModelInputByProvider] = createSignal<
    Record<AiProviderKind, string>
  >({} as Record<AiProviderKind, string>);
  const [providerKeyInputByProvider, setProviderKeyInputByProvider] = createSignal<
    Record<AiProviderKind, string>
  >({} as Record<AiProviderKind, string>);

  const activeProfileMemo = createMemo(() => activeProfile(settings()));
  const providerIdsMemo = createMemo(() => providerDisplayOrder(settings()));
  const activeProviderKeyInput = createMemo(
    () => providerKeyInputByProvider()[settings().active_provider] ?? "",
  );

  const refreshReadiness = async (nextSettings = settings()) => {
    try {
      setReadiness(
        await desktop.resolveProviderReadiness(
          nextSettings,
          providerKeyInputByProvider()[nextSettings.active_provider] ?? null,
        ),
      );
    } catch (error) {
      params.showErrorToast(normalizeError(error));
    }
  };

  const refreshDiscoveredMcp = async (projectPath?: string | null) => {
    try {
      const payload = await desktop.loadSettings(projectPath ?? null);
      setDiscoveredMcp(payload.discoveredMcp);
    } catch (error) {
      params.showErrorToast(normalizeError(error));
    }
  };

  const updateSettings = async (mutator: (draft: AppSettings) => void) => {
    const next = cloneSettings(settings());
    mutator(next);
    setSettings(next);
    await refreshReadiness(next);
  };

  const handleApiKeyInput = (key: string) => {
    const providerId = settings().active_provider;
    setProviderKeyInputByProvider((current) => ({ ...current, [providerId]: key }));
    void desktop
      .resolveProviderReadiness(settings(), key || null)
      .then(setReadiness)
      .catch((error) => params.showErrorToast(normalizeError(error)));
  };

  const handleSaveSettings = async () => {
    const providerId = settings().active_provider;
    const apiKey = activeProviderKeyInput().trim();
    try {
      if (providerId !== "bedrock" && providerId !== "openai-codex") {
        if (apiKey) {
          await desktop.saveProviderApiKey(providerId, apiKey);
        } else {
          await desktop.deleteProviderApiKey(providerId);
        }
      }
      await desktop.saveSettings(settings());
      await refreshReadiness();
      params.showSuccessToast("Settings saved successfully.");
    } catch (error) {
      params.showErrorToast(normalizeError(error));
    }
  };

  const handleAddKnownModel = () => {
    const provider = settings().active_provider;
    const nextName = (newModelInputByProvider()[provider] ?? "").trim();
    if (nextName === "") return;
    void updateSettings((draft) => {
      const profile = activeProfile(draft);
      if (!profile.known_models.includes(nextName)) {
        profile.known_models = [...profile.known_models, nextName];
      }
    });
    setNewModelInputByProvider((current) => ({ ...current, [provider]: "" }));
  };

  const handleRemoveKnownModel = (model: string) => {
    void updateSettings((draft) => {
      const profile = activeProfile(draft);
      profile.known_models = profile.known_models.filter((item) => item !== model);
    });
  };

  const handleAddReasoningEffortOption = (option: ReasoningEffortOption) => {
    const value = option.value.trim();
    if (value === "") return;
    const label = option.label.trim() || value;
    void updateSettings((draft) => {
      const profile = activeProfile(draft);
      const current = reasoningEffortOptions(profile);
      if (current.some((entry) => entry.value === value)) return;
      profile.reasoning_effort_options = [
        ...current,
        {
          value,
          label,
          uses_budget_tokens: option.uses_budget_tokens,
        },
      ];
    });
  };

  const handleRemoveReasoningEffortOption = (value: string) => {
    void updateSettings((draft) => {
      const profile = activeProfile(draft);
      const current = reasoningEffortOptions(profile);
      profile.reasoning_effort_options = current.filter((entry) => entry.value !== value);
      if (
        profile.default_reasoning_effort === value ||
        profile.defaultReasoningEffort === value
      ) {
        profile.default_reasoning_effort = null;
        profile.defaultReasoningEffort = null;
      }
      const budgets = { ...defaultReasoningBudgetTokens(profile) };
      delete budgets[value];
      profile.default_reasoning_budget_tokens = budgets;
      profile.defaultReasoningBudgetTokens = budgets;
    });
  };

  createEffect(() => {
    const providerId = settings().active_provider;
    if (providerId === "openai-codex") {
      setProviderKeyInputByProvider((current) => ({ ...current, [providerId]: "" }));
      void refreshReadiness();
      return;
    }
    void desktop
      .loadProviderApiKey(providerId)
      .then((apiKey) => {
        if (settings().active_provider !== providerId) return;
        const nextKey = apiKey ?? "";
        setProviderKeyInputByProvider((current) => ({ ...current, [providerId]: nextKey }));
        return desktop.resolveProviderReadiness(settings(), nextKey || null);
      })
      .then((nextReadiness) => {
        if (nextReadiness) setReadiness(nextReadiness);
      })
      .catch((error) => params.showErrorToast(normalizeError(error)));
  });

  return {
    settings,
    setSettings,
    discoveredMcp,
    setDiscoveredMcp,
    refreshDiscoveredMcp,
    readiness,
    setReadiness,
    refreshReadiness,
    newModelInputByProvider,
    setNewModelInputByProvider,
    providerKeyInputByProvider,
    setProviderKeyInputByProvider,
    activeProfileMemo,
    providerIdsMemo,
    activeProviderKeyInput,
    updateSettings,
    handleApiKeyInput,
    handleSaveSettings,
    handleAddKnownModel,
    handleRemoveKnownModel,
    handleAddReasoningEffortOption,
    handleRemoveReasoningEffortOption,
  };
}
