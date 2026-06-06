import { Show } from "solid-js";
import { useAppContext } from "../context/AppContext";
import { SidebarIcon } from "./SidebarIcon";
import { isMacOS } from "../lib/utils";

export function AppHeader() {
  const ctx = useAppContext();

  return (
    <header
      class="topbar"
      classList={{
        "topbar-macos": isMacOS(),
        "topbar-maximized": ctx.isMaximized(),
      }}
      data-tauri-drag-region
    >
      <div class="topbar-leading">
        <div class="topbar-copy" data-tauri-drag-region>
          <h2>
            {ctx.screen() === "agents"
              ? "Agents"
              : ctx.activeWorkflow()?.name ?? "Loading…"}
          </h2>
        </div>
      </div>
      <div class="topbar-actions" data-tauri-drag-region>
        <div
          class="readiness-chip"
          classList={{ ready: ctx.readiness()?.ready }}
        >
          <span class="status-dot" />
          <span>{ctx.readiness()?.message ?? "Checking provider"}</span>
        </div>
        <Show when={ctx.screen() === "editor"}>
          <div class="toolbar-group topbar-button-group">
            <button
              class="topbar-icon-button"
              onClick={() => void ctx.persistAll()}
              title="Save"
              aria-label="Save workflow"
              data-tauri-drag-region="false"
            >
              <SidebarIcon name="save" />
            </button>
            <button
              class="topbar-icon-button"
              onClick={() => void ctx.handleValidate()}
              title="Validate"
              aria-label="Validate workflow"
              data-tauri-drag-region="false"
            >
              <SidebarIcon name="validate" />
            </button>
            <button
              class="topbar-icon-button topbar-icon-button-primary"
              onClick={() => void ctx.handleRun()}
              title="Run"
              aria-label="Run workflow"
              data-tauri-drag-region="false"
            >
              <SidebarIcon name="run" />
            </button>
          </div>
        </Show>
      </div>
    </header>
  );
}
