import { createSignal, onMount, Show } from "solid-js";
import * as desktop from "../api";
import { Button, SettingsSection } from "@/components";
import { useAppContext } from "../context/AppContext";

type UpdateUiState =
  | { kind: "idle" }
  | { kind: "working"; label: string }
  | { kind: "message"; label: string; tone: "info" | "error" };

export function AboutSection() {
  const ctx = useAppContext();
  const [version, setVersion] = createSignal("…");
  const [updateState, setUpdateState] = createSignal<UpdateUiState>({ kind: "idle" });

  onMount(() => {
    void desktop.getAppVersion().then(setVersion);
  });

  async function handleUpdate() {
    if (ctx.runState()?.active) {
      setUpdateState({
        kind: "message",
        label: "Stop the current run before updating.",
        tone: "error",
      });
      return;
    }

    setUpdateState({ kind: "working", label: "Downloading update…" });
    const result = await desktop.installAppUpdate();
    if (result.status === "current") {
      ctx.clearAppUpdateAvailable();
      setUpdateState({
        kind: "message",
        label: "You're on the latest version.",
        tone: "info",
      });
      return;
    }
    if (result.status === "unavailable") {
      setUpdateState({
        kind: "message",
        label: "Updates are only available in the packaged macOS app.",
        tone: "info",
      });
      return;
    }
    if (result.status === "error") {
      setUpdateState({
        kind: "message",
        label: result.message,
        tone: "error",
      });
    }
  }

  const isUpdating = () => updateState().kind === "working";
  const statusLabel = () => {
    const state = updateState();
    return state.kind === "idle" ? null : state.label;
  };
  const statusIsError = () => {
    const state = updateState();
    return state.kind === "message" && state.tone === "error";
  };

  return (
    <SettingsSection sectionClass="about-section">
      <div>
        <div class="eyebrow">About</div>
        <h3>OpenFlow</h3>
        <p class="about-version">Version {version()}</p>
        <p class="about-note">
          macOS may show a security warning on first install without an Apple Developer
          account. Use Right-click → Open, or allow the app in System Settings.
        </p>
      </div>
      <div class="about-actions">
        <Button variant="primary" disabled={isUpdating()} onClick={() => void handleUpdate()}>
          {isUpdating() ? "Updating…" : "Update now"}
        </Button>
        <Show when={statusLabel()}>
          {(label) => (
            <p
              class="about-status"
              classList={{ "about-status-error": statusIsError() }}
              role="status"
            >
              {label()}
            </p>
          )}
        </Show>
      </div>
    </SettingsSection>
  );
}
