import { splitProps } from "solid-js";
import type { ComponentProps } from "solid-js";

interface SpinnerProps extends ComponentProps<"span"> {
  size?: "sm" | "md";
  label?: string;
}

export function Spinner(allProps: SpinnerProps) {
  const [local, rest] = splitProps(allProps, ["class", "size", "label"]);
  const sizeClass = () => (local.size === "sm" ? "spinner--sm" : "spinner--md");
  return (
    <span
      class={`spinner ${sizeClass()} ${local.class ?? ""}`}
      role="status"
      aria-label={local.label ?? "Loading"}
      {...rest}
    />
  );
}
