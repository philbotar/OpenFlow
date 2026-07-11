import { createSignal, Show } from "solid-js";
import ChevronRight from "lucide-solid/icons/chevron-right";
import type { ToolCallStatus } from "../../lib/types";
import { prettyJsonText } from "../../lib/utils";
import { toolBubbleLineText } from "./toolBubbleState";

export interface ToolBubbleProps {
  toolName: string;
  status: ToolCallStatus;
  output: string | null | undefined;
  arguments?: unknown;
  intent?: string | null;
  isError?: boolean;
  streaming?: boolean;
  /** Execution cwd — strips absolute path prefixes in the label. */
  cwd?: string | null;
}

export function ToolBubble(props: ToolBubbleProps) {
  const [expanded, setExpanded] = createSignal(false);
  const lineText = () =>
    toolBubbleLineText(
      props.toolName,
      props.status,
      props.arguments,
      props.intent,
      props.cwd,
    );
  const hasOutput = () => Boolean(props.output?.trim());
  const expandable = () => hasOutput() || props.streaming;
  const displayOutput = () =>
    props.output ? prettyJsonText(props.output) : props.output;
  const previewText = () => {
    if (!props.streaming || !props.output) return "";
    const text = props.output.trimEnd();
    return text.length > 120 ? text.slice(-120) : text;
  };

  return (
    <div
      class="tool-line"
      classList={{ "tool-line--expandable": expandable() }}
      data-tool-name={props.toolName}
      data-status={props.status}
      data-streaming={props.streaming ? "true" : undefined}
    >
      <div
        class="tool-line-status-row"
        onClick={() => {
          if (expandable()) {
            setExpanded((value) => !value);
          }
        }}
      >
        <span class="tool-line-name">
          <span class="tool-line-name-text">{lineText()}</span>
          <Show when={expandable()}>
            <button
              type="button"
              class="tool-line-chevron"
              classList={{ "tool-line-chevron--expanded": expanded() }}
              aria-expanded={expanded()}
              aria-label={expanded() ? "Collapse tool output" : "Expand tool output"}
              onClick={(event) => {
                event.stopPropagation();
                setExpanded((value) => !value);
              }}
            >
              <ChevronRight width={14} height={14} />
            </button>
          </Show>
        </span>
      </div>
      <Show when={props.streaming && !expanded() && previewText()}>
        <div class="tool-line-preview">
          <span class="tool-line-preview-text">{previewText()}</span>
        </div>
      </Show>
      <Show when={expandable() && expanded()}>
        <div class="tool-line-output-wrapper tool-line-output-wrapper--expanded">
          <pre
            class="tool-line-output"
            classList={{ "tool-line-output--error": props.isError }}
          >
            {displayOutput()}
          </pre>
        </div>
      </Show>
    </div>
  );
}
