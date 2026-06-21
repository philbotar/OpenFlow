export type SettingsSectionId = "appearance" | "providers";

export const SETTINGS_SECTIONS: ReadonlyArray<{
  id: SettingsSectionId;
  label: string;
}> = [
  { id: "appearance", label: "Appearance" },
  { id: "providers", label: "Providers" },
];
