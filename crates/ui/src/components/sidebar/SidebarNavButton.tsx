import { SidebarIcon, type SidebarIconName } from "../SidebarIcon";

export type SidebarNavButtonProps = {
  icon: SidebarIconName;
  label: string;
  active?: boolean;
  onClick: () => void;
};

export function SidebarNavButton(props: SidebarNavButtonProps) {
  return (
    <button
      type="button"
      class="sidebar-nav-button"
      classList={{ active: props.active }}
      onClick={() => props.onClick()}
    >
      <SidebarIcon name={props.icon} />
      <span>{props.label}</span>
    </button>
  );
}
