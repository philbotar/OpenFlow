import ChevronRight from "lucide-solid/icons/chevron-right";
import { createEffect, createMemo, createSignal, Show, splitProps } from "solid-js";
import type { ComponentProps } from "solid-js";
import { displayChatContent } from "../../lib/stripToolCallMarkup";
import type { ChatMessage } from "../../lib/types";
import { MarkdownContent } from "./MarkdownContent";
import { completedDurationMs, formatDuration } from "./timing";

interface ThinkingBubbleProps extends ComponentProps<"div"> {
  message: ChatMessage;
  defaultExpanded?: boolean;
}

/** Survive the streamed message being replaced by its completed projection. */
const expandedByMessageId = new Map<string, boolean>();

/** Test-only: clear persisted expand state between cases. */
export function resetThinkingBubbleExpandStateForTests(): void {
  expandedByMessageId.clear();
}

export function ThinkingBubble(allProps: ThinkingBubbleProps) {
  const [local, rest] = splitProps(allProps, ["message", "class", "defaultExpanded"]);
  const initialExpanded = () => {
    if (local.defaultExpanded !== undefined) return local.defaultExpanded;
    return local.message.id
      ? (expandedByMessageId.get(local.message.id) ?? false)
      : false;
  };
  const [expanded, setExpanded] = createSignal(initialExpanded());
  const content = createMemo(() =>
    displayChatContent(local.message.role, local.message.content),
  );
  const duration = () =>
    formatDuration(
      completedDurationMs(
        local.message.createdAtMs,
        local.message.completedAtMs,
      ),
    );
  const label = () => {
    if (local.message.streaming) return "Thinking";
    return duration() ? `Thought for ${duration()}` : "Thought for a while";
  };
  const hasContent = () => content().trim().length > 0;

  const setExpandedPersist = (
    next: boolean | ((value: boolean) => boolean),
  ) => {
    setExpanded((current) => {
      const value = typeof next === "function" ? next(current) : next;
      if (local.message.id) {
        if (value) expandedByMessageId.set(local.message.id, true);
        else expandedByMessageId.delete(local.message.id);
      }
      return value;
    });
  };

  createEffect(() => {
    if (local.message.streaming) {
      setExpandedPersist(true);
    }
  });

  return (
    <Show when={hasContent() || local.message.streaming}>
      <div
        class={`tool-line tool-line--thinking tool-line--expandable ${local.class ?? ""}`}
        data-streaming={local.message.streaming ? "true" : "false"}
        data-tool-name="thinking"
        {...rest}
      >
        <button
          type="button"
          class="tool-line-status-row"
          aria-expanded={expanded()}
          onClick={() => setExpandedPersist((value) => !value)}
        >
          <span class="tool-line-name">
            <span class="tool-line-name-text">{label()}</span>
            <span
              class="tool-line-chevron"
              classList={{ "tool-line-chevron--expanded": expanded() }}
              aria-hidden="true"
            >
              <ChevronRight width={14} height={14} />
            </span>
          </span>
        </button>
        <Show when={expanded()}>
          <div class="tool-line-output-wrapper tool-line-output-wrapper--expanded">
            <div class="tool-line-output message-content">
              <Show when={hasContent()}>
                <MarkdownContent content={content()} />
              </Show>
            </div>
          </div>
        </Show>
      </div>
    </Show>
  );
}
