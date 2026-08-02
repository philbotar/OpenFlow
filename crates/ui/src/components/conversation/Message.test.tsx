// @vitest-environment jsdom
import { readFileSync } from "node:fs";
import { render } from "solid-js/web";
import { describe, expect, it, vi } from "vitest";
import { applyTheme } from "../../lib/theme";
import { Message } from "./Message";

const mermaidMocks = vi.hoisted(() => ({
  initialize: vi.fn(),
  render: vi.fn().mockResolvedValue({
    svg: '<svg viewBox="0 0 160 40"><text>Request to Response</text></svg>',
  }),
}));

vi.mock("mermaid", () => ({
  default: mermaidMocks,
}));

const tokensCss = readFileSync("src/styles/tokens.css", "utf8");
const indexCss = readFileSync("src/styles/index.css", "utf8");
const sidebarCss = readFileSync("src/styles/sidebar.css", "utf8");
const dockChromeCss = readFileSync("src/styles/dock-chrome.css", "utf8");
const chatCss = readFileSync("src/styles/chat.css", "utf8");
const fullCssCascade = [
  tokensCss,
  indexCss,
  sidebarCss,
  dockChromeCss,
  chatCss,
].join("\n");

function renderMessage(props: Parameters<typeof Message>[0]) {
  const container = document.createElement("div");
  document.body.append(container);
  const dispose = render(() => <Message {...props} />, container);
  return { container, dispose };
}

