import ChevronDown from "lucide-solid/icons/chevron-down";
import ChevronRight from "lucide-solid/icons/chevron-right";
import { createMemo, createSignal, Show, splitProps } from "solid-js";
import type { ComponentProps } from "solid-js";
import { displayChatContent } from "../../lib/stripToolCallMarkup";
import type { ChatMessage } from "../../lib/types";
import { MarkdownContent } from "./MarkdownContent";

interface ThinkingBubbleProps extends ComponentProps<"div"> {
  message: ChatMessage;
}

export function ThinkingBubble(allProps: ThinkingBubbleProps) {
  const [local, rest] = splitProps(allProps, ["message", "class"]);
  const [expanded, setExpanded] = createSignal(false);
  const content = createMemo(() =>
    displayChatContent(local.message.role, local.message.content),
  );
  const preview = createMemo(() => {
    const text = content().replace(/\s+/g, " ").trim();
    if (!text) return "";
    const limit = 120;
    return text.length > limit ? `${text.slice(0, limit)}…` : text;
  });

  return (
    <Show when={content().trim()}>
      <div
        class={`thinking-bubble conversation-item-enter ${local.class ?? ""}`}
        data-streaming={local.message.streaming ? "true" : "false"}
        {...rest}
      >
        <button
          type="button"
          class="thinking-bubble-toggle"
          aria-expanded={expanded()}
          onClick={() => setExpanded((value) => !value)}
        >
          <span class="thinking-bubble-icon" aria-hidden="true">
            {expanded() ? <ChevronDown class="thinking-bubble-chevron" /> : <ChevronRight class="thinking-bubble-chevron" />}
          </span>
          <span class="thinking-bubble-label">
            Thinking{local.message.streaming ? "…" : ""}
          </span>
          <Show when={!expanded() && preview()}>
            <span class="thinking-bubble-preview">{preview()}</span>
          </Show>
        </button>
        <Show when={expanded()}>
          <div class="thinking-bubble-body message-content">
            <MarkdownContent content={content()} />
          </div>
        </Show>
      </div>
    </Show>
  );
}
