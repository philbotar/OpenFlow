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
        class={`tool-line tool-line--thinking tool-line--expandable conversation-item-enter ${local.class ?? ""}`}
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
          <span class="tool-line-status tool-line-status--muted" aria-hidden="true">
            {local.message.streaming ? "•" : ""}
          </span>
          <span class="tool-line-name">
            <span class="tool-line-name-text">
              thinking{local.message.streaming ? "…" : ""}
              <Show when={preview()}>
                {" "}
                <span class="tool-line-target">{preview()}</span>
              </Show>
            </span>
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
              <MarkdownContent content={content()} />
            </div>
          </div>
        </Show>
      </div>
    </Show>
  );
}
