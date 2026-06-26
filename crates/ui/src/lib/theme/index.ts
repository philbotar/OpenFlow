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

export type TerminalThemeColors = {
  background: string;
  foreground: string;
  cursor: string;
  selectionBackground: string;
};

export function readTerminalThemeColors(
  root: HTMLElement = document.documentElement,
): TerminalThemeColors {
  const styles = getComputedStyle(root);
  const read = (name: string, fallback: string) => styles.getPropertyValue(name).trim() || fallback;
  return {
    background: read("--terminal-bg", "rgba(255, 255, 255, 0.66)"),
    foreground: read("--terminal-fg", "#18181b"),
    cursor: read("--terminal-cursor", "#18181b"),
    selectionBackground: read("--terminal-selection-bg", "rgba(111, 124, 247, 0.18)"),
  };
}
