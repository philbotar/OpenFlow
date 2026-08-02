// @vitest-environment jsdom
import { createSignal } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";
import { listMcpCapabilities, previewMcpResource } from "../api";
import type { McpPromptSelection, McpResourceSelection, McpServerConfig } from "../lib/types";
import { McpContextEditor } from "./McpContextEditor";

vi.mock("../api", () => ({
  listMcpCapabilities: vi.fn(),
  previewMcpResource: vi.fn(),
  previewMcpPrompt: vi.fn(),
}));

async function flushPromises() {
  await Promise.resolve();
  await Promise.resolve();
  await new Promise((resolve) => setTimeout(resolve, 0));
}

const server: McpServerConfig = {
  schemaVersion: 1,
  id: "docs",
  displayName: "Docs",
  source: { type: "manual" },
  install: { type: "external" },
  connection: { type: "stdio", command: "docs-mcp", args: [], environment: {} },
  trust: { approvedFingerprint: "fingerprint" },
  policy: {
    defaultToolAccess: "read",
    defaultToolConcurrency: "exclusive",
    allowRoots: false,
    allowSampling: false,
    allowElicitation: false,
  },
  enabled: true,
};

describe("McpContextEditor", () => {
  let container: HTMLDivElement;
  let dispose: (() => void) | undefined;

  beforeEach(() => {
    container = document.createElement("div");
    document.body.appendChild(container);
    vi.mocked(listMcpCapabilities).mockResolvedValue({
      serverId: "docs",
      resources: [
        {
          serverId: "docs",
          uri: "docs://guide",
          name: "guide",
          title: "Guide",
          description: "Project guide",
          mimeType: "text/plain",
          sizeBytes: 8,
          subscribable: true,
        },
      ],
      prompts: [],
    });
    vi.mocked(previewMcpResource).mockResolvedValue({
      kind: "resource",
      serverId: "docs",
      source: "docs://guide",
      content: "abcde",
      originalSizeBytes: 8,
      includedSizeBytes: 5,
      truncated: true,
    });
  });

  afterEach(() => {
    dispose?.();
    container.remove();
    vi.clearAllMocks();
  });

  test("adds only checked resources and previews bounded content with provenance", async () => {
    const [resources, setResources] = createSignal<McpResourceSelection[]>([]);
    const [prompts, setPrompts] = createSignal<McpPromptSelection[]>([]);
    dispose = render(
      () => (
        <McpContextEditor
          servers={[server]}
          resources={resources()}
          prompts={prompts()}
          onChange={(nextResources, nextPrompts) => {
            setResources(nextResources);
            setPrompts(nextPrompts);
          }}
        />
      ),
      container,
    );

    expect(container.textContent).not.toContain("docs://guide");
    const load = [...container.querySelectorAll("button")].find((button) =>
      button.textContent?.includes("Load resources"),
    ) as HTMLButtonElement;
    load.click();
    await flushPromises();

    expect(container.textContent).toContain("docs://guide");
    const resourceItem = [...container.querySelectorAll(".mcp-context-item")].find((item) =>
      item.textContent?.includes("docs://guide"),
    ) as HTMLElement;
    (resourceItem.querySelector('input[type="checkbox"]') as HTMLInputElement).click();
    expect(resources()).toEqual([
      { serverId: "docs", uri: "docs://guide", maxBytes: 65_536, subscribe: false },
    ]);

    const maxBytes = resourceItem.querySelector('input[type="number"]') as HTMLInputElement;
    maxBytes.value = "5";
    maxBytes.dispatchEvent(new InputEvent("input", { bubbles: true }));
    const preview = [...resourceItem.querySelectorAll("button")].find(
      (button) => button.textContent === "Preview",
    ) as HTMLButtonElement;
    preview.click();
    await flushPromises();

    expect(previewMcpResource).toHaveBeenCalledWith("docs", "docs://guide", 5);
    expect(container.textContent).toContain("Preview · docs · docs://guide");
    expect(container.textContent).toContain("abcde");
    expect(container.textContent).toContain("truncated");
  });
});
