export type SettingsSectionId = "appearance" | "providers" | "mcp" | "diagnostics" | "about";

export const SETTINGS_SECTIONS: ReadonlyArray<{
  id: SettingsSectionId;
  label: string;
}> = [
  { id: "appearance", label: "Appearance" },
  { id: "providers", label: "Providers" },
  { id: "mcp", label: "MCP Servers" },
  { id: "diagnostics", label: "Diagnostics" },
  { id: "about", label: "About" },
];
