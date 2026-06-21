// @vitest-environment jsdom
import { render } from "solid-js/web";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";
import { AppContext, type AppContextValue } from "../context/AppContext";
import type { AppSettings } from "../lib/types";
import { McpSection } from "./McpSection";

vi.mock("../api", () => ({
  probeMcpServer: vi.fn(),
}));

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

function stubContext(): AppContextValue {
  return {
    settings: () => settings,
    updateSettings: async () => {},
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

  test("renders configured server row", () => {
    render(
      () => (
        <AppContext.Provider value={stubContext()}>
          <McpSection />
        </AppContext.Provider>
      ),
      mountPoint,
    );
    expect(mountPoint.textContent).toContain("External tool servers");
    expect(mountPoint.textContent).toContain("Configured servers");
  });
});
