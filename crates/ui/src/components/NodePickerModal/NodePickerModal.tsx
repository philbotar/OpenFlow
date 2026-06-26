import { For, Show } from "solid-js";
import { useAppContext } from "../../context/AppContext";
import { PickerModal } from "../PickerModal";

export function NodePickerModal() {
  const ctx = useAppContext();

  return (
    <PickerModal
      open={ctx.addNodePickerOpen()}
      onClose={ctx.closeAddNodePicker}
      ariaLabel="Add agent node"
      eyebrow="Add node"
      title="Choose a starting point"
      description="Start blank or reuse one of your saved agents."
    >
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
    </PickerModal>
  );
}
