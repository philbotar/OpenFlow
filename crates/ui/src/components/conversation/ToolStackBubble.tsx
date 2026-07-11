import { createSignal, Show, type JSX } from "solid-js";
import ChevronRight from "lucide-solid/icons/chevron-right";

/** Survive remounts while a live run appends tools to the same stack. */
const expandedByKey = new Map<string, boolean>();

/** Test-only: clear persisted expand state between cases. */
export function resetToolStackExpandStateForTests(): void {
  expandedByKey.clear();
}

export interface ToolStackBubbleProps {
  summaryText: string;
  /** Stable id for this stack (e.g. nodeId + first toolCallId). */
  persistKey?: string;
  children: JSX.Element;
}

export function ToolStackBubble(props: ToolStackBubbleProps) {
  const initial = () =>
    props.persistKey ? (expandedByKey.get(props.persistKey) ?? false) : false;
  const [expanded, setExpanded] = createSignal(initial());

  const setExpandedPersist = (next: boolean | ((value: boolean) => boolean)) => {
    setExpanded((current) => {
      const value = typeof next === "function" ? next(current) : next;
      if (props.persistKey) {
        if (value) expandedByKey.set(props.persistKey, true);
        else expandedByKey.delete(props.persistKey);
      }
      return value;
    });
  };

  return (
    <div
      class="tool-line tool-line--expandable tool-stack"
      data-expanded={expanded() ? "true" : undefined}
    >
      <div
        class="tool-line-status-row tool-stack-status-row"
        onClick={() => setExpandedPersist((value) => !value)}
      >
        <span class="tool-line-name">
          <span class="tool-line-name-text">{props.summaryText}</span>
          <button
            type="button"
            class="tool-line-chevron"
            classList={{ "tool-line-chevron--expanded": expanded() }}
            aria-expanded={expanded()}
            aria-label={expanded() ? "Collapse tool stack" : "Expand tool stack"}
            onClick={(event) => {
              event.stopPropagation();
              setExpandedPersist((value) => !value);
            }}
          >
            <ChevronRight width={14} height={14} />
          </button>
        </span>
      </div>
      <Show when={expanded()}>
        <div class="tool-stack-children">{props.children}</div>
      </Show>
    </div>
  );
}
