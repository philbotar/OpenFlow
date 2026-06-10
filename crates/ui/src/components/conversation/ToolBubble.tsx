import { Show } from "solid-js";
import type { ToolCallStatus } from "../../lib/types";
import { toolBubbleRowStatusText, toolBubbleTargetText } from "./toolBubbleState";

export interface ToolBubbleProps {
  toolName: string;
  status: ToolCallStatus;
  output: string | null | undefined;
  arguments?: unknown;
  isError?: boolean;
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
  const targetText = () => toolBubbleTargetText(props.toolName, props.arguments);
  const rowStatusText = () => toolBubbleRowStatusText(props.status);
  const icon = () => statusIcon(props.status);

  return (
    <div class="tool-line" data-tool-name={props.toolName}>
      <span class={`tool-line-status ${icon().class}`}>
        {icon().label}
      </span>
      <span class="tool-line-name">
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
    </div>
  );
}
