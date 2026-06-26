import { For, Show } from "solid-js";
import { useAppContext } from "../../context/AppContext";
import { isMacOS, ICON_STROKE_WIDTH } from "../../lib/utils";
import ChevronRight from "lucide-solid/icons/chevron-right";
import { ProjectFolderRow } from "./ProjectFolderRow";
import { SidebarList } from "./SidebarList";
import { SidebarListRow } from "./SidebarListRow";
import { SidebarIconButton } from "./SidebarIconButton";
import { SidebarNavButton } from "./SidebarNavButton";
import { CollapsibleSection } from "../CollapsibleSection";

function WorkflowRows() {
  const ctx = useAppContext();

  return (
    <For each={ctx.independentWorkflows()}>
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
  );
}

export function Sidebar() {
  const ctx = useAppContext();

  return (
    <aside
      class="sidebar"
      classList={{
        "sidebar-macos": isMacOS(),
        "sidebar-maximized": ctx.isMaximized(),
      }}
      aria-hidden={ctx.leftPanelHidden() && !ctx.isCompactViewport()}
    >
      <SidebarList>
        <SidebarNavButton
          icon="agents"
          label="Agents"
          active={ctx.screen() === "agents"}
          onClick={ctx.handleOpenAgents}
        />
        <SidebarNavButton
          icon="schedule"
          label="Schedule"
          active={ctx.screen() === "schedule"}
          onClick={ctx.handleOpenSchedule}
        />
        <div class="sidebar-section-group">
          <div class="sidebar-section-header workflows-section-header">
            <div class="sidebar-section-label">Workflows</div>
            <div class="sidebar-section-trailing">
              <button
                type="button"
                class="workflows-section-chevron-btn"
                onClick={ctx.handleToggleWorkflowsSection}
                aria-expanded={ctx.workflowsSectionExpanded()}
                aria-label="Toggle workflows section"
              >
                <ChevronRight
                  class="workflows-section-chevron"
                  aria-hidden="true"
                  absoluteStrokeWidth
                  strokeWidth={ICON_STROKE_WIDTH}
                />
              </button>
              <SidebarIconButton
                icon="sparkles"
                label="Build with AI"
                class="sidebar-section-action"
                active={ctx.screen() === "workflow-authoring"}
                onClick={() => void ctx.handleOpenWorkflowAuthoring()}
              />
              <SidebarIconButton
                icon="plus"
                label="New workflow"
                class="sidebar-section-action"
                onClick={() => void ctx.handleCreateWorkflow()}
              />
            </div>
          </div>
          <Show
            when={ctx.appReady()}
            fallback={
              <div class="sidebar-skeleton" aria-hidden="true">
                <span class="skeleton-line" />
                <span class="skeleton-line" />
                <span class="skeleton-line" />
              </div>
            }
          >
            <CollapsibleSection open={ctx.workflowsSectionExpanded()}>
              <WorkflowRows />
            </CollapsibleSection>
          </Show>
        </div>
        <div
          class="sidebar-section-group sidebar-projects-section"
          classList={{ "sidebar-projects-section--expanded": ctx.projectsSectionExpanded() }}
        >
          <div class="sidebar-section-header workflows-section-header">
            <div class="sidebar-section-label">Projects</div>
            <div class="sidebar-section-trailing">
              <button
                type="button"
                class="workflows-section-chevron-btn"
                onClick={ctx.handleToggleProjectsSection}
                aria-expanded={ctx.projectsSectionExpanded()}
                aria-label="Toggle projects section"
              >
                <ChevronRight
                  class="workflows-section-chevron"
                  aria-hidden="true"
                  absoluteStrokeWidth
                  strokeWidth={ICON_STROKE_WIDTH}
                />
              </button>
              <SidebarIconButton
                icon="plus"
                label="Add project"
                class="sidebar-section-action"
                onClick={() => void ctx.handleAddProject()}
              />
            </div>
          </div>
          <CollapsibleSection open={ctx.projectsSectionExpanded()} class="sidebar-projects-collapsible">
            <div class="sidebar-projects-scroll">
              <For each={ctx.projects()}>
                {(project) => (
                  <ProjectFolderRow
                    project={project}
                    workflows={ctx.workflowsForProject(project)}
                    expanded={ctx.isProjectExpanded(project.id)}
                    selected={ctx.selectedProjectId() === project.id}
                    activeWorkflowId={ctx.activeWorkflowId()}
                    screen={ctx.screen()}
                    editingWorkflowId={ctx.editingWorkflowId()}
                    workflowNameDraft={ctx.workflowNameDraft()}
                    onToggleExpand={() => ctx.handleToggleProjectExpanded(project.id)}
                    onSelectProject={() => ctx.handleSelectProject(project.id)}
                    onSelectWorkflow={(workflowId) => {
                      ctx.handleSelectProject(project.id);
                      ctx.handleSwitchWorkflow(workflowId);
                    }}
                    onRenameWorkflow={ctx.handleStartWorkflowNameEdit}
                    onCreateWorkflow={() => void ctx.handleCreateWorkflow(project.id)}
                    onAddExistingWorkflow={() => ctx.handleOpenAssignWorkflowPicker(project.id)}
                    setWorkflowNameInputRef={ctx.setWorkflowNameInputRef}
                    setWorkflowNameDraft={ctx.setWorkflowNameDraft}
                    onWorkflowNameCommit={ctx.handleWorkflowNameCommit}
                    onWorkflowNameKeyDown={ctx.handleWorkflowNameKeyDown}
                  />
                )}
              </For>
            </div>
          </CollapsibleSection>
        </div>
      </SidebarList>
      <div class="sidebar-footer">
        <div class="settings-nav-menu">
          <SidebarNavButton
            icon="help"
            label="Shortcuts"
            onClick={() => ctx.openShortcutsModal()}
          />
          <SidebarNavButton
            icon="settings"
            label="Settings"
            updateAvailable={ctx.appUpdateAvailable()}
            onClick={() => {
              ctx.closeAddNodePicker();
              ctx.navigateToScreen("settings");
            }}
          />
        </div>
      </div>
    </aside>
  );
}
