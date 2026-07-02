import { For, Show } from "solid-js";
import { useAppContext } from "../context/AppContext";
import { PanelEmptyState } from "./PanelEmptyState";
import { workflowMembershipLabel } from "../lib/projects";
import { PickerModal } from "./PickerModal";

export function WorkflowPickerModal() {
  const ctx = useAppContext();
  const projectId = () => ctx.assignWorkflowPickerProjectId();
  const project = () => ctx.projects().find((item) => item.id === projectId()) ?? null;
  const addable = () =>
    projectId() ? ctx.workflowsAddableToProject(projectId()!) : [];

  return (
    <PickerModal
      open={Boolean(projectId())}
      onClose={ctx.closeAssignWorkflowPicker}
      ariaLabel="Add workflow to project"
      backdropClass="app-picker-backdrop"
      eyebrow="Add workflow"
      title={project()?.name ?? "Project"}
      description="Copy a workflow into this project."
    >
      <div class="node-picker-list">
        <Show
          when={addable().length > 0}
          fallback={
            <PanelEmptyState
              title="No workflows to copy"
              description="Create a workflow from the project menu, then add it here."
            />
          }
        >
          <For each={addable()}>
            {(workflow) => (
              <button
                class="node-picker-option"
                onClick={() =>
                  void ctx.handleCopyWorkflowToProject(projectId()!, workflow.id)
                }
              >
                <span class="node-picker-option-title">{workflow.name}</span>
                <span class="node-picker-option-copy">
                  {workflowMembershipLabel(ctx.projects(), workflow.id)}
                </span>
              </button>
            )}
          </For>
        </Show>
      </div>
    </PickerModal>
  );
}
