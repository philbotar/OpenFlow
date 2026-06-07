import ChevronRight from "lucide-solid/icons/chevron-right";
import Folder from "lucide-solid/icons/folder";
import { For, Show } from "solid-js";
import type { Project, Workflow } from "../../lib/types";
import { ICON_STROKE_WIDTH } from "../../lib/utils";
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
  setWorkflowNameInputRef: (el: HTMLInputElement | undefined) => void;
  setWorkflowNameDraft: (value: string) => void;
  onWorkflowNameCommit: () => void;
  onWorkflowNameKeyDown: (event: KeyboardEvent) => void;
};

export function ProjectFolderRow(props: ProjectFolderRowProps) {
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
