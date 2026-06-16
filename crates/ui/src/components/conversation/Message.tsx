import { Show, splitProps } from "solid-js";
import type { ComponentProps, JSX } from "solid-js";
import { MarkdownContent } from "./MarkdownContent";

// ── Message ──────────────────────────────────────────────────────────

export type MessageRole = "user" | "assistant" | "system" | "thinking";

interface MessageProps extends ComponentProps<"div"> {
  from: MessageRole;
  label: string;
  content: string;
  streaming?: boolean;
}

export function Message(allProps: MessageProps) {
  // Don't destructure props: it breaks Solid reactivity, forcing a full
  // component recreation (and markdown re-parse) for every content update.
  const [local, rest] = splitProps(allProps, [
    "class",
    "from",
    "label",
    "content",
    "streaming",
  ]);
  const animationClass = () =>
    local.from === "assistant" ? "" : "conversation-item-enter";
  return (
    <div
      class={`chat-row message message-${local.from} role-${local.from} ${animationClass()} ${local.class ?? ""}`}
      {...rest}
    >
      <Show when={local.from !== "assistant"}>
        <div class={`chat-role ${local.from === "system" ? "is-system" : ""}`}>
          {local.label}
        </div>
      </Show>
      <div
        class="message-content"
        classList={{
          "message-streaming-caret": Boolean(local.streaming),
          "message-content--empty": local.content.trim().length === 0,
        }}
      >
        <MarkdownContent content={local.content} />
      </div>
    </div>
  );
}

// ── MessageContent ───────────────────────────────────────────────────

interface MessageContentProps extends ComponentProps<"div"> {
  children?: JSX.Element;
}

export function MessageContent(allProps: MessageContentProps) {
  const [local, rest] = splitProps(allProps, ["class", "children"]);
  return (
    <div class={`message-content ${local.class ?? ""}`} {...rest}>
      {local.children}
    </div>
  );
}
