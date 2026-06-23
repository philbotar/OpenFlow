// @vitest-environment jsdom
import { render } from "solid-js/web";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";
import { AppContext, type AppContextValue } from "../context/AppContext";
import type { AppSettings, McpDiscoveryRow } from "../lib/types";
import { McpSection } from "./McpSection";

vi.mock("../api", () => ({
  probeMcpServer: vi.fn(),
}));

const discoveredMcp: McpDiscoveryRow[] = [
  {
    id: "linear",
    displayName: "linear",
    command: "npx",
    args: ["-y", "linear-mcp"],
    enabled: true,
    source: "cursor",
    sourcePath: "/Users/me/.cursor/mcp.json",
  },
];

const settings: AppSettings = {
  active_provider: "openai",
  providers: {},
  mcp: {
    servers: [
      {
        id: "gh",
        displayName: "GitHub",
        command: "npx",
        args: ["-y", "@modelcontextprotocol/server-github"],
        env: {},
        enabled: true,
      },
    ],
  },
};

function stubContext(overrides: Partial<AppContextValue> = {}): AppContextValue {
  return {
    settings: () => settings,
    discoveredMcp: () => discoveredMcp,
    updateSettings: async () => {},
    refreshDiscoveredMcp: async () => {},
    ...overrides,
  } as unknown as AppContextValue;
}

describe("McpSection", () => {
  let mountPoint: HTMLDivElement;

  beforeEach(() => {
    mountPoint = document.createElement("div");
    document.body.appendChild(mountPoint);
  });

  afterEach(() => {
    mountPoint.remove();
  });

  function renderSection(overrides: Partial<AppContextValue> = {}) {
    render(
      () => (
        <AppContext.Provider value={stubContext(overrides)}>
          <McpSection />
        </AppContext.Provider>
      ),
      mountPoint,
    );
  }

  function cardHeading(id: string) {
    return mountPoint.querySelector(`#${id}`);
  }

  test("renders section intro and four grouped cards", () => {
    renderSection();

    expect(mountPoint.textContent).toContain("External tool servers");
    expect(mountPoint.querySelectorAll(".mcp-card")).toHaveLength(4);
    expect(cardHeading("mcp-discovery-heading")?.textContent).toBe("Discovery");
    expect(cardHeading("mcp-discovered-heading")?.textContent).toBe("Discovered servers");
    expect(cardHeading("mcp-servers-heading")?.textContent).toBe("Configured servers");
    expect(cardHeading("mcp-add-heading")?.textContent).toBe("Add custom server");
  });

  test("renders discovery summary counts", () => {
    renderSection();
    expect(mountPoint.textContent).toContain("1 discovered · 1 configured");
  });

  test("renders configured server row", () => {
    renderSection();
    const nameInput = mountPoint.querySelector(
      ".mcp-configured-fields input.text-input",
    ) as HTMLInputElement | null;
    expect(nameInput?.value).toBe("GitHub");
    expect(mountPoint.textContent).toContain("Configured servers");
  });

  test("renders discovered server row", () => {
    renderSection();
    expect(mountPoint.textContent).toContain("linear");
    expect(mountPoint.textContent).toContain("cursor");
  });

  test("shows compact empty states inside management cards", () => {
    renderSection({
      discoveredMcp: () => [],
      settings: () => ({
        active_provider: "openai",
        providers: {},
        mcp: { servers: [] },
      }),
    });

    const emptyStates = mountPoint.querySelectorAll(".mcp-empty-state");
    expect(emptyStates).toHaveLength(2);
    expect(mountPoint.textContent).toContain("No discovered MCP servers.");
    expect(mountPoint.textContent).toContain("No MCP servers configured.");
  });

  test("cards expose aria-labelledby groups", () => {
    renderSection();

    for (const id of [
      "mcp-discovery-heading",
      "mcp-discovered-heading",
      "mcp-servers-heading",
      "mcp-add-heading",
    ]) {
      expect(mountPoint.querySelector(`section[aria-labelledby="${id}"]`)).not.toBeNull();
    }
  });

  test("composer card uses secondary surface treatment", () => {
    renderSection();
    expect(mountPoint.querySelector(".mcp-card--composer")).not.toBeNull();
  });
});
