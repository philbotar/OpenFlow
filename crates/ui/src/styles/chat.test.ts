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
  it("keeps Thinking and token usage on one status row", () => {
    expect(fullCssCascade).toMatch(
      /\.direct-chat-status-row\s*\{[^}]*display:\s*flex;[^}]*align-items:\s*center;[^}]*justify-content:\s*space-between;/,
    );
    expect(fullCssCascade).toMatch(
      /\.direct-chat-token-usage\s*\{[^}]*margin-left:\s*auto;/,
    );
  });
});
