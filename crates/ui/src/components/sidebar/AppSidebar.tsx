import { For, Show } from "solid-js";
import { useAppContext } from "../../context/AppContext";
import { isMacOS } from "../../lib/utils";
import { formatUiZoomLabel as fmtZoom } from "../../lib/uiZoom";
import { SidebarList } from "./SidebarList";
import { SidebarListRow } from "./SidebarListRow";
import { SidebarNavButton } from "./SidebarNavButton";

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
      <SidebarList>
        <SidebarNavButton
          icon="agents"
          label="Agents"
          active={ctx.screen() === "agents"}
          onClick={ctx.handleOpenAgents}
        />
        <SidebarNavButton
          icon="plus"
          label="New workflow"
          onClick={() => void ctx.handleCreateWorkflow()}
        />
        <For each={ctx.workflows()}>
          {(workflow) => {
            const active = () =>
              workflow.id === ctx.activeWorkflowId() && ctx.screen() === "editor";
            const editing = () => workflow.id === ctx.editingWorkflowId();
            return (
              <SidebarListRow
                title={workflow.name}
                active={active()}
                editing={editing()}
                onSelect={() => ctx.handleSwitchWorkflow(workflow.id)}
                onRename={() =>
                  ctx.handleStartWorkflowNameEdit(workflow.id, workflow.name)
                }
                editSlot={
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
                }
              />
            );
          }}
        </For>
      </SidebarList>
      <div class="sidebar-footer">
        <div class="settings-nav-menu">
          <SidebarNavButton
            icon="settings"
            label={ctx.screen() === "settings" ? "Back to editor" : "Settings"}
            onClick={() => {
              ctx.closeAddNodePicker();
              ctx.setScreen(ctx.screen() === "settings" ? "editor" : "settings");
            }}
          />
          <div class="settings-nav-popup" aria-hidden="true">
            Zoom {fmtZoom(ctx.uiZoom())}
          </div>
        </div>
      </div>
    </aside>
  );
}
