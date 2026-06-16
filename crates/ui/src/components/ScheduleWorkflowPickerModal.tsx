import { For, Show, createMemo, createSignal } from "solid-js";
import type { Workflow } from "../lib/types";
import { AnimatedModal } from "./AnimatedModal";

interface ScheduleWorkflowPickerModalProps {
  open: boolean;
  workflows: Workflow[];
  onClose: () => void;
  onSelect: (workflowId: string) => void;
}

export function ScheduleWorkflowPickerModal(props: ScheduleWorkflowPickerModalProps) {
  const [search, setSearch] = createSignal("");
  const filteredWorkflows = createMemo(() => {
    const query = search().trim().toLowerCase();
    if (!query) return props.workflows;
    return props.workflows.filter((workflow) =>
      workflow.name.toLowerCase().includes(query),
    );
  });

  const close = () => {
    setSearch("");
    props.onClose();
  };

  const select = (workflowId: string) => {
    props.onSelect(workflowId);
    setSearch("");
  };

  return (
    <AnimatedModal
      open={props.open}
      onClose={close}
      ariaLabel="Add workflow to schedule"
    >
      <div class="node-picker-header">
        <div>
          <div class="eyebrow">Add workflow</div>
          <h3>Choose a workflow</h3>
          <p>Pick an unscheduled workflow to add to the automation schedule.</p>
        </div>
      </div>
      <input
        class="text-input node-picker-search"
        value={search()}
        placeholder="Search workflows"
        onInput={(event) => setSearch(event.currentTarget.value)}
      />
      <div class="node-picker-list">
        <Show
          when={filteredWorkflows().length > 0}
          fallback={
            <div class="node-picker-empty">
              {props.workflows.length === 0
                ? "All workflows are already scheduled."
                : "No unscheduled workflows match."}
            </div>
          }
        >
          <For each={filteredWorkflows()}>
            {(workflow) => (
              <button
                class="node-picker-option"
                type="button"
                onClick={() => select(workflow.id)}
              >
                <span class="node-picker-option-title">{workflow.name}</span>
              </button>
            )}
          </For>
        </Show>
      </div>
      <div class="button-row end">
        <button class="secondary-button" type="button" onClick={close}>
          Cancel
        </button>
      </div>
    </AnimatedModal>
  );
}
