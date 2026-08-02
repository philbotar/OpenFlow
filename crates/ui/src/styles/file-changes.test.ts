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

describe("file changes panel styles", () => {
  it("uses theme tokens for diff actions and diff content", () => {
    expect(fullCssCascade).toMatch(
      /\.file-change-action\s*\{[^}]*background:\s*var\(--surface-hover\);[^}]*color:\s*var\(--text\);/,
    );
    expect(fullCssCascade).toMatch(
      /\.file-edit-diff\s*\{[^}]*border:\s*1px solid var\(--border-subtle\);[^}]*background:\s*color-mix\(in srgb,\s*var\(--surface-ground\)[^;]*;[^}]*color:\s*var\(--text\);/,
    );
  });

  it("centers the collapsed summary in a compact control-height row", () => {
    expect(fullCssCascade).toMatch(
      /\.file-changes-panel\.is-collapsed\s*\{[^}]*padding-block:\s*0;/,
    );
    expect(fullCssCascade).toMatch(
      /\.file-changes-panel\.is-collapsed \.file-changes-panel-header\s*\{[^}]*min-height:\s*var\(--control-size-compact\);/,
    );
    expect(fullCssCascade).toMatch(
      /\.file-changes-panel-title\s*\{[^}]*display:\s*flex;[^}]*align-items:\s*center;/,
    );
  });
});
