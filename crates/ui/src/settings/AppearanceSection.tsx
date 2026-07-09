import { useAppContext } from "../context/AppContext";

export function AppearanceSection() {
  const ctx = useAppContext();

  return (
    <section class="settings-section appearance-section">
      <header class="panel-header">
        <div class="panel-header-copy">
          <div class="eyebrow">Appearance</div>
          <h3>Theme</h3>
          <p class="field-help">Choose light, dark, or match your system setting.</p>
        </div>
      </header>

      <div class="segmented-control" role="group" aria-label="Theme preference">
        <button
          type="button"
          classList={{ active: ctx.themePreference() === "system" }}
          onClick={() => ctx.handleSetThemePreference("system")}
        >
          System
        </button>
        <button
          type="button"
          classList={{ active: ctx.themePreference() === "light" }}
          onClick={() => ctx.handleSetThemePreference("light")}
        >
          Light
        </button>
        <button
          type="button"
          classList={{ active: ctx.themePreference() === "dark" }}
          onClick={() => ctx.handleSetThemePreference("dark")}
        >
          Dark
        </button>
      </div>
    </section>
  );
}
