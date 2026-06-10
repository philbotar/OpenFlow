import { Show } from "solid-js";
import { useAppContext } from "../context/AppContext";
import { SidebarIcon } from "./SidebarIcon";
import { Spinner } from "./Spinner";
import { isMacOS } from "../lib/utils";

export function AppHeader() {
  const ctx = useAppContext();
  const mod = () => (isMacOS() ? "⌘" : "Ctrl");

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
          <Show
            when={ctx.appReady()}
            fallback={<span class="skeleton-line skeleton-line--title" aria-hidden="true" />}
          >
            <h2>
              {ctx.screen() === "agents"
                ? "Agents"
                : ctx.activeWorkflow()?.name ?? "Workflow"}
            </h2>
          </Show>
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
              classList={{ "topbar-icon-button-active": ctx.workflowSettingsOpen() }}
              onClick={() => ctx.handleToggleWorkflowSettings()}
              title={`Workflow settings (${mod()}+S to save)`}
              aria-label="Workflow settings"
              aria-pressed={ctx.workflowSettingsOpen()}
              data-tauri-drag-region="false"
            >
              <SidebarIcon name="settings" />
            </button>
            <button
              class="topbar-icon-button"
              onClick={() => void ctx.persistAll()}
              title={`Save (${mod()}+S)`}
              aria-label="Save workflow"
              data-tauri-drag-region="false"
            >
              <SidebarIcon name="save" />
            </button>
            <button
              class="topbar-icon-button"
              onClick={() => void ctx.handleValidate()}
              title="Validate workflow"
              aria-label="Validate workflow"
              data-tauri-drag-region="false"
            >
              <SidebarIcon name="validate" />
            </button>
            <Show
              when={ctx.runState()?.active}
              fallback={
                <button
                  class="topbar-icon-button topbar-icon-button-primary"
                  classList={{ "topbar-icon-button--loading": ctx.startingRun() }}
                  onClick={() => void ctx.handleRun()}
                  disabled={ctx.startingRun()}
                  title={`Run (${mod()}+Enter)`}
                  aria-label="Run workflow"
                  data-tauri-drag-region="false"
                >
                  <Show when={ctx.startingRun()} fallback={<SidebarIcon name="run" />}>
                    <Spinner size="sm" />
                  </Show>
                </button>
              }
            >
              <button
                class="topbar-icon-button topbar-icon-button-danger"
                classList={{ "topbar-icon-button--loading": ctx.stoppingRun() }}
                onClick={() => void ctx.handleStopRun()}
                disabled={ctx.stoppingRun()}
                title={`Stop (${mod()}+.)`}
                aria-label="Stop workflow"
                data-tauri-drag-region="false"
              >
                <Show when={ctx.stoppingRun()} fallback={<SidebarIcon name="stop" />}>
                  <Spinner size="sm" />
                </Show>
              </button>
            </Show>
          </div>
        </Show>
      </div>
    </header>
  );
}
