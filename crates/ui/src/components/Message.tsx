import type { ComponentProps, JSX } from "solid-js";

// ── Message ──────────────────────────────────────────────────────────

type MessageRole = "user" | "assistant" | "system" | "thinking";

interface MessageProps extends ComponentProps<"div"> {
  from: MessageRole;
  label: string;
  children?: JSX.Element;
}

export function Message(allProps: MessageProps) {
  const { class: className, from, label, children, ...rest } = allProps;
  return (
    <div
      class={`chat-row message message-${from} role-${from} ${className ?? ""}`}
      {...rest}
    >
      <div class={`chat-role ${from === "system" ? "is-system" : ""}`}>{label}</div>
      <pre class="message-content">{children}</pre>
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
