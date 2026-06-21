import { For, Show } from "solid-js";
import ChevronLeft from "lucide-solid/icons/chevron-left";
import { useAppContext } from "../context/AppContext";
import { isMacOS, ICON_STROKE_WIDTH } from "../lib/utils";
import { SETTINGS_SECTIONS, type SettingsSectionId } from "./types";

export function SettingsNav(props: {
  activeSection: SettingsSectionId;
  onSelectSection: (section: SettingsSectionId) => void;
}) {
  const ctx = useAppContext();

  return (
    <nav
      class="settings-nav"
      classList={{
        "settings-nav-macos": isMacOS(),
        "settings-nav-maximized": ctx.isMaximized(),
      }}
      aria-label="Settings"
    >
      <Show when={isMacOS()}>
        <div
          class="settings-nav-drag-spacer"
          aria-hidden="true"
          data-tauri-drag-region
        />
      </Show>
      <button
        type="button"
        class="settings-back-button"
        onClick={() => ctx.navigateToScreen("editor", "nav-back")}
        data-tauri-drag-region="false"
      >
        <ChevronLeft
          class="settings-back-icon"
          aria-hidden="true"
          absoluteStrokeWidth
          strokeWidth={ICON_STROKE_WIDTH}
        />
        <span>Back to editor</span>
      </button>
      <div class="settings-nav-heading" data-tauri-drag-region>
        <div class="eyebrow">Settings</div>
      </div>
      <div class="settings-nav-list">
        <For each={SETTINGS_SECTIONS}>
          {(section) => (
            <button
              type="button"
              class="settings-nav-button"
              classList={{ "is-active": props.activeSection === section.id }}
              aria-current={props.activeSection === section.id ? "page" : undefined}
              onClick={() => props.onSelectSection(section.id)}
              data-tauri-drag-region="false"
            >
              {section.label}
            </button>
          )}
        </For>
      </div>
    </nav>
  );
}
