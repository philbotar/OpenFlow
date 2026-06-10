export type SettingsSectionId =
  | "appearance"
  | "authentication"
  | "provider"
  | "reasoning"
  | "models";

export const SETTINGS_SECTIONS: ReadonlyArray<{
  id: SettingsSectionId;
  label: string;
}> = [
  { id: "appearance", label: "Appearance" },
  { id: "authentication", label: "Authentication" },
  { id: "provider", label: "Provider" },
  { id: "reasoning", label: "Reasoning" },
  { id: "models", label: "Models" },
];
