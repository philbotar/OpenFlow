import { splitProps, type ComponentProps, type JSX } from "solid-js";

export interface ButtonRowProps extends ComponentProps<"div"> {
  align?: "start" | "end";
  children: JSX.Element;
}

export function ButtonRow(allProps: ButtonRowProps) {
  const [local, rest] = splitProps(allProps, ["class", "align", "children"]);

  const className = () => {
    const parts = ["button-row"];
    if (local.align === "end") parts.push("end");
    if (local.class) parts.push(local.class);
    return parts.join(" ");
  };

  return (
    <div class={className()} {...rest}>
      {local.children}
    </div>
  );
}
