import { useAppContext } from "../context/AppContext";

export function AppearanceSection() {
  const ctx = useAppContext();

  return (
    <div class="settings-section">
      <div>
        <div class="eyebrow">Appearance</div>
        <h3>Theme</h3>
        <p>Choose light, dark, or match your system setting.</p>
      </div>
      <div class="theme-segment" role="group" aria-label="Theme preference">
        <button
          type="button"
          classList={{ "is-active": ctx.themePreference() === "system" }}
          onClick={() => ctx.handleSetThemePreference("system")}
        >
          System
        </button>
        <button
          type="button"
          classList={{ "is-active": ctx.themePreference() === "light" }}
          onClick={() => ctx.handleSetThemePreference("light")}
        >
          Light
        </button>
        <button
          type="button"
          classList={{ "is-active": ctx.themePreference() === "dark" }}
          onClick={() => ctx.handleSetThemePreference("dark")}
        >
          Dark
        </button>
      </div>
    </div>
  );
}
