import { createSignal, Show } from "solid-js";
import { AppearanceSection } from "../settings/AppearanceSection";
import { AboutSection } from "../settings/AboutSection";
import { DiagnosticsSection } from "../settings/DiagnosticsSection";
import { McpSection } from "../settings/McpSection";
import { ProvidersSection } from "../settings/ProvidersSection";
import { SettingsNav } from "../settings/SettingsNav";
import type { SettingsSectionId } from "../settings/types";

export function SettingsScreen() {
  const [activeSection, setActiveSection] = createSignal<SettingsSectionId>("appearance");

  return (
    <section class="settings-screen settings-shell">
      <SettingsNav activeSection={activeSection()} onSelectSection={setActiveSection} />
      <div class="settings-content">
        <Show when={activeSection() === "appearance"}>
          <AppearanceSection />
        </Show>
        <Show when={activeSection() === "providers"}>
          <ProvidersSection />
        </Show>
        <Show when={activeSection() === "mcp"}>
          <McpSection />
        </Show>
        <Show when={activeSection() === "diagnostics"}>
          <DiagnosticsSection />
        </Show>
        <Show when={activeSection() === "about"}>
          <AboutSection />
        </Show>
      </div>
    </section>
  );
}
