// @vitest-environment jsdom
import { render } from "solid-js/web";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";
import { AppContext, type AppContextValue } from "../context/AppContext";
import { AboutSection } from "./AboutSection";

const { getAppVersion, installAppUpdate } = vi.hoisted(() => ({
  getAppVersion: vi.fn(),
  installAppUpdate: vi.fn(),
}));

vi.mock("../port", () => ({
  createUiDesktopOutboundAdapter: () => ({
    getAppVersion,
    installAppUpdate,
  }),
}));

describe("AboutSection", () => {
  let container: HTMLDivElement;
  let dispose: (() => void) | undefined;

  beforeEach(() => {
    container = document.createElement("div");
    document.body.appendChild(container);
    getAppVersion.mockResolvedValue("0.1.1");
    installAppUpdate.mockResolvedValue({ status: "current" });
  });

  afterEach(() => {
    dispose?.();
    container.remove();
    vi.clearAllMocks();
  });

  function renderSection(overrides: Partial<AppContextValue> = {}) {
    const ctx = {
      runState: () => null,
      ...overrides,
    } as AppContextValue;

    dispose = render(
      () => (
        <AppContext.Provider value={ctx}>
          <AboutSection />
        </AppContext.Provider>
      ),
      container,
    );
  }

  test("shows app version from desktop port", async () => {
    renderSection();
    await vi.waitFor(() => {
      expect(container.textContent).toContain("Version 0.1.1");
    });
  });

  test("reports up to date after update check", async () => {
    renderSection();
    container.querySelector<HTMLButtonElement>(".primary-button")?.click();
    await vi.waitFor(() => {
      expect(container.textContent).toContain("You're on the latest version.");
    });
    expect(installAppUpdate).toHaveBeenCalledOnce();
  });

  test("blocks update while a run is active", async () => {
    renderSection({
      runState: () => ({ active: true }) as NonNullable<ReturnType<AppContextValue["runState"]>>,
    });
    container.querySelector<HTMLButtonElement>(".primary-button")?.click();
    await vi.waitFor(() => {
      expect(container.textContent).toContain("Stop the current run before updating.");
    });
    expect(installAppUpdate).not.toHaveBeenCalled();
  });
});
