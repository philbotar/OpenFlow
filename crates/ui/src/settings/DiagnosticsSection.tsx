import { createSignal, Show } from "solid-js";
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
    <div class="settings-section diagnostics-section">
      <header class="providers-section-header">
        <div class="providers-section-intro">
          <div class="eyebrow">Diagnostics</div>
          <h3>Local debug output</h3>
          <p>Show detailed errors in toasts and append toast diagnostics to a local temp file.</p>
        </div>
      </header>

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
        Local only. OpenFlow does not upload, sync, or send this file anywhere.
      </p>

      <Show when={debugOutput()}>
        <div class="diagnostics-log-path">
          <span>Temp log</span>
          <code>{ctx.localDebugLogPath() ?? "Preparing log path…"}</code>
        </div>
      </Show>
    </div>
  );
}
