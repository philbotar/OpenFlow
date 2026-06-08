import { For, Show } from "solid-js";
import { useAppContext } from "../context/AppContext";
import { workflowMembershipLabel } from "../lib/projects";

export function WorkflowPickerModal() {
  const ctx = useAppContext();
  const projectId = () => ctx.assignWorkflowPickerProjectId();
  const project = () => ctx.projects().find((item) => item.id === projectId()) ?? null;
  const addable = () =>
    projectId() ? ctx.workflowsAddableToProject(projectId()!) : [];

  return (
    <Show when={projectId()}>
      <div class="node-picker-backdrop" onClick={ctx.closeAssignWorkflowPicker}>
        <section
          class="node-picker-card"
          role="dialog"
          aria-modal="true"
          aria-label="Add workflow to project"
          onClick={(event) => event.stopPropagation()}
        >
          <div class="node-picker-header">
            <div>
              <div class="eyebrow">Add workflow</div>
              <h3>{project()?.name ?? "Project"}</h3>
              <p>Link an existing workflow to this project.</p>
            </div>
          </div>
          <div class="node-picker-list">
            <Show
              when={addable().length > 0}
              fallback={
                <div class="node-picker-empty">
                  No other workflows available. Create a new one from the project menu.
                </div>
              }
            >
              <For each={addable()}>
                {(workflow) => (
                  <button
                    class="node-picker-option"
                    onClick={() =>
                      void ctx.handleAssignWorkflowToProject(projectId()!, workflow.id)
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
        </section>
      </div>
    </Show>
  );
}
