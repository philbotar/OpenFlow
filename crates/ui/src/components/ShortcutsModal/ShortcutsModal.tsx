import { For } from "solid-js";
import { AnimatedModal } from "../AnimatedModal";

const SHORTCUTS = [
  { keys: "⌘/Ctrl + Enter", action: "Run or continue workflow" },
  { keys: "⌘/Ctrl + .", action: "Stop workflow" },
  { keys: "⌘/Ctrl + S", action: "Save" },
  { keys: "⌘/Ctrl + 0", action: "Reset zoom" },
  { keys: "⌘/Ctrl + +", action: "Zoom in" },
  { keys: "⌘/Ctrl + -", action: "Zoom out" },
  { keys: "⌘/Ctrl + J", action: "Toggle right panel" },
  { keys: "Delete / Backspace", action: "Delete selected node or edge" },
  { keys: "?", action: "Show keyboard shortcuts" },
  { keys: "Escape", action: "Close modal" },
] as const;

interface ShortcutsModalProps {
  open: boolean;
  onClose: () => void;
}

export function ShortcutsModal(props: ShortcutsModalProps) {
  return (
    <AnimatedModal open={props.open} onClose={props.onClose} ariaLabel="Keyboard shortcuts">
      <div class="shortcuts-modal-header">
        <div class="eyebrow">Help</div>
        <h3>Keyboard shortcuts</h3>
      </div>
      <dl class="shortcuts-list">
        <For each={SHORTCUTS}>
          {(item) => (
            <>
              <dt>{item.keys}</dt>
              <dd>{item.action}</dd>
            </>
          )}
        </For>
      </dl>
      <div class="button-row end">
        <button type="button" class="secondary-button" onClick={() => props.onClose()}>
          Close
        </button>
      </div>
    </AnimatedModal>
  );
}
