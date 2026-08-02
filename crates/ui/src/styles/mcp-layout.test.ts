import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const indexCss = readFileSync("src/styles/index.css", "utf8");

describe("MCP settings layout", () => {
  it("keeps title and actions inline while full server content spans below", () => {
    expect(indexCss).toMatch(
      /\.mcp-connection-row\s*\{[^}]*display:\s*grid;[^}]*grid-template-columns:\s*minmax\(0, 1fr\) max-content;/,
    );
    expect(indexCss).toMatch(
      /> \.mcp-server-row-main\s*\{[^}]*display:\s*contents;/,
    );
    expect(indexCss).toMatch(
      /> \.mcp-server-row-main > \*\s*\{[^}]*grid-column:\s*1 \/ -1;/,
    );
    expect(indexCss).toMatch(
      /> \.mcp-server-row-main > \.mcp-server-name\s*\{[^}]*grid-row:\s*1;[^}]*grid-column:\s*1;/,
    );
    expect(indexCss).toMatch(
      /> \.mcp-server-row-actions\s*\{[^}]*grid-row:\s*1;[^}]*grid-column:\s*2;/,
    );
  });
});
