// @vitest-environment jsdom
import { render } from "solid-js/web";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";
import { AppContext, type AppContextValue } from "../context/AppContext";
import type { AppSettings } from "../lib/types";
import { SearchSection } from "./SearchSection";

vi.mock("../api", () => ({
  loadSearchApiKey: vi.fn(async (provider: string) =>
    provider === "brave" ? "bk-123" : null,
  ),
  saveSearchApiKey: vi.fn(async () => {}),
  deleteSearchApiKey: vi.fn(async () => {}),
}));

import { deleteSearchApiKey, saveSearchApiKey } from "../api";

const settings: AppSettings = {
  active_provider: "openai",
  providers: {},
  search: { enabled: true, keys: {} },
};

function stubContext(overrides: Partial<AppContextValue> = {}): AppContextValue {
  return {
    settings: () => settings,
    updateSettings: async () => {},
    showSuccessToast: () => {},
    showErrorToast: () => {},
    ...overrides,
  } as unknown as AppContextValue;
}

describe("SearchSection", () => {
  let mountPoint: HTMLDivElement;
  let dispose: (() => void) | undefined;

  beforeEach(() => {
    mountPoint = document.createElement("div");
    document.body.appendChild(mountPoint);
  });

  afterEach(() => {
    dispose?.();
    mountPoint.remove();
    vi.clearAllMocks();
  });

  test("renders a key row per search provider", () => {
    dispose = render(
      () => (
        <AppContext.Provider value={stubContext()}>
          <SearchSection />
        </AppContext.Provider>
      ),
      mountPoint,
    );
    const inputs = mountPoint.querySelectorAll("input[type='password']");
    expect(inputs.length).toBe(12);
    expect(mountPoint.textContent).toContain("Brave");
    expect(mountPoint.textContent).toContain("Tavily");
  });

  test("saving a key calls the API with provider id and value", async () => {
    dispose = render(
      () => (
        <AppContext.Provider value={stubContext()}>
          <SearchSection />
        </AppContext.Provider>
      ),
      mountPoint,
    );
    const input = mountPoint.querySelector<HTMLInputElement>(
      "input[data-provider='tavily']",
    );
    expect(input).not.toBeNull();
    input!.value = "tv-42";
    input!.dispatchEvent(new Event("input", { bubbles: true }));
    const saveButton = mountPoint.querySelector<HTMLButtonElement>(
      "button[data-save-provider='tavily']",
    );
    saveButton!.click();
    await Promise.resolve();
    expect(saveSearchApiKey).toHaveBeenCalledWith("tavily", "tv-42");
  });

  test("removing a key calls delete", async () => {
    dispose = render(
      () => (
        <AppContext.Provider value={stubContext()}>
          <SearchSection />
        </AppContext.Provider>
      ),
      mountPoint,
    );
    await Promise.resolve();
    await Promise.resolve();
    const removeButton = mountPoint.querySelector<HTMLButtonElement>(
      "button[data-remove-provider='brave']",
    );
    expect(removeButton).not.toBeNull();
    removeButton!.click();
    await Promise.resolve();
    expect(deleteSearchApiKey).toHaveBeenCalledWith("brave");
  });
});
