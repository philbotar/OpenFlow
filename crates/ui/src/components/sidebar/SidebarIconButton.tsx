import { SidebarIcon, type SidebarIconName } from "../SidebarIcon";

export type SidebarIconButtonProps = {
  icon: SidebarIconName;
  label: string;
  class?: string;
  onClick: () => void;
};

export function SidebarIconButton(props: SidebarIconButtonProps) {
  return (
    <button
      type="button"
      class={props.class ? `sidebar-icon-button ${props.class}` : "sidebar-icon-button"}
      title={props.label}
      aria-label={props.label}
      onClick={() => props.onClick()}
    >
      <SidebarIcon name={props.icon} />
    </button>
  );
}
