// @vitest-environment jsdom
import { createSignal } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";

const apiMocks = vi.hoisted(() => ({
  cancelCodexLogin: vi.fn(),
  codexLoginStatus: vi.fn(),
  disconnectCodex: vi.fn(),
  refreshBedrockModels: vi.fn(),
  startCodexLogin: vi.fn(),
  verifyBedrockCredentials: vi.fn(),
}));

vi.mock("../api", () => apiMocks);

import { AppContext, type AppContextValue } from "../context/AppContext";
import type { AppSettings, ProviderProfile } from "../lib/types";
import { activeProfile } from "../lib/workflow";
import { ProvidersSection } from "./ProvidersSection";

const OPENAI: ProviderProfile = {
  display_name: "OpenAI",
  base_url: "https://api.openai.com/v1",
  transport: "responses",
  responses_path: "responses",
  chat_completions_path: "chat/completions",
  request_timeout_secs: 300,
  known_models: ["gpt-4.1-mini"],
  default_model: "gpt-4.1-mini",
  editable: false,
};

const ANTHROPIC: ProviderProfile = {
  display_name: "Anthropic",
  base_url: "https://api.anthropic.com",
  transport: "chat_completions",
  responses_path: "v1/responses",
  chat_completions_path: "v1/messages",
  request_timeout_secs: 300,
  known_models: ["claude-sonnet-4-20250514"],
  default_model: "claude-sonnet-4-20250514",
  editable: false,
  reasoning_effort_options: [
    { value: "low", label: "Low", uses_budget_tokens: true },
    { value: "medium", label: "Medium", uses_budget_tokens: false },
  ],
  default_reasoning_effort: "low",
  default_reasoning_budget_tokens: { low: 10_240 },
};

const CUSTOM: ProviderProfile = {
  display_name: "Compatible",
  base_url: "https://example.invalid/v1",
  transport: "chat_completions",
  responses_path: "responses",
  chat_completions_path: "chat/completions",
  request_timeout_secs: 300,
  known_models: ["compatible-model"],
  default_model: "compatible-model",
  editable: true,
};

const BEDROCK: ProviderProfile = {
  display_name: "Amazon Bedrock",
  base_url: "",
  transport: "chat_completions",
  responses_path: "v1/responses",
  chat_completions_path: "v1/chat/completions",
  request_timeout_secs: 300,
  known_models: ["anthropic.claude-sonnet-4-20250514-v1:0"],
  default_model: "anthropic.claude-sonnet-4-20250514-v1:0",
  editable: false,
  aws_profile: "bedrock",
  aws_region: "us-east-1",
};

const CODEX: ProviderProfile = {
  display_name: "OpenAI Codex",
  base_url: "https://chatgpt.com/backend-api/codex",
  transport: "responses",
  responses_path: "responses",
  chat_completions_path: "",
  request_timeout_secs: 300,
  known_models: ["gpt-5.4"],
  default_model: "gpt-5.4",
  editable: false,
};

function makeSettings(activeProvider: keyof typeof baseProviders): AppSettings {
  return {
    active_provider: activeProvider,
    providers: structuredClone(baseProviders),
  };
}

const baseProviders = {
  openai: OPENAI,
  anthropic: ANTHROPIC,
  custom_openai_compatible: CUSTOM,
  bedrock: BEDROCK,
  "openai-codex": CODEX,
};

