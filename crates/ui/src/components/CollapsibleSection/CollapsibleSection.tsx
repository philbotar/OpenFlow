import type { JSX } from "solid-js";

export function CollapsibleSection(props: { open: boolean; children: JSX.Element; class?: string }) {
  return (
    <div
      class={`collapsible-section ${props.class ?? ""}`}
      classList={{ "collapsible-section--open": props.open }}
    >
      <div class="collapsible-section-inner">{props.children}</div>
    </div>
  );
}
