import { useAppContext } from "../context/AppContext";

export function AuthSection() {
  const ctx = useAppContext();

  return (
    <div class="settings-section">
      <div>
        <div class="eyebrow">Authentication</div>
        <h3>Provider API key</h3>
        <p>
          Stored in plaintext in your local settings file for the selected provider.
          Protect this machine and settings file accordingly. Environment variables
          still act as fallback.
        </p>
      </div>
      <input
        type="password"
        value={ctx.activeProviderKeyInput()}
        onInput={(event) => ctx.handleApiKeyInput(event.currentTarget.value)}
        placeholder={ctx.readiness()?.envVar || "optional local provider key"}
        class="text-input"
      />
    </div>
  );
}