describe("Message", () => {
  it("does not animate streaming assistant rows", () => {
    const { container, dispose } = renderMessage({
      from: "assistant",
      label: "Assistant",
      content: "Working through it",
      streaming: true,
    });

    const row = container.querySelector(".message-assistant");
    expect(row?.classList.contains("conversation-item-enter")).toBe(false);
    expect(container.querySelector(".message-streaming-caret")).not.toBeNull();
    dispose();
  });

  it("does not animate completed assistant rows", () => {
    const { container, dispose } = renderMessage({
      from: "assistant",
      label: "Assistant",
      content: "Done",
    });

    const row = container.querySelector(".message-assistant");
    expect(row?.classList.contains("conversation-item-enter")).toBe(false);
    dispose();
  });

  it("does not animate user rows", () => {
    const { container, dispose } = renderMessage({
      from: "user",
      label: "You",
      content: "Hello",
    });

    const row = container.querySelector(".message-user");
    expect(row?.classList.contains("conversation-item-enter")).toBe(false);
    dispose();
  });

  it("does not render a user role label", () => {
    const { container, dispose } = renderMessage({
      from: "user",
      label: "You",
      content: "Hello",
    });

    expect(container.querySelector(".chat-role")).toBeNull();
    expect(container.textContent).not.toContain("You");
    dispose();
  });

  it("shows the sent date, time, and elapsed time from the prior message", () => {
    const sentAtMs = Date.UTC(2026, 6, 30, 3, 4, 5);
    const { container, dispose } = renderMessage({
      from: "assistant",
      label: "Assistant",
      content: "Done",
      sentAtMs,
      elapsedSincePreviousMs: 95_000,
    });

    const timestamp = container.querySelector("time");
    expect(timestamp?.dateTime).toBe("2026-07-30T03:04:05.000Z");
    expect(timestamp?.textContent).toContain("2026");
    expect(container.querySelector(".message-meta")?.textContent).toContain(
      "1m 35s after previous message",
    );
    expect(container.querySelector(".message-meta")?.getAttribute("aria-label")).toContain(
      "Sent",
    );
    dispose();
  });

  it("reveals message timing on hover or focus without hiding tool timing", () => {
    expect(fullCssCascade).toMatch(
      /@media \(hover: hover\) and \(pointer: fine\) \{[\s\S]*?\.chat-message-row \.message-meta\s*\{[^}]*opacity:\s*0;[^}]*\}[\s\S]*?\.chat-message-row:hover \.message-meta,\s*\.chat-message-row:focus-within \.message-meta\s*\{[^}]*opacity:\s*1;/,
    );
    expect(fullCssCascade).not.toMatch(
      /\.tool-line-duration\s*\{[^}]*opacity:\s*0;/,
    );
  });

  it("exposes Codex-inspired transcript layout hooks by role", () => {
    const user = renderMessage({
      from: "user",
      label: "You",
      content: "Align this on the right",
    });
    const assistant = renderMessage({
      from: "assistant",
      label: "Assistant",
      content: "Keep assistant prose open",
    });

    expect(user.container.querySelector(".chat-message-row--user")).not.toBeNull();
    expect(user.container.querySelector(".chat-message-bubble--user")).not.toBeNull();
    expect(assistant.container.querySelector(".chat-message-row--assistant")).not.toBeNull();
    expect(assistant.container.querySelector(".chat-message-bubble--assistant")).not.toBeNull();

    user.dispose();
    assistant.dispose();
  });

  it("renders GitHub-Flavored Markdown tables in assistant replies", () => {
    const { container, dispose } = renderMessage({
      from: "assistant",
      label: "Assistant",
      content: [
        "| Retailer | Price |",
        "| --- | ---: |",
        "| DJI Australia | A$959 |",
        "| Amazon AU | A$958.97 |",
      ].join("\n"),
    });

    const table = container.querySelector("table");
    expect(table).not.toBeNull();
    expect(table?.querySelectorAll("th")).toHaveLength(2);
    expect(table?.querySelectorAll("tbody tr")).toHaveLength(2);
    expect(table?.textContent).toContain("Amazon AU");

    dispose();
  });

  it("renders fenced Mermaid diagrams in assistant replies", async () => {
    const { container, dispose } = renderMessage({
      from: "assistant",
      label: "Assistant",
      content: "```mermaid\nflowchart LR\n  Request --> Response\n```",
    });

    try {
      await vi.waitFor(() => {
        const diagram = container.querySelector('[role="img"][aria-label="Mermaid diagram"]');
        expect(diagram).not.toBeNull();
        expect(diagram!.querySelector("svg")).not.toBeNull();
        expect(diagram!.textContent).toContain("Request to Response");
      });
    } finally {
      dispose();
      container.remove();
    }
  });

  it("opens rendered Mermaid diagrams in a full-screen dialog", async () => {
    const { container, dispose } = renderMessage({
      from: "assistant",
      label: "Assistant",
      content: "```mermaid\nflowchart LR\n  Request --> Response\n```",
    });

    try {
      const openButton = await vi.waitFor(() => {
        const button = container.querySelector<HTMLButtonElement>(
          'button[aria-label="View Mermaid diagram full screen"]',
        );
        expect(button).not.toBeNull();
        return button!;
      });

      openButton.click();

      const dialog = await vi.waitFor(() => {
        const element = document.body.querySelector(
          '[role="dialog"][aria-label="Mermaid diagram full screen"]',
        );
        expect(element).not.toBeNull();
        return element!;
      });
      expect(
        dialog.querySelector('[role="img"][aria-label="Mermaid diagram, full screen"] svg'),
      ).not.toBeNull();

      dialog.querySelector<HTMLButtonElement>('button[aria-label="Exit full screen"]')!.click();

      await vi.waitFor(() => {
        expect(
          document.body.querySelector(
            '[role="dialog"][aria-label="Mermaid diagram full screen"]',
          ),
        ).toBeNull();
      });
    } finally {
      dispose();
      container.remove();
    }
  });

  it("renders Mermaid diagrams with the active app color scheme", async () => {
    const style = document.createElement("style");
    style.textContent = tokensCss;
    document.head.append(style);
    applyTheme("dark");
    mermaidMocks.initialize.mockClear();
    const { container, dispose } = renderMessage({
      from: "assistant",
      label: "Assistant",
      content: "```mermaid\nflowchart LR\n  Dark --> Theme\n```",
    });

    try {
      await vi.waitFor(() => {
        expect(container.querySelector('[role="img"][aria-label="Mermaid diagram"]')).not.toBeNull();
      });
      expect(mermaidMocks.initialize).toHaveBeenLastCalledWith(
        expect.objectContaining({
          theme: "base",
          themeVariables: expect.objectContaining({
            background: "#101010",
            primaryColor: "#242425",
            primaryTextColor: "#f2f2f3",
            lineColor: "#55555c",
          }),
        }),
      );
    } finally {
      dispose();
      container.remove();
      style.remove();
      document.documentElement.removeAttribute("data-theme");
      document.documentElement.style.removeProperty("color-scheme");
    }
  });

  it("keeps Mermaid labels readable on source-defined light node fills", async () => {
    applyTheme("dark");
    mermaidMocks.render.mockResolvedValueOnce({
      svg: [
        '<svg viewBox="0 0 320 160">',
        '<g class="node" data-node="light">',
        '<rect class="label-container" style="fill: rgb(232, 241, 255)"></rect>',
        '<foreignObject><div><span class="nodeLabel" style="color: rgb(242, 242, 243)">Workflow definition</span></div></foreignObject>',
        "</g>",
        '<g class="node" data-node="dark">',
        '<rect class="label-container" style="fill: rgb(36, 36, 37)"></rect>',
        '<foreignObject><div><span class="nodeLabel" style="color: rgb(242, 242, 243)">Workflow loader</span></div></foreignObject>',
        "</g>",
        "</svg>",
      ].join(""),
    });
    const { container, dispose } = renderMessage({
      from: "assistant",
      label: "Assistant",
      content: "```mermaid\nflowchart TB\n  Light --> Dark\n```",
    });

    try {
      await vi.waitFor(() => {
        const lightLabel = container.querySelector<HTMLElement>(
          '[data-node="light"] .nodeLabel',
        );
        expect(getComputedStyle(lightLabel!).color).toBe("rgb(24, 24, 27)");
      });
      const darkLabel = container.querySelector<HTMLElement>('[data-node="dark"] .nodeLabel');
      expect(getComputedStyle(darkLabel!).color).toBe("rgb(242, 242, 243)");
    } finally {
      dispose();
      container.remove();
      document.documentElement.removeAttribute("data-theme");
      document.documentElement.style.removeProperty("color-scheme");
    }
  });

  it("keeps Mermaid cluster titles readable on source-defined light fills", async () => {
    applyTheme("dark");
    mermaidMocks.render.mockResolvedValueOnce({
      svg: [
        '<svg viewBox="0 0 320 160">',
        '<g class="cluster" data-cluster="light">',
        '<rect style="fill: rgb(240, 253, 244)"></rect>',
        '<g class="cluster-label"><foreignObject><div><span class="nodeLabel" style="color: rgb(242, 242, 243)">Tokio execution runtime</span></div></foreignObject></g>',
        "</g>",
        "</svg>",
      ].join(""),
    });
    const { container, dispose } = renderMessage({
      from: "assistant",
      label: "Assistant",
      content: "```mermaid\nflowchart TB\n  subgraph Runtime\n    A --> B\n  end\n```",
    });

    try {
      await vi.waitFor(() => {
        const clusterLabel = container.querySelector<HTMLElement>(
          '[data-cluster="light"] .nodeLabel',
        );
        expect(getComputedStyle(clusterLabel!).color).toBe("rgb(24, 24, 27)");
      });
    } finally {
      dispose();
      container.remove();
      document.documentElement.removeAttribute("data-theme");
      document.documentElement.style.removeProperty("color-scheme");
    }
  });

  it("gives fenced code blocks an elevated dark-theme surface", () => {
    const style = document.createElement("style");
    style.textContent = `${tokensCss}\n${indexCss}`;
    document.head.append(style);
    applyTheme("dark");
    const { container, dispose } = renderMessage({
      from: "assistant",
      label: "Assistant",
      content: "```ts\nconst answer = 42;\n```",
    });

    try {
      const codeBlock = container.querySelector("pre");
      expect(codeBlock).not.toBeNull();
      expect(getComputedStyle(codeBlock!).background).toBe("var(--surface-emphasis)");
      expect(getComputedStyle(codeBlock!).borderTopWidth).toBe("1px");
      expect(getComputedStyle(codeBlock!).color).toBe("var(--text-primary)");
    } finally {
      dispose();
      container.remove();
      style.remove();
      document.documentElement.removeAttribute("data-theme");
      document.documentElement.style.removeProperty("color-scheme");
    }
  });
});
