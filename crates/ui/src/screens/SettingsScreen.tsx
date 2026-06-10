import { createMemo, createSignal, Show } from "solid-js";
import { useAppContext } from "../context/AppContext";
import { reasoningEffortOptions } from "../lib/workflow";
import { AppearanceSection } from "../settings/AppearanceSection";
import { AuthSection } from "../settings/AuthSection";
import { ModelsSection } from "../settings/ModelsSection";
import { ProviderSection } from "../settings/ProviderSection";
import { ReasoningSection } from "../settings/ReasoningSection";
import { SettingsNav } from "../settings/SettingsNav";
import type { SettingsSectionId } from "../settings/types";

export function SettingsScreen() {
  const ctx = useAppContext();
  const [activeSection, setActiveSection] = createSignal<SettingsSectionId>("appearance");
  const showReasoning = createMemo(
    () => reasoningEffortOptions(ctx.activeProfileMemo()).length > 0,
  );

  return (
    <section class="settings-screen settings-shell">
      <SettingsNav activeSection={activeSection()} onSelectSection={setActiveSection} />
      <div class="settings-content">
        <Show when={activeSection() === "appearance"}>
          <AppearanceSection />
        </Show>
        <Show when={activeSection() === "authentication"}>
          <AuthSection />
        </Show>
        <Show when={activeSection() === "provider"}>
          <ProviderSection />
        </Show>
        <Show when={activeSection() === "reasoning" && showReasoning()}>
          <ReasoningSection />
        </Show>
        <Show when={activeSection() === "models"}>
          <ModelsSection />
        </Show>
      </div>
    </section>
  );
}
