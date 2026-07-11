import ChevronRight from "lucide-solid/icons/chevron-right";
import { createEffect, createMemo, createSignal, Show, splitProps } from "solid-js";
import type { ComponentProps } from "solid-js";
import { displayChatContent } from "../../lib/stripToolCallMarkup";
import type { ChatMessage } from "../../lib/types";
import { MarkdownContent } from "./MarkdownContent";

interface ThinkingBubbleProps extends ComponentProps<"div"> {
  message: ChatMessage;
  defaultExpanded?: boolean;
}

export function ThinkingBubble(allProps: ThinkingBubbleProps) {
  const [local, rest] = splitProps(allProps, ["message", "class", "defaultExpanded"]);
  const [expanded, setExpanded] = createSignal(local.defaultExpanded ?? false);
  const content = createMemo(() =>
    displayChatContent(local.message.role, local.message.content),
  );
  const label = () =>
    local.message.streaming ? "Thinking" : "Thought for a while";
  const hasContent = () => content().trim().length > 0;

  createEffect(() => {
    if (local.message.streaming) {
      setExpanded(true);
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
          onClick={() => setExpanded((value) => !value)}
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
