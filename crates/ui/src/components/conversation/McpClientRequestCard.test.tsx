// @vitest-environment jsdom
import { render } from "solid-js/web";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";
import { AppContext, type AppContextValue } from "../../context/AppContext";
import type { PendingMcpClientRequest } from "../../lib/types";

const openExternalUrl = vi.hoisted(() => vi.fn(async () => {}));

vi.mock("../../api", () => ({ openExternalUrl }));

import { McpClientRequestCard } from "./McpClientRequestCard";

const baseRequest: PendingMcpClientRequest = {
  requestId: "mcp-request-1",
  serverId: "github",
  nodeId: "research",
  toolCallId: "tool-call-1",
  toolName: "mcp_6_github_search",
  kind: "sampling",
  message: "Server requests an approved model sampling call.",
  maxTokens: 64,
};

describe("McpClientRequestCard", () => {
  let container: HTMLDivElement;
  let dispose: (() => void) | undefined;
  let handleMcpClientRequest: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    container = document.createElement("div");
    document.body.appendChild(container);
    handleMcpClientRequest = vi.fn(async () => {});
    openExternalUrl.mockClear();
  });

  afterEach(() => {
    dispose?.();
    container.remove();
  });

  function renderCard(request: PendingMcpClientRequest) {
    dispose = render(
      () => (
        <AppContext.Provider
          value={{ handleMcpClientRequest } as unknown as AppContextValue}
        >
          <McpClientRequestCard request={request} />
        </AppContext.Provider>
      ),
      container,
    );
  }

  function button(name: string) {
    return Array.from(container.querySelectorAll("button")).find((candidate) =>
      candidate.textContent?.includes(name),
    );
  }

  test("sampling approval is one-shot and preserves request provenance", async () => {
    renderCard(baseRequest);

    expect(container.textContent).toContain("mcp_6_github_search");
    expect(container.textContent).toContain("max 64 requested tokens");
    button("Allow once")?.click();
    await vi.waitFor(() => {
      expect(handleMcpClientRequest).toHaveBeenCalledWith("mcp-request-1", {
        allow: true,
      });
    });
  });

  test("form elicitation requires fields then submits structured content", async () => {
    renderCard({
      ...baseRequest,
      kind: "elicitationForm",
      message: "Choose release settings",
      maxTokens: undefined,
      requestedSchema: {
        type: "object",
        properties: {
          environment: { type: "string", title: "Environment", enum: ["staging", "prod"] },
          confirm: { type: "boolean", title: "Confirm" },
        },
        required: ["environment", "confirm"],
      },
    });

    expect(button("Submit")?.disabled).toBe(true);
    const environment = container.querySelector<HTMLSelectElement>(
      'select[aria-label="Environment"]',
    );
    environment!.value = "prod";
    environment!.dispatchEvent(new Event("change", { bubbles: true }));
    container.querySelector<HTMLInputElement>('input[aria-label="Confirm"]')?.click();
    expect(button("Submit")?.disabled).toBe(false);

    button("Submit")?.click();
    await vi.waitFor(() => {
      expect(handleMcpClientRequest).toHaveBeenCalledWith("mcp-request-1", {
        allow: true,
        content: { environment: "prod", confirm: true },
      });
    });
  });

  test("URL elicitation opens the system browser before accepting", async () => {
    renderCard({
      ...baseRequest,
      kind: "elicitationUrl",
      message: "Complete server consent",
      maxTokens: undefined,
      url: "https://example.com/consent",
    });

    button("Open & continue")?.click();
    await vi.waitFor(() => {
      expect(openExternalUrl).toHaveBeenCalledWith("https://example.com/consent");
      expect(handleMcpClientRequest).toHaveBeenCalledWith("mcp-request-1", {
        allow: true,
      });
    });
    expect(openExternalUrl.mock.invocationCallOrder[0]).toBeLessThan(
      handleMcpClientRequest.mock.invocationCallOrder[0],
    );
  });
});
