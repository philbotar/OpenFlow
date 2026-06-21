// @vitest-environment jsdom
import { createSignal } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";
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
  known_models: ["compatible-model"],
  default_model: "compatible-model",
  editable: true,
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
};

describe("ProvidersSection", () => {
  let container: HTMLDivElement;
  let dispose: (() => void) | undefined;

  beforeEach(() => {
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
    const handleSaveSettings = vi.fn();

    const ctx = {
      settings,
      activeProfileMemo: () => activeProfile(settings()),
      providerIdsMemo: () => ["openai", "anthropic", "custom_openai_compatible"],
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
      handleSaveSettings,
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

  test("readiness chip gets ready class when provider is ready", () => {
    renderSection();
    expect(container.querySelector(".readiness-chip.ready")).not.toBeNull();
  });

  test("hides reasoning subsection when profile has no effort options", () => {
    renderSection("openai");
    expect(subheading("providers-reasoning-heading")).toBeNull();
  });

  test("shows reasoning subsection when profile has effort options", () => {
    renderSection("anthropic");
    expect(subheading("providers-reasoning-heading")?.textContent).toBe("Reasoning defaults");
  });

  test("shows budget token input when selected effort uses budget", () => {
    renderSection("anthropic");
    const budgetInput = container.querySelector(
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
    const chip = container.querySelector(".model-chip") as HTMLButtonElement;
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
});
