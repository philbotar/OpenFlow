// @vitest-environment jsdom
import { createSignal } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";
import { AppContext, type AppContextValue } from "../context/AppContext";
import { SettingsScreen } from "../screens/SettingsScreen";
import type { SettingsSectionId } from "../settings/types";

describe("SettingsScreen", () => {
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

  function renderScreen(overrides: Partial<AppContextValue> = {}) {
    const [settingsSection, setSettingsSection] =
      createSignal<SettingsSectionId>("appearance");
    const ctx = {
      settingsSection,
      setSettingsSection,
      themePreference: () => "system" as const,
      handleSetThemePreference: vi.fn(),
      activeProfileMemo: () => ({
        display_name: "OpenAI",
        base_url: "https://api.openai.com/v1",
        transport: "responses" as const,
        responses_path: "responses",
        chat_completions_path: "chat/completions",
        known_models: ["gpt-4.1-mini"],
        default_model: "gpt-4.1-mini",
        editable: false,
      }),
      settings: () => ({
        active_provider: "openai",
        providers: {},
      }),
      providerIdsMemo: () => ["openai"],
      activeProviderKeyInput: () => "",
      newModelInputByProvider: () => ({}),
      readiness: () => ({ ready: false, provider: "OpenAI", message: "Missing key", envVar: "" }),
      handleApiKeyInput: vi.fn(),
      handleAddKnownModel: vi.fn(),
      handleRemoveKnownModel: vi.fn(),
      handleAddReasoningEffortOption: vi.fn(),
      handleRemoveReasoningEffortOption: vi.fn(),
      handleSaveSettings: vi.fn(),
      updateSettings: vi.fn(),
      setNewModelInputByProvider: vi.fn(),
      navigateToScreen: vi.fn(),
      isMaximized: () => false,
      ...overrides,
    } as AppContextValue;

    dispose = render(
      () => (
        <AppContext.Provider value={ctx}>
          <SettingsScreen />
        </AppContext.Provider>
      ),
      container,
    );
  }

  function navButtons() {
    return [...container.querySelectorAll<HTMLButtonElement>(".settings-nav-button")];
  }

  test("nav lists Appearance, Providers, Search, MCP Servers, Diagnostics, and About", () => {
    renderScreen();
    expect(navButtons().map((button) => button.textContent?.trim())).toEqual([
      "Appearance",
      "Providers",
      "Search",
      "MCP Servers",
      "Diagnostics",
      "About",
    ]);
  });

  test("defaults to Appearance section", () => {
    renderScreen();
    expect(container.querySelector(".appearance-section .segmented-control")).not.toBeNull();
    expect(container.querySelector(".providers-section")).toBeNull();
  });

  test("selecting Providers shows providers section", () => {
    renderScreen();
    navButtons()[1]?.click();
    expect(container.querySelector(".providers-section")).not.toBeNull();
    expect(container.querySelector(".appearance-section")).toBeNull();
  });

  test("selecting About shows about section", () => {
    renderScreen();
    navButtons()[5]?.click();
    expect(container.querySelector(".about-section")).not.toBeNull();
  });

  test("active nav button exposes aria-current page", () => {
    renderScreen();
    expect(navButtons()[0]?.getAttribute("aria-current")).toBe("page");
    navButtons()[1]?.click();
    expect(navButtons()[1]?.getAttribute("aria-current")).toBe("page");
    expect(navButtons()[0]?.hasAttribute("aria-current")).toBe(false);
  });
});
