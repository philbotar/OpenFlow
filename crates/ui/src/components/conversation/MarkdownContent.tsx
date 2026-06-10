import { SolidMarkdown } from "solid-markdown";
import type { ComponentProps } from "solid-js";

interface MarkdownContentProps extends ComponentProps<"div"> {
  content: string;
}

export function MarkdownContent(props: MarkdownContentProps) {
  const { content, class: className, ...rest } = props;
  return (
    <div class={`markdown-body ${className ?? ""}`} {...rest}>
      <SolidMarkdown renderingStrategy="reconcile">{content}</SolidMarkdown>
    </div>
  );
}
