import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const fullCssCascade = [
  "tokens.css",
  "index.css",
  "sidebar.css",
  "dock-chrome.css",
  "chat.css",
]
  .map((file) => readFileSync(`src/styles/${file}`, "utf8"))
  .join("\n");

describe("chat styles", () => {
  it("gives assistant messages a wider reading lane than user messages", () => {
    expect(fullCssCascade).toMatch(
      /\.chat-message-bubble--assistant\s*\{[^}]*max-width:\s*min\(110ch,\s*100%\);/,
    );
    expect(fullCssCascade).toMatch(
      /\.chat-message-bubble--user\s*\{[^}]*max-width:\s*min\(56ch,\s*72%\);/,
    );
  });

  it("keeps Thinking and token usage on one status row", () => {
    expect(fullCssCascade).toMatch(
      /\.direct-chat-status-row\s*\{[^}]*display:\s*flex;[^}]*align-items:\s*center;[^}]*justify-content:\s*space-between;/,
    );
    expect(fullCssCascade).toMatch(
      /\.direct-chat-token-usage\s*\{[^}]*margin-left:\s*auto;/,
    );
  });

  it("styles project and approval controls as runtime pills", () => {
    expect(fullCssCascade).toMatch(
      /\.chat-runtime-menu-trigger,\s*\.composer-runtime-controls \.composer-runtime-select \.text-select-trigger\s*\{[^}]*min-height:\s*28px;[^}]*border:\s*1px solid transparent;[^}]*border-radius:\s*var\(--radius-pill\);[^}]*background-color:\s*var\(--surface-hover\);/,
    );
    expect(fullCssCascade).toMatch(
      /\.chat-runtime-menu-trigger:hover:not\(:disabled\),\s*\.chat-runtime-menu-trigger\[aria-expanded="true"\],\s*\.composer-runtime-controls \.composer-runtime-select \.text-select-trigger:hover:not\(:disabled\),\s*\.composer-runtime-controls \.composer-runtime-select \.text-select-trigger\[aria-expanded="true"\]\s*\{[^}]*border-color:\s*var\(--border-subtle\);[^}]*background-color:\s*var\(--surface-emphasis\);[^}]*color:\s*var\(--text\);/,
    );
  });

  it("uses one vertical gap between every tool and message item", () => {
    expect(fullCssCascade).toMatch(
      /\.chat-segment-body\s*\{[^}]*gap:\s*var\(--chat-item-gap\);/,
    );
    expect(fullCssCascade).not.toMatch(
      /\.chat-segment-body > \.chat-message-row,\s*\.chat-segment-body > \.node-completed-row\s*\{[^}]*margin-block:/,
    );
  });

  it("keeps the authoring inspector as a full-height right rail", () => {
    expect(fullCssCascade).toMatch(
      /\.workflow-authoring-body--with-preview:has\(> \.workflow-authoring-inspector\)\s*\{[^}]*grid-template-columns:\s*minmax\(0, 1fr\) minmax\(280px, 360px\);/,
    );
    expect(fullCssCascade).toMatch(
      /\.workflow-authoring-body--with-preview > \.workflow-authoring-inspector\s*\{[^}]*grid-column:\s*2;[^}]*grid-row:\s*1 \/ -1;[^}]*height:\s*100%;[^}]*max-height:\s*none;[^}]*margin:\s*0;[^}]*border:\s*0;[^}]*border-left:\s*1px solid var\(--border\);/,
    );
  });

  it("keeps the authoring composer and create action inline", () => {
    expect(fullCssCascade).toMatch(
      /\.workflow-authoring-composer-row\s*\{[^}]*grid-template-columns:\s*minmax\(0, 1fr\) auto;[^}]*align-items:\s*center;/,
    );
    expect(fullCssCascade).toMatch(
      /\.workflow-authoring-apply-group\s*\{[^}]*width:\s*auto;/,
    );
  });
});
