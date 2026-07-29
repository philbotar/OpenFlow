import { SolidMarkdown } from "solid-markdown";
import type { SolidMarkdownComponents } from "solid-markdown";
import { splitProps } from "solid-js";
import type { ComponentProps } from "solid-js";
import remarkGfm from "remark-gfm";
import { MermaidDiagram } from "./MermaidDiagram";

interface MarkdownContentProps extends ComponentProps<"div"> {
  content: string;
}

type MarkdownPreComponent = Exclude<SolidMarkdownComponents["pre"], string | undefined>;
type MarkdownPreNode = Parameters<MarkdownPreComponent>[0]["node"];

function mermaidSource(node: MarkdownPreNode): string | undefined {
  const code = node.children.length === 1 ? node.children[0] : undefined;
  if (code?.type !== "element" || code.tagName !== "code") return undefined;

  const classNames = code.properties?.className;
  const classes = Array.isArray(classNames) ? classNames : [classNames];
  const isMermaid = classes.some(
    (className) => typeof className === "string" && className.toLowerCase() === "language-mermaid",
  );
  if (!isMermaid) return undefined;

  return code.children
    .filter((child) => child.type === "text")
    .map((child) => child.value)
    .join("")
    .replace(/\n$/, "");
}

const MarkdownPre: MarkdownPreComponent = (props) => {
  const [local, rest] = splitProps(props, [
    "node",
    "children",
    "sourcePosition",
    "index",
    "siblingCount",
  ]);
  const source = () => mermaidSource(local.node);

  return source() === undefined ? (
    <pre {...rest}>{local.children}</pre>
  ) : (
    <MermaidDiagram source={source()!} />
  );
};

const markdownComponents: SolidMarkdownComponents = {
  pre: MarkdownPre,
};

export function MarkdownContent(props: MarkdownContentProps) {
  // Don't destructure props: it breaks Solid reactivity, forcing a full
  // component recreation (and markdown re-parse) for every content update.
  const [local, rest] = splitProps(props, ["content", "class"]);
  return (
    <div class={`markdown-body ${local.class ?? ""}`} {...rest}>
      <SolidMarkdown
        components={markdownComponents}
        renderingStrategy="reconcile"
        remarkPlugins={[remarkGfm]}
      >
        {local.content}
      </SolidMarkdown>
    </div>
  );
}
