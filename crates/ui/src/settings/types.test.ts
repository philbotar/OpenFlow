import { describe, expect, it } from "vitest";
import { SETTINGS_SECTIONS } from "./types";

describe("SETTINGS_SECTIONS", () => {
  it("exposes appearance, providers, search, MCP, diagnostics, and about sections", () => {
    expect(SETTINGS_SECTIONS.map((section) => section.id)).toEqual([
      "appearance",
      "providers",
      "search",
      "mcp",
      "diagnostics",
      "about",
    ]);
  });

  it("uses human-readable nav labels", () => {
    expect(SETTINGS_SECTIONS.map((section) => section.label)).toEqual([
      "Appearance",
      "Providers",
      "Search",
      "MCP Servers",
      "Diagnostics",
      "About",
    ]);
  });
});
