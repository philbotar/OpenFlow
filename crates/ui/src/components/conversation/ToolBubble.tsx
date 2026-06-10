import { createEffect, createSignal, onCleanup, onMount, Show } from "solid-js";
import type { ToolCallStatus } from "../../lib/types";
import { toolBubbleOutputText, toolBubbleRowStatusText } from "./toolBubbleState";

const SCROLL_THRESHOLD = 48;

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

function isTerminal(status: ToolCallStatus): boolean {
  return status === "completed" || status === "failed" || status === "aborted";
}

export function ToolBubble(props: ToolBubbleProps) {
  let outputEl: HTMLDivElement | undefined;
  const [isAtBottom, setIsAtBottom] = createSignal(true);
  const [isExpanded, setIsExpanded] = createSignal(false);
  let resizeObserver: ResizeObserver | undefined;

  const rowStatusText = () => toolBubbleRowStatusText(props.status);

  const outputText = () =>
    toolBubbleOutputText(props.status, props.output, props.arguments, props.isError ?? false);

  const icon = () => statusIcon(props.status);

  const hasOutput = () =>
    isTerminal(props.status) && !!props.output?.trim();

  const scrollOutputToBottom = (smooth: boolean) => {
    if (!outputEl) return;
    outputEl.scrollTo({
      top: outputEl.scrollHeight,
      behavior: smooth ? "smooth" : "instant",
    });
  };

  const onOutputScroll = () => {
    if (!outputEl) return;
    const diff = outputEl.scrollHeight - outputEl.scrollTop - outputEl.clientHeight;
    setIsAtBottom(diff < SCROLL_THRESHOLD);
  };

  onMount(() => {
    if (!outputEl) return;
    resizeObserver = new ResizeObserver(() => {
      if (isAtBottom()) scrollOutputToBottom(false);
    });
    resizeObserver.observe(outputEl);
    scrollOutputToBottom(false);
    onCleanup(() => resizeObserver?.disconnect());
  });

  createEffect(() => {
    outputText();
    if (isAtBottom()) scrollOutputToBottom(false);
  });

  return (
    <div class="tool-line" data-tool-name={props.toolName}>
      {/* Status icon + tool name line */}
      <span class={`tool-line-status ${icon().class}`}>
        {icon().label}
      </span>
      <span class="tool-line-name">
        {props.toolName}
        <Show when={rowStatusText()}> {rowStatusText()}</Show>
      </span>

      {/* Hover-only chevron */}
      <Show when={hasOutput()}>
        <button
          class={`tool-line-chevron ${isExpanded() ? "tool-line-chevron--expanded" : ""}`}
          onClick={() => setIsExpanded((prev) => !prev)}
          aria-label="Toggle output"
        >
          <svg width="12" height="12" viewBox="0 0 12 12" fill="none" xmlns="http://www.w3.org/2000/svg">
            <path d="M4.5 3L7.5 6L4.5 9" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
          </svg>
        </button>
      </Show>

      {/* Expandable output area */}
      <div class={`tool-line-output-wrapper ${isExpanded() && hasOutput() ? "tool-line-output-wrapper--expanded" : ""}`}>
        <div
          ref={outputEl}
          class={`tool-line-output ${props.isError ? "tool-line-output--error" : ""}`}
          onScroll={onOutputScroll}
          role="log"
          aria-live="polite"
        >
          {hasOutput() ? outputText() : ""}
        </div>
      </div>
    </div>
  );
}
