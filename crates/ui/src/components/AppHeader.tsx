import { Show } from "solid-js";
import { useAppContext } from "../context/AppContext";
import { SidebarIcon } from "./SidebarIcon";
import { Spinner } from "./Spinner";
import { Tooltip } from "./Tooltip";
import { isMacOS } from "../lib/utils";

export function AppHeader() {
  const ctx = useAppContext();

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

  const runDisabledReason = () =>
    ctx.readiness()?.ready
      ? undefined
      : (ctx.readiness()?.message ?? "Add an API key in Settings to run workflows");

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
        <Show when={ctx.isCompactViewport() && ctx.screen() !== "settings"}>
          <Tooltip label="Open navigation">
            <button
              type="button"
              class="topbar-nav-button topbar-sidebar-toggle"
              classList={{
                "topbar-sidebar-toggle--panel-hidden": !ctx.sidebarDrawerOpen(),
              }}
              onClick={() => ctx.toggleSidebarDrawer()}
              aria-label="Open navigation"
              aria-expanded={ctx.sidebarDrawerOpen()}
              data-tauri-drag-region="false"
            >
              <SidebarIcon name="panel-left-close" />
            </button>
          </Tooltip>
        </Show>
        <Show when={!ctx.isCompactViewport() && ctx.screen() !== "settings"}>
          <Tooltip
            label={ctx.leftPanelHidden() ? "Show sidebar" : "Hide sidebar"}
            shortcutId="toggleLeftSidebar"
          >
            <button
              type="button"
              class="topbar-icon-button topbar-sidebar-toggle"
              classList={{
                "topbar-sidebar-toggle--panel-hidden": ctx.leftPanelHidden(),
              }}
              onClick={() => ctx.handleToggleLeftPanel()}
              aria-label={ctx.leftPanelHidden() ? "Show left sidebar" : "Hide left sidebar"}
              data-tauri-drag-region="false"
            >
              <SidebarIcon name="panel-left-close" />
            </button>
          </Tooltip>
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
            <div class="topbar-primary-actions">
              <Show
                when={ctx.runState()?.active}
                fallback={
                  <Show
                    when={ctx.continuableRun()}
                    fallback={
                      <Tooltip
                        label="Run workflow without a starter message"
                        shortcutId="run"
                        disabledReason={runDisabledReason()}
                      >
                        <button
                          type="button"
                          class="topbar-primary-button"
                          classList={{ "topbar-icon-button--loading": ctx.startingRun() }}
                          onClick={() => void ctx.handleRun()}
                          disabled={
                            !ctx.readiness()?.ready ||
                            ctx.startingRun() ||
                            ctx.replayRunId() !== null ||
                            !ctx.activeWorkflow()
                          }
                          aria-label="Run workflow"
                          data-tauri-drag-region="false"
                        >
                          <Show when={ctx.startingRun()} fallback={<SidebarIcon name="run" />}>
                            <Spinner size="sm" />
                          </Show>
                          <span>{ctx.startingRun() ? "Starting…" : "Run"}</span>
                        </button>
                      </Tooltip>
                    }
                  >
                    <Tooltip
                      label="Continue the paused workflow run"
                      shortcutId="run"
                      disabledReason={runDisabledReason()}
                    >
                      <button
                        type="button"
                        class="topbar-primary-button"
                        classList={{ "topbar-icon-button--loading": ctx.startingRun() }}
                        onClick={() => void ctx.handleContinueRun()}
                        disabled={
                          !ctx.readiness()?.ready ||
                          ctx.startingRun() ||
                          ctx.replayRunId() !== null
                        }
                        aria-label="Continue workflow"
                        data-tauri-drag-region="false"
                      >
                        <Show when={ctx.startingRun()} fallback={<SidebarIcon name="run" />}>
                          <Spinner size="sm" />
                        </Show>
                        <span>{ctx.startingRun() ? "Starting…" : "Continue"}</span>
                      </button>
                    </Tooltip>
                  </Show>
                }
              >
                <Tooltip label="Stop" shortcutId="stop">
                  <button
                    type="button"
                    class="topbar-danger-button"
                    classList={{ "topbar-icon-button--loading": ctx.stoppingRun() }}
                    onClick={() => void ctx.handleStopRun()}
                    disabled={ctx.stoppingRun()}
                    aria-label="Stop workflow"
                    data-tauri-drag-region="false"
                  >
                    <Show when={ctx.stoppingRun()} fallback={<SidebarIcon name="stop" />}>
                      <Spinner size="sm" />
                    </Show>
                    <span>{ctx.stoppingRun() ? "Stopping…" : "Stop"}</span>
                  </button>
                </Tooltip>
              </Show>
            </div>
            <div class="topbar-utility-group">
              <Show when={ctx.activeProject() && ctx.gitRepoAvailable()}>
                <Tooltip label="Git">
                  <button
                    type="button"
                    class="topbar-icon-button"
                    classList={{ "topbar-icon-button-active": ctx.gitPanelOpen() }}
                    onClick={() => ctx.handleToggleGitPanel()}
                    aria-label="Git"
                    aria-pressed={ctx.gitPanelOpen()}
                    data-tauri-drag-region="false"
                  >
                    <SidebarIcon name="git-branch" />
                  </button>
                </Tooltip>
              </Show>
              <Tooltip label="Inspector" shortcutId="toggleInspector">
                <button
                  type="button"
                  class="topbar-icon-button"
                  classList={{ "topbar-icon-button-active": ctx.inspectorOpen() && Boolean(ctx.selectedNodeId()) }}
                  onClick={() => ctx.handleToggleInspector()}
                  aria-label="Inspector"
                  aria-pressed={ctx.inspectorOpen() && Boolean(ctx.selectedNodeId())}
                  data-tauri-drag-region="false"
                >
                  <SidebarIcon name="inspector" />
                </button>
              </Tooltip>
              <Tooltip label="Workflow settings">
                <button
                  type="button"
                  class="topbar-icon-button"
                  classList={{ "topbar-icon-button-active": ctx.workflowSettingsOpen() }}
                  onClick={() => ctx.handleToggleWorkflowSettings()}
                  aria-label="Workflow settings"
                  aria-pressed={ctx.workflowSettingsOpen()}
                  data-tauri-drag-region="false"
                >
                  <SidebarIcon name="settings" />
                </button>
              </Tooltip>
              <Tooltip label="Save" shortcutId="save">
                <button
                  type="button"
                  class="topbar-icon-button"
                  onClick={() => void ctx.persistAll()}
                  aria-label="Save workflow"
                  data-tauri-drag-region="false"
                >
                  <SidebarIcon name="save" />
                </button>
              </Tooltip>
            </div>
          </div>
        </Show>
        <div
          class="readiness-chip"
          classList={{ ready: ctx.readiness()?.ready }}
          title={ctx.readiness()?.message ?? "Checking API key and provider settings"}
          role="status"
        >
          <span class="status-dot" aria-hidden="true" />
          <span>{ctx.readiness()?.message ?? "Checking API key…"}</span>
        </div>
      </div>
    </header>
  );
}
