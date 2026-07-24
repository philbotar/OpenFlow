import { createSignal, Show } from "solid-js";
import { SectionHeader, SettingsSection } from "@/components";
import { useAppContext } from "../context/AppContext";

export function DiagnosticsSection() {
  const ctx = useAppContext();
  const [saving, setSaving] = createSignal(false);
  const debugOutput = () => ctx.settings().local_diagnostics?.debug_output === true;

  async function handleDebugOutputToggle(enabled: boolean) {
    setSaving(true);
    try {
      await ctx.updateSettings((settings) => {
        settings.local_diagnostics ??= { debug_output: false };
        settings.local_diagnostics.debug_output = enabled;
      });
      await ctx.handleSaveSettings();
    } finally {
      setSaving(false);
    }
  }

  return (
    <SettingsSection sectionClass="diagnostics-section">
      <SectionHeader
        eyebrow="Diagnostics"
        title="Local debug output"
        description="Show detailed errors in toasts and append toast diagnostics plus full model HTTP responses (may include reasoning, tool arguments, and file content) to a local temp file."
      />

      <label class="checkbox-row diagnostics-toggle">
        <input
          type="checkbox"
          checked={debugOutput()}
          disabled={saving()}
          onChange={(event) => void handleDebugOutputToggle(event.currentTarget.checked)}
        />
        <span>Enable debug output</span>
      </label>

      <p class="diagnostics-note">
        Local only. OpenFlow does not upload, sync, or send this file anywhere. Treat the log as
        sensitive — it can contain model reasoning and tool payloads.
      </p>

      <Show when={debugOutput()}>
        <div class="diagnostics-log-path">
          <span>Temp log</span>
          <code>{ctx.localDebugLogPath() ?? "Preparing log path…"}</code>
        </div>
      </Show>
    </SettingsSection>
  );
}
