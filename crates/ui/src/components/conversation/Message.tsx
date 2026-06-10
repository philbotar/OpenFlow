import type { ComponentProps, JSX } from "solid-js";
import { MarkdownContent } from "./MarkdownContent";

// ── Message ──────────────────────────────────────────────────────────

export type MessageRole = "user" | "assistant" | "system" | "thinking";

interface MessageProps extends ComponentProps<"div"> {
  from: MessageRole;
  label: string;
  content: string;
}

export function Message(allProps: MessageProps) {
  const { class: className, from, label, content, ...rest } = allProps;
  return (
    <div
      class={`chat-row message message-${from} role-${from} ${className ?? ""}`}
      {...rest}
    >
      <div class={`chat-role ${from === "system" ? "is-system" : ""}`}>{label}</div>
      <div class="message-content">
        <MarkdownContent content={content} />
      </div>
    </div>
  );
}

// ── MessageContent ───────────────────────────────────────────────────

interface MessageContentProps extends ComponentProps<"div"> {
  children?: JSX.Element;
}

export function MessageContent(allProps: MessageContentProps) {
  const { class: className, children, ...rest } = allProps;
  return (
    <div class={`message-content ${className ?? ""}`} {...rest}>
      {children}
    </div>
  );
}
