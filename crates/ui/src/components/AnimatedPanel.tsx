import { onMount } from "solid-js";
import { Dynamic } from "solid-js/web";
import type { JSX, ParentProps } from "solid-js";
import { animateIn, panelEnterKeyframes } from "../lib/motion";

interface AnimatedPanelProps extends ParentProps {
  class?: string;
  tag?: "aside" | "section" | "div";
}

export function AnimatedPanel(props: AnimatedPanelProps) {
  let ref: HTMLElement | undefined;

  onMount(() => {
    if (ref) {
      animateIn(ref, panelEnterKeyframes);
    }
  });

  return (
    <Dynamic component={props.tag ?? "aside"} ref={ref} class={props.class}>
      {props.children as JSX.Element}
    </Dynamic>
  );
}
