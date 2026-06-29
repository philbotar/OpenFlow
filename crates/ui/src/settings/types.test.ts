import { describe, expect, it } from "vitest";
import { SETTINGS_SECTIONS } from "./types";

describe("SETTINGS_SECTIONS", () => {
  it("exposes appearance, providers, MCP, diagnostics, and about sections", () => {
    expect(SETTINGS_SECTIONS.map((section) => section.id)).toEqual([
      "appearance",
      "providers",
      "mcp",
      "diagnostics",
      "about",
    ]);
  });

  it("uses human-readable nav labels", () => {
    expect(SETTINGS_SECTIONS.map((section) => section.label)).toEqual([
      "Appearance",
      "Providers",
      "MCP Servers",
      "Diagnostics",
      "About",
    ]);
  });
});
