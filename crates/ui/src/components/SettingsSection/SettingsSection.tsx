import { splitProps, type ComponentProps, type JSX } from "solid-js";

export interface SettingsSectionProps extends ComponentProps<"div"> {
  sectionClass?: string;
  children: JSX.Element;
}

export function SettingsSection(allProps: SettingsSectionProps) {
  const [local, rest] = splitProps(allProps, ["class", "sectionClass", "children"]);

  const className = () =>
    ["settings-section", local.sectionClass, local.class].filter(Boolean).join(" ");

  return (
    <div class={className()} {...rest}>
      {local.children}
    </div>
  );
}