describe("ProvidersSection", () => {
  let container: HTMLDivElement;
  let dispose: (() => void) | undefined;

  beforeEach(() => {
    apiMocks.cancelCodexLogin.mockReset().mockResolvedValue({ state: "cancelled" });
    apiMocks.codexLoginStatus.mockReset().mockResolvedValue({ state: "disconnected" });
    apiMocks.disconnectCodex.mockReset().mockResolvedValue({ state: "disconnected" });
    apiMocks.refreshBedrockModels.mockReset().mockResolvedValue([]);
    apiMocks.startCodexLogin.mockReset().mockResolvedValue({ state: "awaitingBrowser" });
    apiMocks.verifyBedrockCredentials.mockReset().mockResolvedValue("AWS credentials verified");
    container = document.createElement("div");
    document.body.appendChild(container);
  });

  afterEach(() => {
    dispose?.();
    container.remove();
  });

  function renderSection(
    initialProvider: keyof typeof baseProviders = "openai",
    overrides: Partial<AppContextValue> = {},
  ) {
    const [settings, setSettings] = createSignal(makeSettings(initialProvider));
    const handleApiKeyInput = vi.fn();
    const handleAddKnownModel = vi.fn();
    const handleRemoveKnownModel = vi.fn();
    const handleAddReasoningEffortOption = vi.fn();
    const handleRemoveReasoningEffortOption = vi.fn();
    const handleSaveSettings = vi.fn();

    const ctx = {
      settings,
      activeProfileMemo: () => activeProfile(settings()),
      providerIdsMemo: () => [
        "openai",
        "openai-codex",
        "anthropic",
        "custom_openai_compatible",
        "bedrock",
      ],
      activeProviderKeyInput: () => "stored-key",
      newModelInputByProvider: () => ({ custom_openai_compatible: "new-model" }),
      readiness: () => ({
        ready: true,
        provider: "OpenAI",
        message: "Ready via env var",
        envVar: "OPENAI_API_KEY",
      }),
      handleApiKeyInput,
      handleAddKnownModel,
      handleRemoveKnownModel,
      handleAddReasoningEffortOption,
      handleRemoveReasoningEffortOption,
      handleSaveSettings,
      refreshReadiness: vi.fn().mockResolvedValue(undefined),
      showErrorToast: vi.fn(),
      showSuccessToast: vi.fn(),
      updateSettings: async (mutator: (draft: AppSettings) => void) => {
        setSettings((current) => {
          const next = structuredClone(current);
          mutator(next);
          return next;
        });
      },
      setNewModelInputByProvider: vi.fn(),
      ...overrides,
    } as AppContextValue;

    dispose = render(
      () => (
        <AppContext.Provider value={ctx}>
          <ProvidersSection />
        </AppContext.Provider>
      ),
      container,
    );

    return {
      settings,
      handleApiKeyInput,
      handleAddKnownModel,
      handleRemoveKnownModel,
      handleAddReasoningEffortOption,
      handleRemoveReasoningEffortOption,
      handleSaveSettings,
    };
  }

  function subheading(id: string) {
    return container.querySelector(`#${id}`);
  }

  test("renders page header, readiness chip, and base subsections", () => {
    renderSection();

    expect(container.querySelector(".providers-section")).not.toBeNull();
    expect(container.querySelector(".readiness-chip")?.textContent).toContain(
      "Ready via env var",
    );
    expect(subheading("providers-active-heading")?.textContent).toBe("Active provider");
    expect(subheading("providers-auth-heading")?.textContent).toBe("API key");
    expect(subheading("providers-connection-heading")?.textContent).toBe("Connection");
    expect(subheading("providers-models-heading")?.textContent).toBe("Models");
  });

  test("updates the model request timeout independently of endpoint editing", async () => {
    const { settings } = renderSection("custom_openai_compatible");
    const label = Array.from(container.querySelectorAll("label")).find(
      (candidate) => candidate.querySelector("span")?.textContent === "Model timeout (seconds)",
    );
    const input = label?.querySelector("input");

    expect(input).not.toBeNull();
    input!.value = "180";
    input!.dispatchEvent(new InputEvent("input", { bubbles: true }));
    await Promise.resolve();

    expect(settings().providers.custom_openai_compatible.request_timeout_secs).toBe(180);
  });

  test("readiness chip gets ready class when provider is ready", () => {
    renderSection();
    expect(container.querySelector(".readiness-chip.ready")).not.toBeNull();
  });

  test("shows reasoning subsection even when profile has no effort options", () => {
    renderSection("openai");
    expect(subheading("providers-reasoning-heading")?.textContent).toBe("Reasoning defaults");
    expect(
      Array.from(container.querySelectorAll("button")).some(
        (button) => button.textContent?.trim() === "Add effort",
      ),
    ).toBe(true);
  });

  test("shows reasoning subsection when profile has effort options", () => {
    renderSection("anthropic");
    expect(subheading("providers-reasoning-heading")?.textContent).toBe("Reasoning defaults");
  });

  test("calls handleAddReasoningEffortOption when add effort is clicked", () => {
    const { handleAddReasoningEffortOption } = renderSection("openai");
    const valueInput = container.querySelector(
      'input[placeholder="Value (e.g. none)"]',
    ) as HTMLInputElement;
    const labelInput = container.querySelector(
      'input[placeholder="Label (optional)"]',
    ) as HTMLInputElement;
    valueInput.value = "none";
    valueInput.dispatchEvent(new InputEvent("input", { bubbles: true }));
    labelInput.value = "Fast";
    labelInput.dispatchEvent(new InputEvent("input", { bubbles: true }));
    const addButton = [...container.querySelectorAll("button")].find(
      (button) => button.textContent?.trim() === "Add effort",
    ) as HTMLButtonElement;
    addButton.click();
    expect(handleAddReasoningEffortOption).toHaveBeenCalledWith({
      value: "none",
      label: "Fast",
      uses_budget_tokens: false,
    });
  });

  test("removes a reasoning effort option and clears matching default", async () => {
    dispose?.();
    container.remove();
    container = document.createElement("div");
    document.body.appendChild(container);

    const [liveSettings, setLiveSettings] = createSignal(makeSettings("anthropic"));
    dispose = render(
      () => (
        <AppContext.Provider
          value={
            {
              settings: liveSettings,
              activeProfileMemo: () => activeProfile(liveSettings()),
              providerIdsMemo: () => ["openai", "anthropic", "custom_openai_compatible", "bedrock"],
              activeProviderKeyInput: () => "stored-key",
              newModelInputByProvider: () => ({}),
              readiness: () => ({
                ready: true,
                provider: "Anthropic",
                message: "Ready",
                envVar: "ANTHROPIC_API_KEY",
              }),
              handleApiKeyInput: vi.fn(),
              handleAddKnownModel: vi.fn(),
              handleRemoveKnownModel: vi.fn(),
              handleAddReasoningEffortOption: vi.fn(),
              handleRemoveReasoningEffortOption: (value: string) => {
                setLiveSettings((current) => {
                  const next = structuredClone(current);
                  const profile = activeProfile(next);
                  const options = profile.reasoning_effort_options ?? [];
                  profile.reasoning_effort_options = options.filter((entry) => entry.value !== value);
                  if (profile.default_reasoning_effort === value) {
                    profile.default_reasoning_effort = null;
                  }
                  const budgets = { ...(profile.default_reasoning_budget_tokens ?? {}) };
                  delete budgets[value];
                  profile.default_reasoning_budget_tokens = budgets;
                  return next;
                });
              },
              handleSaveSettings: vi.fn(),
              updateSettings: async (mutator: (draft: AppSettings) => void) => {
                setLiveSettings((current) => {
                  const next = structuredClone(current);
                  mutator(next);
                  return next;
                });
              },
              setNewModelInputByProvider: vi.fn(),
            } as unknown as AppContextValue
          }
        >
          <ProvidersSection />
        </AppContext.Provider>
      ),
      container,
    );

    const chip = container.querySelector(
      '.model-chip[data-effort-value="low"]',
    ) as HTMLButtonElement;
    expect(chip).not.toBeNull();
    chip.click();
    await Promise.resolve();

    expect(liveSettings().providers.anthropic.reasoning_effort_options?.map((o) => o.value)).toEqual(
      ["medium"],
    );
    expect(liveSettings().providers.anthropic.default_reasoning_effort).toBeNull();
    expect(liveSettings().providers.anthropic.default_reasoning_budget_tokens?.low).toBeUndefined();
  });

  test("shows budget token input when selected effort uses budget", () => {
    renderSection("anthropic");
    const budgetLabel = Array.from(container.querySelectorAll("label")).find((candidate) =>
      candidate.querySelector("span")?.textContent?.startsWith("Budget tokens for"),
    );
    const budgetInput = budgetLabel?.querySelector(
      'input[type="number"]',
    ) as HTMLInputElement | null;
    expect(budgetInput).not.toBeNull();
    expect(budgetInput?.value).toBe("10240");
  });

  test("disables connection inputs for non-editable provider", () => {
    renderSection("openai");
    const baseUrl = container.querySelector(
      ".field-grid input.text-input",
    ) as HTMLInputElement | null;
    expect(baseUrl?.disabled).toBe(true);
    expect(container.textContent).toContain("Managed provider");
  });

  test("enables connection inputs for editable provider", () => {
    renderSection("custom_openai_compatible");
    const inputs = [...container.querySelectorAll<HTMLInputElement>('.field-grid input.text-input')];
    expect(inputs.length).toBeGreaterThan(0);
    expect(inputs.every((input) => !input.disabled)).toBe(true);
  });

  test("binds API key input to activeProviderKeyInput", () => {
    renderSection();
    const apiKeyInput = container.querySelector('input[type="password"]') as HTMLInputElement;
    expect(apiKeyInput.value).toBe("stored-key");
  });

  test("calls handleApiKeyInput when API key changes", () => {
    const { handleApiKeyInput } = renderSection();
    const apiKeyInput = container.querySelector('input[type="password"]') as HTMLInputElement;
    apiKeyInput.value = "next-key";
    apiKeyInput.dispatchEvent(new Event("input", { bubbles: true }));
    expect(handleApiKeyInput).toHaveBeenCalledWith("next-key");
  });

  test("calls handleAddKnownModel when add model is clicked", () => {
    const { handleAddKnownModel } = renderSection("custom_openai_compatible");
    const addButton = [...container.querySelectorAll("button")].find(
      (button) => button.textContent?.trim() === "Add model",
    ) as HTMLButtonElement;
    addButton.click();
    expect(handleAddKnownModel).toHaveBeenCalledTimes(1);
  });

  test("calls handleRemoveKnownModel when model chip is clicked", () => {
    const { handleRemoveKnownModel } = renderSection("custom_openai_compatible");
    const chip = [...container.querySelectorAll(".model-chip")].find((candidate) =>
      candidate.textContent?.includes("compatible-model"),
    ) as HTMLButtonElement;
    chip.click();
    expect(handleRemoveKnownModel).toHaveBeenCalledWith("compatible-model");
  });

  test("calls handleSaveSettings when save is clicked", () => {
    const { handleSaveSettings } = renderSection();
    const saveButton = [...container.querySelectorAll("button")].find(
      (button) => button.textContent?.trim() === "Save settings",
    ) as HTMLButtonElement;
    saveButton.click();
    expect(handleSaveSettings).toHaveBeenCalledTimes(1);
  });

  test("subsections expose aria-labelledby groups", () => {
    renderSection("anthropic");
    for (const id of [
      "providers-active-heading",
      "providers-auth-heading",
      "providers-connection-heading",
      "providers-reasoning-heading",
      "providers-models-heading",
    ]) {
      const section = container.querySelector(`section[aria-labelledby="${id}"]`);
      expect(section).not.toBeNull();
    }
  });

  test("bedrock shows aws profile field instead of api key input", () => {
    renderSection("bedrock");
    expect(subheading("providers-auth-heading")?.textContent).toBe("AWS credentials");
    expect(container.querySelector('input[type="password"]')).toBeNull();
    const profileInput = container.querySelector(
      'input[placeholder="e.g. bedrock"]',
    ) as HTMLInputElement;
    expect(profileInput).not.toBeNull();
    expect(profileInput.value).toBe("bedrock");
  });

  test("bedrock aws profile input updates settings", async () => {
    const { settings } = renderSection("bedrock");
    const profileInput = container.querySelector(
      'input[placeholder="e.g. bedrock"]',
    ) as HTMLInputElement;
    profileInput.value = "work-profile";
    profileInput.dispatchEvent(new Event("input", { bubbles: true }));
    expect(settings().providers.bedrock?.aws_profile).toBe("work-profile");
  });

  test("bedrock credential command input updates settings", async () => {
    const { settings } = renderSection("bedrock");
    const commandInput = Array.from(container.querySelectorAll("label")).find((label) =>
      label.textContent?.includes("Credential command"),
    )?.querySelector("input") as HTMLInputElement;
    commandInput.value = "aws configure export-credentials --profile bedrock";
    commandInput.dispatchEvent(new Event("input", { bubbles: true }));
    expect(settings().providers.bedrock?.aws_credential_command).toBe(
      "aws configure export-credentials --profile bedrock",
    );
  });

  test("bedrock shows test aws connection button", () => {
    renderSection("bedrock");
    expect(
      Array.from(container.querySelectorAll("button")).some((button) =>
        button.textContent?.includes("Test AWS connection"),
      ),
    ).toBe(true);
  });

  test("bedrock aws region input updates aws_region instead of base_url", async () => {
    const { settings } = renderSection("bedrock");
    const regionInput = Array.from(container.querySelectorAll("label")).find((label) =>
      label.textContent?.includes("AWS region"),
    )?.querySelector("input") as HTMLInputElement;

    expect(regionInput.value).toBe("us-east-1");
    regionInput.value = "ap-southeast-2";
    regionInput.dispatchEvent(new Event("input", { bubbles: true }));

    expect(settings().providers.bedrock?.aws_region).toBe("ap-southeast-2");
    expect(settings().providers.bedrock?.base_url).toBe("");
  });

  test("Codex shows ChatGPT sign-in instead of an API key", async () => {
    renderSection("openai-codex");

    await vi.waitFor(() => {
      expect(apiMocks.codexLoginStatus).toHaveBeenCalledTimes(1);
    });
    expect(subheading("providers-auth-heading")?.textContent).toBe("ChatGPT account");
    expect(container.querySelector('input[type="password"]')).toBeNull();
    expect(container.textContent).toContain("Sign in with ChatGPT");
  });

  test("Codex device fallback shows the one-time code and supports cancellation", async () => {
    apiMocks.startCodexLogin.mockResolvedValueOnce({
      state: "awaitingDevice",
      verificationUrl: "https://auth.example/device",
      userCode: "ABCD-EFGH",
      expiresAt: 1_900_000_000,
    });
    renderSection("openai-codex");
    await vi.waitFor(() => expect(apiMocks.codexLoginStatus).toHaveBeenCalled());

    const signIn = [...container.querySelectorAll("button")].find((button) =>
      button.textContent?.includes("Sign in with ChatGPT"),
    ) as HTMLButtonElement;
    signIn.click();

    await vi.waitFor(() => expect(container.textContent).toContain("ABCD-EFGH"));
    expect(container.querySelector('a[href="https://auth.example/device"]')).not.toBeNull();
    const cancel = [...container.querySelectorAll("button")].find((button) =>
      button.textContent?.includes("Cancel sign-in"),
    ) as HTMLButtonElement;
    cancel.click();
    await vi.waitFor(() => expect(apiMocks.cancelCodexLogin).toHaveBeenCalledTimes(1));
  });

  test("Codex connected state exposes only email and disconnect", async () => {
    apiMocks.codexLoginStatus.mockResolvedValueOnce({
      state: "connected",
      email: "person@example.com",
    });
    renderSection("openai-codex");

    await vi.waitFor(() => expect(container.textContent).toContain("Connected to ChatGPT"));
    expect(container.textContent).toContain("person@example.com");
    expect(container.textContent).not.toContain("access_token");
    expect(container.textContent).not.toContain("refresh_token");
    const disconnect = [...container.querySelectorAll("button")].find(
      (button) => button.textContent?.trim() === "Disconnect",
    ) as HTMLButtonElement;
    disconnect.click();
    await vi.waitFor(() => expect(apiMocks.disconnectCodex).toHaveBeenCalledTimes(1));
  });
});
