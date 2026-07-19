import type { JSX } from "solid-js";
import { splitProps } from "solid-js";

export interface SectionHeaderProps {
  eyebrow?: string;
  title: string;
  description?: string | JSX.Element;
  actions?: JSX.Element;
  class?: string;
  introClass?: string;
}

export function SectionHeader(allProps: SectionHeaderProps) {
  const [local] = splitProps(allProps, [
    "eyebrow",
    "title",
    "description",
    "actions",
    "class",
    "introClass",
  ]);

  return (
    <header class={`providers-section-header ${local.class ?? ""}`.trim()}>
      <div class={`providers-section-intro ${local.introClass ?? ""}`.trim()}>
        {local.eyebrow ? <div class="eyebrow">{local.eyebrow}</div> : null}
        <h3>{local.title}</h3>
        {local.description ? (
          typeof local.description === "string" ? (
            <p>{local.description}</p>
          ) : (
            local.description
          )
        ) : null}
      </div>
      {local.actions}
    </header>
  );
}
