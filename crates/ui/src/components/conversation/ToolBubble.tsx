import { createEffect, createSignal, onCleanup, onMount, Show } from "solid-js";
import type { ToolCallStatus } from "../../lib/types";
import { toolBubbleOutputText } from "./toolBubbleState";

const SCROLL_THRESHOLD = 48;

export interface ToolBubbleProps {
  toolName: string;
  status: ToolCallStatus;
  output: string | null | undefined;
  arguments?: unknown;
  isError?: boolean;
}

export function ToolBubble(props: ToolBubbleProps) {
  let outputEl: HTMLDivElement | undefined;
  const [isAtBottom, setIsAtBottom] = createSignal(true);
  let resizeObserver: ResizeObserver | undefined;

  const bodyText = () =>
    toolBubbleOutputText(props.status, props.output, props.arguments, props.isError ?? false);

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
    bodyText();
    if (isAtBottom()) scrollOutputToBottom(false);
  });

  return (
    <div
      class={`tool-bubble-row ${props.isError ? "tool-bubble-row--error" : ""}`}
      data-tool-name={props.toolName}
    >
      <div class={`tool-bubble ${props.isError ? "tool-bubble--error" : ""}`}>
        <div class="tool-bubble-header">Tool Invocation: {props.toolName}</div>
        <div
          ref={outputEl}
          class="tool-bubble-output"
          onScroll={onOutputScroll}
          role="log"
          aria-live="polite"
        >
          <Show
            when={bodyText()}
            fallback={
              <Show when={props.status === "running"}>
                <span class="tool-bubble-placeholder">Running…</span>
              </Show>
            }
          >
            {bodyText()}
          </Show>
        </div>
      </div>
    </div>
  );
}
