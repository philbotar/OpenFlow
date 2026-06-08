import ChevronRight from "lucide-solid/icons/chevron-right";
import Folder from "lucide-solid/icons/folder";
import { createEffect, createSignal, For, onCleanup, Show } from "solid-js";
import type { Project, Workflow } from "../../lib/types";
import { ICON_STROKE_WIDTH } from "../../lib/utils";
import { SidebarIcon } from "../SidebarIcon";
import { SidebarListRow } from "./SidebarListRow";

export type ProjectFolderRowProps = {
  project: Project;
  workflows: Workflow[];
  expanded: boolean;
  selected: boolean;
  activeWorkflowId: string | null;
  screen: string;
  editingWorkflowId: string | null;
  workflowNameDraft: string;
  onToggleExpand: () => void;
  onSelectProject: () => void;
  onSelectWorkflow: (workflowId: string) => void;
  onRenameWorkflow: (workflowId: string, name: string) => void;
  onCreateWorkflow: () => void;
  onAddExistingWorkflow: () => void;
  setWorkflowNameInputRef: (el: HTMLInputElement | undefined) => void;
  setWorkflowNameDraft: (value: string) => void;
  onWorkflowNameCommit: () => void;
  onWorkflowNameKeyDown: (event: KeyboardEvent) => void;
};

export function ProjectFolderRow(props: ProjectFolderRowProps) {
  const [menuOpen, setMenuOpen] = createSignal(false);
  let menuAnchor: HTMLDivElement | undefined;

  const closeMenu = () => setMenuOpen(false);

  createEffect(() => {
    if (!menuOpen()) return;

    const handlePointerDown = (event: PointerEvent) => {
      const target = event.target;
      if (!(target instanceof Node)) return;
      if (menuAnchor?.contains(target)) return;
      closeMenu();
    };

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") closeMenu();
    };

    document.addEventListener("pointerdown", handlePointerDown);
    document.addEventListener("keydown", handleKeyDown);
    onCleanup(() => {
      document.removeEventListener("pointerdown", handlePointerDown);
      document.removeEventListener("keydown", handleKeyDown);
    });
  });

  return (
    <div class="project-folder-group">
      <div
        class="project-folder-header"
        classList={{
          expanded: props.expanded,
          selected: props.selected,
        }}
      >
        <button
          type="button"
          class="project-folder-row"
          onClick={() => {
            props.onSelectProject();
            props.onToggleExpand();
          }}
          aria-expanded={props.expanded}
        >
          <ChevronRight
            class="project-folder-chevron"
            aria-hidden="true"
            absoluteStrokeWidth
            strokeWidth={ICON_STROKE_WIDTH}
          />
          <Folder
            class="project-folder-icon"
            aria-hidden="true"
            absoluteStrokeWidth
            strokeWidth={ICON_STROKE_WIDTH}
          />
          <span class="project-folder-title" title={props.project.path}>
            {props.project.name}
          </span>
        </button>
        <div class="project-folder-menu-anchor" ref={menuAnchor}>
          <button
            type="button"
            class="sidebar-icon-button project-folder-action"
            title="Add workflow"
            aria-label={`Add workflow to ${props.project.name}`}
            aria-haspopup="menu"
            aria-expanded={menuOpen()}
            onClick={(event) => {
              event.stopPropagation();
              setMenuOpen((open) => !open);
            }}
          >
            <SidebarIcon name="plus" />
          </button>
          <Show when={menuOpen()}>
            <div class="project-folder-menu" role="menu">
              <button
                type="button"
                class="project-folder-menu-item"
                role="menuitem"
                onClick={(event) => {
                  event.stopPropagation();
                  closeMenu();
                  props.onCreateWorkflow();
                }}
              >
                New workflow
              </button>
              <button
                type="button"
                class="project-folder-menu-item"
                role="menuitem"
                onClick={(event) => {
                  event.stopPropagation();
                  closeMenu();
                  props.onAddExistingWorkflow();
                }}
              >
                Add existing…
              </button>
            </div>
          </Show>
        </div>
      </div>
      <Show when={props.expanded}>
        <div class="project-workflow-list">
          <For each={props.workflows}>
            {(workflow) => {
              const active = () =>
                workflow.id === props.activeWorkflowId && props.screen === "editor";
              const editing = () => workflow.id === props.editingWorkflowId;
              return (
                <SidebarListRow
                  title={workflow.name}
                  active={active()}
                  editing={editing()}
                  onSelect={() => props.onSelectWorkflow(workflow.id)}
                  onRename={() => props.onRenameWorkflow(workflow.id, workflow.name)}
                  editSlot={
                    <input
                      ref={(el) => props.setWorkflowNameInputRef(el)}
                      value={props.workflowNameDraft}
                      onInput={(event) =>
                        props.setWorkflowNameDraft(event.currentTarget.value)
                      }
                      onBlur={props.onWorkflowNameCommit}
                      onKeyDown={props.onWorkflowNameKeyDown}
                      class="workflow-row-input"
                      aria-label={`Workflow name for ${workflow.name}`}
                    />
                  }
                />
              );
            }}
          </For>
        </div>
      </Show>
    </div>
  );
}
