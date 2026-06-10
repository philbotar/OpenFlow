import { SolidMarkdown } from "solid-markdown";
import { splitProps } from "solid-js";
import type { ComponentProps } from "solid-js";

interface MarkdownContentProps extends ComponentProps<"div"> {
  content: string;
}

export function MarkdownContent(props: MarkdownContentProps) {
  // Don't destructure props: it breaks Solid reactivity, forcing a full
  // component recreation (and markdown re-parse) for every content update.
  const [local, rest] = splitProps(props, ["content", "class"]);
  return (
    <div class={`markdown-body ${local.class ?? ""}`} {...rest}>
      <SolidMarkdown renderingStrategy="reconcile">{local.content}</SolidMarkdown>
    </div>
  );
}
