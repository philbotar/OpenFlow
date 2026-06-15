import { createSignal, Show } from "solid-js";
import ChevronRight from "lucide-solid/icons/chevron-right";
import type { ToolCallStatus } from "../../lib/types";
import { toolBubbleRowStatusText, toolBubbleTargetText } from "./toolBubbleState";

export interface ToolBubbleProps {
  toolName: string;
  status: ToolCallStatus;
  output: string | null | undefined;
  arguments?: unknown;
  intent?: string | null;
  isError?: boolean;
  streaming?: boolean;
}

function statusIcon(status: ToolCallStatus): { class: string; label: string } {
  switch (status) {
    case "proposed":
      return { class: "tool-line-status--muted", label: "" };
    case "running":
      return { class: "tool-line-status--running", label: "" };
    case "completed":
      return { class: "tool-line-status--success", label: "✓" };
    case "failed":
      return { class: "tool-line-status--error", label: "✗" };
    case "aborted":
      return { class: "tool-line-status--error", label: "✗" };
    case "blocked":
      return { class: "tool-line-status--muted", label: "⊘" };
    case "awaiting_approval":
      return { class: "tool-line-status--warning", label: "⏳" };
    default:
      return { class: "tool-line-status--muted", label: "" };
  }
}

export function ToolBubble(props: ToolBubbleProps) {
  const [expanded, setExpanded] = createSignal(false);
  const intentText = () => props.intent?.trim() ?? "";
  const targetText = () => intentText() || toolBubbleTargetText(props.toolName, props.arguments);
  const rowStatusText = () => toolBubbleRowStatusText(props.status);
  const icon = () => statusIcon(props.status);
  const hasOutput = () => Boolean(props.output?.trim());
  const expandable = () => hasOutput() || props.streaming;
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
        <span class={`tool-line-status ${icon().class}`}>{icon().label}</span>
        <span class="tool-line-name">
          <span class="tool-line-name-text">
            {props.toolName}
            <Show when={targetText()}>
              {" "}
              <span class="tool-line-target">{targetText()}</span>
            </Show>
            <Show when={!targetText() && rowStatusText()}>
              {" "}
              {rowStatusText()}
            </Show>
          </span>
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
            {props.output}
          </pre>
        </div>
      </Show>
    </div>
  );
}
