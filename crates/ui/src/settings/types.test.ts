import { describe, expect, it } from "vitest";
import { SETTINGS_SECTIONS } from "./types";

describe("SETTINGS_SECTIONS", () => {
  it("exposes appearance, providers, and MCP sections", () => {
    expect(SETTINGS_SECTIONS.map((section) => section.id)).toEqual([
      "appearance",
      "providers",
      "mcp",
    ]);
  });

  it("uses human-readable nav labels", () => {
    expect(SETTINGS_SECTIONS.map((section) => section.label)).toEqual([
      "Appearance",
      "Providers",
      "MCP Servers",
    ]);
  });
});
