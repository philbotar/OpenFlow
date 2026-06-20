import { For, Show } from "solid-js";
import { useAppContext } from "../context/AppContext";
import { PanelEmptyState } from "./PanelEmptyState";
import { workflowMembershipLabel } from "../lib/projects";
import { AnimatedModal } from "./AnimatedModal";

export function WorkflowPickerModal() {
  const ctx = useAppContext();
  const projectId = () => ctx.assignWorkflowPickerProjectId();
  const project = () => ctx.projects().find((item) => item.id === projectId()) ?? null;
  const addable = () =>
    projectId() ? ctx.workflowsAddableToProject(projectId()!) : [];

  return (
    <AnimatedModal
      open={Boolean(projectId())}
      onClose={ctx.closeAssignWorkflowPicker}
      ariaLabel="Add workflow to project"
      backdropClass="app-picker-backdrop"
    >
      <div class="node-picker-header">
        <div>
          <div class="eyebrow">Add workflow</div>
          <h3>{project()?.name ?? "Project"}</h3>
          <p>Copy a workflow into this project.</p>
        </div>
      </div>
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
      <div class="button-row end">
        <button class="secondary-button" onClick={ctx.closeAssignWorkflowPicker}>
          Cancel
        </button>
      </div>
    </AnimatedModal>
  );
}
