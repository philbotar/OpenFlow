import { splitProps, type ComponentProps } from "solid-js";

export type ButtonVariant = "primary" | "secondary" | "danger";
export type ButtonSize = "default" | "small" | "compact";

export interface ButtonProps extends ComponentProps<"button"> {
  variant?: ButtonVariant;
  size?: ButtonSize;
  ghost?: boolean;
  stretch?: boolean;
}

export function Button(allProps: ButtonProps) {
  const [local, rest] = splitProps(allProps, ["class", "variant", "size", "ghost", "stretch"]);

  const className = () => {
    const variant = local.variant ?? "secondary";
    const parts = [`${variant}-button`];
    if (local.size === "small") parts.push("small");
    if (local.size === "compact") parts.push("compact");
    if (local.ghost) parts.push("ghost");
    if (local.stretch) parts.push("stretch");
    if (local.class) parts.push(local.class);
    return parts.join(" ");
  };

  return <button type="button" class={className()} {...rest} />;
}
