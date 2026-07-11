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

  test("renders one connections surface with custom add collapsed", () => {
    renderSection();

    expect(mountPoint.textContent).toContain("MCP servers");
    expect(mountPoint.querySelectorAll(".mcp-connection-row")).toHaveLength(2);
    expect(mountPoint.textContent).toContain("GitHub");
    expect(mountPoint.textContent).toContain("linear");
    expect(mountPoint.textContent).not.toContain("Discovered servers");
    expect(mountPoint.textContent).not.toContain("Configured servers");
    expect(mountPoint.querySelector(".mcp-composer-fields")).toBeNull();

    mountPoint.querySelector<HTMLButtonElement>(".mcp-add-trigger")?.click();

    expect(mountPoint.querySelector(".mcp-composer-fields")).not.toBeNull();
  });

  test("renders discovery summary counts", () => {
    renderSection();
    expect(mountPoint.textContent).toContain("1 discovered · 1 saved in OpenFlow");
  });

  test("renders configured server row", () => {
    renderSection();
    const nameInput = mountPoint.querySelector(
      ".mcp-configured-fields input.text-input",
    ) as HTMLInputElement | null;
    expect(nameInput?.value).toBe("GitHub");
    expect(mountPoint.textContent).toContain("OpenFlow settings");
  });

  test("renders discovered server row", () => {
    renderSection();
    expect(mountPoint.textContent).toContain("linear");
    expect(mountPoint.textContent).toContain("cursor");
  });

  test("shows one empty state for connections", () => {
    renderSection({
      discoveredMcp: () => [],
      settings: () => ({
        active_provider: "openai",
        providers: {},
        mcp: { servers: [] },
      }),
    });

    const emptyStates = mountPoint.querySelectorAll(".mcp-empty-state");
    expect(emptyStates).toHaveLength(1);
    expect(mountPoint.textContent).toContain("No MCP servers yet.");
  });

  test("connection sections expose aria-labelledby groups", () => {
    renderSection();

    for (const id of ["mcp-connections-heading", "mcp-advanced-heading", "mcp-add-heading"]) {
      expect(mountPoint.querySelector(`section[aria-labelledby="${id}"]`)).not.toBeNull();
    }
  });

  test("composer card uses secondary surface treatment", () => {
    renderSection();
    expect(mountPoint.querySelector(".mcp-card--composer")).not.toBeNull();
  });
});
