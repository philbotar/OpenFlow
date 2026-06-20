import { For, Show } from "solid-js";
import { useAppContext } from "../../context/AppContext";
import { AnimatedModal } from "../AnimatedModal";

export function NodePickerModal() {
  const ctx = useAppContext();

  return (
    <AnimatedModal
      open={ctx.addNodePickerOpen()}
      onClose={ctx.closeAddNodePicker}
      ariaLabel="Add agent node"
    >
      <div class="node-picker-header">
        <div>
          <div class="eyebrow">Add node</div>
          <h3>Choose a starting point</h3>
          <p>Start blank or reuse one of your saved agents.</p>
        </div>
      </div>
      <div class="node-picker-list">
        <button
          class="node-picker-option"
          onClick={() => void ctx.handleAddNode(null)}
        >
          <span class="node-picker-option-title">Blank agent node</span>
          <span class="node-picker-option-copy">
            Start with the default prompts, schema, and tool access.
          </span>
        </button>
        <Show
          when={ctx.agents().length > 0}
          fallback={
            <div class="node-picker-empty">
              No saved agents yet. Create one in the Agents screen.
            </div>
          }
        >
          <For each={ctx.agents()}>
            {(agent) => (
              <button
                class="node-picker-option"
                onClick={() => void ctx.handleAddNode(agent.id)}
              >
                <span class="node-picker-option-title">
                  {agent.name || "Untitled agent"}
                </span>
                <span class="node-picker-option-copy">
                  {agent.model || "No model selected"}
                </span>
              </button>
            )}
          </For>
        </Show>
      </div>
      <div class="button-row end">
        <button class="secondary-button" onClick={ctx.closeAddNodePicker}>
          Cancel
        </button>
      </div>
    </AnimatedModal>
  );
}
