import { Show } from "solid-js";
import { useAppContext } from "../context/AppContext";
import { SidebarIcon } from "./SidebarIcon";
import { Spinner } from "./Spinner";
import { isMacOS } from "../lib/utils";

export function AppHeader() {
  const ctx = useAppContext();
  const mod = () => (isMacOS() ? "⌘" : "Ctrl");

  const title = () => {
    switch (ctx.screen()) {
      case "agents":
        return "Agents";
      case "schedule":
        return "Schedule";
      case "settings":
        return "Settings";
      case "workflow-authoring":
        return "Build workflow with AI";
      default:
        return ctx.activeWorkflow()?.name ?? "Workflow";
    }
  };

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
        <Show when={isMacOS() && !ctx.isMaximized()}>
          <div
            class="topbar-window-controls-spacer"
            aria-hidden="true"
            data-tauri-drag-region
          />
        </Show>
        <Show when={ctx.isCompactViewport()}>
          <button
            type="button"
            class="topbar-nav-button topbar-sidebar-toggle"
            onClick={() => ctx.toggleSidebarDrawer()}
            title="Open navigation"
            aria-label="Open navigation"
            aria-expanded={ctx.sidebarDrawerOpen()}
            data-tauri-drag-region="false"
          >
            <SidebarIcon name="panel-left-open" />
          </button>
        </Show>
        <Show when={!ctx.isCompactViewport()}>
          <button
            type="button"
            class="topbar-icon-button topbar-sidebar-toggle"
            onClick={() => ctx.handleToggleLeftPanel()}
            title={ctx.leftPanelHidden() ? `Show sidebar (${mod()}+B)` : `Hide sidebar (${mod()}+B)`}
            aria-label={ctx.leftPanelHidden() ? "Show left sidebar" : "Hide left sidebar"}
            data-tauri-drag-region="false"
          >
            <SidebarIcon
              name={ctx.leftPanelHidden() ? "panel-left-open" : "panel-left-close"}
            />
          </button>
        </Show>
      </div>
      <div class="topbar-title" data-tauri-drag-region>
        <Show
          when={ctx.appReady()}
          fallback={<span class="skeleton-line skeleton-line--title" aria-hidden="true" />}
        >
          <span>{title()}</span>
        </Show>
      </div>
      <div class="topbar-actions" data-tauri-drag-region>
        <Show when={ctx.screen() === "editor"}>
          <div class="toolbar-group topbar-button-group ">
            <Show when={ctx.runState()?.active}>
              <button
                type="button"
                class="topbar-danger-button"
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
                <span>{ctx.stoppingRun() ? "Stopping…" : "Stop"}</span>
              </button>
            </Show>
            <div class="topbar-utility-group">
              <Show when={ctx.activeProject() && ctx.gitRepoAvailable()}>
                <button
                  type="button"
                  class="topbar-icon-button"
                  classList={{ "topbar-icon-button-active": ctx.gitPanelOpen() }}
                  onClick={() => ctx.handleToggleGitPanel()}
                  title="Git"
                  aria-label="Git"
                  aria-pressed={ctx.gitPanelOpen()}
                  data-tauri-drag-region="false"
                >
                  <SidebarIcon name="git-branch" />
                </button>
              </Show>
              <button
                type="button"
                class="topbar-icon-button"
                classList={{ "topbar-icon-button-active": ctx.inspectorOpen() && Boolean(ctx.selectedNodeId()) }}
                onClick={() => ctx.handleToggleInspector()}
                title="Inspector"
                aria-label="Inspector"
                aria-pressed={ctx.inspectorOpen() && Boolean(ctx.selectedNodeId())}
                data-tauri-drag-region="false"
              >
                <SidebarIcon name="inspector" />
              </button>
              <button
                type="button"
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
                type="button"
                class="topbar-icon-button"
                onClick={() => void ctx.persistAll()}
                title={`Save (${mod()}+S)`}
                aria-label="Save workflow"
                data-tauri-drag-region="false"
              >
                <SidebarIcon name="save" />
              </button>
            </div>
          </div>
        </Show>
        <div
          class="readiness-chip"
          classList={{ ready: ctx.readiness()?.ready }}
        >
          <span class="status-dot" />
          <span>{ctx.readiness()?.message ?? "Checking provider"}</span>
        </div>
      </div>
    </header>
  );
}
