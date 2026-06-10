export type ThemePreference = "system" | "light" | "dark";
export type ResolvedTheme = "light" | "dark";

const STORAGE_KEY = "step-through-theme";

export function readStoredTheme(storage: Storage): ThemePreference {
  const raw = storage.getItem(STORAGE_KEY);
  if (raw === "light" || raw === "dark" || raw === "system") {
    return raw;
  }
  return "system";
}

export function writeStoredTheme(storage: Storage, theme: ThemePreference): void {
  storage.setItem(STORAGE_KEY, theme);
}

export function resolveTheme(preference: ThemePreference): ResolvedTheme {
  if (preference === "light" || preference === "dark") {
    return preference;
  }
  return globalThis.matchMedia?.("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

export function applyTheme(resolved: ResolvedTheme): void {
  document.documentElement.setAttribute("data-theme", resolved);
  document.documentElement.style.colorScheme = resolved;
}

export function watchSystemTheme(onChange: (resolved: ResolvedTheme) => void): () => void {
  const media = globalThis.matchMedia?.("(prefers-color-scheme: dark)");
  if (!media) {
    return () => {};
  }
  const handler = () => onChange(media.matches ? "dark" : "light");
  media.addEventListener("change", handler);
  return () => media.removeEventListener("change", handler);
}
