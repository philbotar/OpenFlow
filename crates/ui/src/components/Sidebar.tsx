import { For, Show } from "solid-js";
import { useAppContext } from "../context/AppContext";
import { SidebarIcon } from "./SidebarIcon";
import { isMacOS } from "../lib/utils";
import { formatUiZoomLabel as fmtZoom } from "../lib/uiZoom";

export function Sidebar() {
  const ctx = useAppContext();

  return (
    <aside
      class="sidebar"
      classList={{
        "sidebar-macos": isMacOS(),
        "sidebar-maximized": ctx.isMaximized(),
      }}
    >
      <Show when={isMacOS()}>
        <div
          class="sidebar-window-controls-spacer"
          aria-hidden="true"
          data-tauri-drag-region
        />
      </Show>
      <div class="sidebar-brand" data-tauri-drag-region>
        <div class="brand-mark" aria-hidden="true" />
        <div class="brand-copy">
          <span class="brand-title">OpenFlow</span>
        </div>
      </div>
      <div class="sidebar-list">
        <button
          class="sidebar-nav-button"
          classList={{ active: ctx.screen() === "agents" }}
          onClick={ctx.handleOpenAgents}
        >
          <SidebarIcon name="agents" />
          <span>Agents</span>
        </button>
        <button
          class="sidebar-nav-button"
          onClick={() => void ctx.handleCreateWorkflow()}
        >
          <SidebarIcon name="plus" />
          <span>New workflow</span>
        </button>
        <For each={ctx.workflows()}>
          {(workflow) => {
            const active = () =>
              workflow.id === ctx.activeWorkflowId() && ctx.screen() === "editor";
            const editing = () => workflow.id === ctx.editingWorkflowId();
            return (
              <div
                class="workflow-row"
                classList={{ active: active(), editing: editing() }}
              >
                <Show
                  when={!editing()}
                  fallback={
                    <div class="workflow-row-main">
                      <input
                        ref={(el) => ctx.setWorkflowNameInputRef(el)}
                        value={ctx.workflowNameDraft()}
                        onInput={(event) =>
                          ctx.setWorkflowNameDraft(event.currentTarget.value)
                        }
                        onBlur={ctx.handleWorkflowNameCommit}
                        onKeyDown={ctx.handleWorkflowNameKeyDown}
                        class="workflow-row-input"
                        aria-label={`Workflow name for ${workflow.name}`}
                      />
                    </div>
                  }
                >
                  <button
                    type="button"
                    class="workflow-row-main"
                    onClick={() => ctx.handleSwitchWorkflow(workflow.id)}
                  >
                    <div class="workflow-row-details">
                      <span class="workflow-row-title">{workflow.name}</span>
                    </div>
                  </button>
                </Show>
                <button
                  type="button"
                  class="sidebar-icon-button workflow-row-action hover-show"
                  onClick={() =>
                    ctx.handleStartWorkflowNameEdit(workflow.id, workflow.name)
                  }
                  title="Rename workflow"
                  aria-label={`Rename ${workflow.name}`}
                >
                  <SidebarIcon name="edit" />
                </button>
              </div>
            );
          }}
        </For>
      </div>
      <div class="sidebar-footer">
        <div class="settings-nav-menu">
          <button
            class="sidebar-nav-button"
            onClick={() => {
              ctx.closeAddNodePicker();
              ctx.setScreen(ctx.screen() === "settings" ? "editor" : "settings");
            }}
          >
            <SidebarIcon name="settings" />
            <span>{ctx.screen() === "settings" ? "Back to editor" : "Settings"}</span>
          </button>
          <div class="settings-nav-popup" aria-hidden="true">
            Zoom {fmtZoom(ctx.uiZoom())}
          </div>
        </div>
      </div>
    </aside>
  );
}
