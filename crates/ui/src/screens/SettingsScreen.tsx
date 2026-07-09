import { Show } from "solid-js";
import { AppearanceSection } from "../settings/AppearanceSection";
import { AboutSection } from "../settings/AboutSection";
import { DiagnosticsSection } from "../settings/DiagnosticsSection";
import { McpSection } from "../settings/McpSection";
import { ProvidersSection } from "../settings/ProvidersSection";
import { SearchSection } from "../settings/SearchSection";
import { SettingsNav } from "../settings/SettingsNav";
import { useAppContext } from "../context/AppContext";

export function SettingsScreen() {
  const ctx = useAppContext();

  return (
    <section class="settings-screen settings-shell">
      <SettingsNav
        activeSection={ctx.settingsSection()}
        onSelectSection={ctx.setSettingsSection}
      />
      <div class="settings-content">
        <Show when={ctx.settingsSection() === "appearance"}>
          <AppearanceSection />
        </Show>
        <Show when={ctx.settingsSection() === "providers"}>
          <ProvidersSection />
        </Show>
        <Show when={ctx.settingsSection() === "search"}>
          <SearchSection />
        </Show>
        <Show when={ctx.settingsSection() === "mcp"}>
          <McpSection />
        </Show>
        <Show when={ctx.settingsSection() === "diagnostics"}>
          <DiagnosticsSection />
        </Show>
        <Show when={ctx.settingsSection() === "about"}>
          <AboutSection />
        </Show>
      </div>
    </section>
  );
}
