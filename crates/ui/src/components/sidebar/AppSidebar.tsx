import { createEffect, createSignal, For, onCleanup, Show } from "solid-js";
import { useAppContext } from "../../context/AppContext";
import { isMacOS, ICON_STROKE_WIDTH } from "../../lib/utils";
import ChevronRight from "lucide-solid/icons/chevron-right";
import { ChatHistoryRow } from "./ChatHistoryRow";
import { ProjectFolderRow } from "./ProjectFolderRow";
import { SidebarList } from "./SidebarList";
import { SidebarIconButton } from "./SidebarIconButton";
import { SidebarNavButton } from "./SidebarNavButton";
import { WorkflowListRow } from "./WorkflowListRow";
import { CollapsibleSection } from "../CollapsibleSection";
import { Tooltip } from "../Tooltip";

function WorkflowRows() {
  const ctx = useAppContext();

  return (
    <For each={ctx.independentWorkflows()}>
      {(workflow) => {
        const active = () =>
          workflow.id === ctx.activeWorkflowId() && ctx.screen() === "editor";
        const editing = () => workflow.id === ctx.editingWorkflowId();
        return (
          <WorkflowListRow
            title={workflow.name}
            active={active()}
            editing={editing()}
            onSelect={() => ctx.handleSwitchWorkflow(workflow.id)}
            onRename={() =>
              ctx.handleStartWorkflowNameEdit(workflow.id, workflow.name)
            }
            onDelete={() => void ctx.handleDeleteWorkflow(workflow.id)}
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

function ChatRows() {
  const ctx = useAppContext();

  return (
    <For each={ctx.chats()}>
      {(chat) => (
        <ChatHistoryRow
          title={chat.title}
          active={chat.id === ctx.activeChatId() && ctx.screen() === "chat"}
          onSelect={() => void ctx.handleOpenChat(chat.id)}
          onDelete={() => void ctx.handleDeleteChat(chat.id)}
        />
      )}
    </For>
  );
}

export function Sidebar() {
  const ctx = useAppContext();
  const [newWorkflowMenuOpen, setNewWorkflowMenuOpen] = createSignal(false);
  let newWorkflowMenuAnchor: HTMLDivElement | undefined;

  const closeNewWorkflowMenu = () => setNewWorkflowMenuOpen(false);

  createEffect(() => {
    if (!newWorkflowMenuOpen()) return;

    const handlePointerDown = (event: PointerEvent) => {
      const target = event.target;
      if (!(target instanceof Node)) return;
      if (newWorkflowMenuAnchor?.contains(target)) return;
      closeNewWorkflowMenu();
    };

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") closeNewWorkflowMenu();
    };

    document.addEventListener("pointerdown", handlePointerDown);
    document.addEventListener("keydown", handleKeyDown);
    onCleanup(() => {
      document.removeEventListener("pointerdown", handlePointerDown);
      document.removeEventListener("keydown", handleKeyDown);
    });
  });

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
        <div class="sidebar-new-workflow-menu" ref={newWorkflowMenuAnchor}>
          <SidebarNavButton
            icon="plus"
            label="New workflow"
            ariaHasPopup="menu"
            ariaExpanded={newWorkflowMenuOpen()}
            onClick={() => setNewWorkflowMenuOpen((open) => !open)}
          />
          <Show when={newWorkflowMenuOpen()}>
            <div
              class="sidebar-new-workflow-popover"
              role="menu"
              aria-label="New workflow options"
            >
              <button
                type="button"
                class="sidebar-new-workflow-menu-item"
                role="menuitem"
                onClick={() => {
                  closeNewWorkflowMenu();
                  void ctx.handleCreateWorkflow();
                }}
              >
                Create new workflow
              </button>
              <button
                type="button"
                class="sidebar-new-workflow-menu-item"
                role="menuitem"
                onClick={() => {
                  closeNewWorkflowMenu();
                  void ctx.handleOpenWorkflowAuthoring();
                }}
              >
                Create with AI
              </button>
            </div>
          </Show>
        </div>
        <SidebarNavButton
          icon="chat"
          label="New chat"
          onClick={() => void ctx.handleCreateChat()}
        />
        <Show
          when={
            ctx.workflowAuthoringSessionReady() &&
            ctx.screen() !== "workflow-authoring"
          }
        >
          <SidebarNavButton
            icon="sparkles"
            label="Resume AI workflow draft"
            onClick={() => void ctx.handleOpenWorkflowAuthoring()}
          />
        </Show>
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
        <div class="sidebar-section-group sidebar-chats-section" aria-label="Chats">
          <div class="sidebar-section-header workflows-section-header">
            <div class="sidebar-section-label">Chats</div>
            <div class="sidebar-section-trailing">
              <Tooltip label="Toggle chats section">
                <button
                  type="button"
                  class="workflows-section-chevron-btn"
                  onClick={ctx.handleToggleChatsSection}
                  aria-expanded={ctx.chatsSectionExpanded()}
                  aria-label="Toggle chats section"
                >
                  <ChevronRight
                    class="workflows-section-chevron"
                    aria-hidden="true"
                    absoluteStrokeWidth
                    strokeWidth={ICON_STROKE_WIDTH}
                  />
                </button>
              </Tooltip>
            </div>
          </div>
          <CollapsibleSection
            open={ctx.chatsSectionExpanded()}
            class="sidebar-chats-collapsible"
          >
            <ChatRows />
          </CollapsibleSection>
        </div>
        <div class="sidebar-section-group sidebar-workflows-section">
          <div class="sidebar-section-header workflows-section-header">
            <div class="sidebar-section-label">Workflows</div>
            <div class="sidebar-section-trailing">
              <Tooltip label="Toggle workflows section">
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
              </Tooltip>
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
            <CollapsibleSection
              open={ctx.workflowsSectionExpanded()}
              class="sidebar-workflows-collapsible"
            >
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
              <Tooltip label="Toggle projects section">
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
              </Tooltip>
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
                    onDeleteWorkflow={(workflowId) =>
                      void ctx.handleDeleteWorkflow(workflowId)
                    }
                    onCreateWorkflow={() => void ctx.handleCreateWorkflow(project.id)}
                    onCreateWorkflowWithAi={() =>
                      void ctx.handleOpenWorkflowAuthoring(undefined, project.id)
                    }
                    onAddExistingWorkflow={() => ctx.handleOpenAssignWorkflowPicker(project.id)}
                    onRemoveProject={() => void ctx.handleRemoveProject(project.id)}
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
