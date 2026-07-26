import { SidebarIcon, type SidebarIconName } from "../SidebarIcon";
import { Tooltip } from "../Tooltip";

export type SidebarIconButtonProps = {
  icon: SidebarIconName;
  label: string;
  class?: string;
  active?: boolean;
  onClick: () => void;
};

export function SidebarIconButton(props: SidebarIconButtonProps) {
  return (
    <Tooltip label={props.label}>
      <button
        type="button"
        class={props.class ? `sidebar-icon-button ${props.class}` : "sidebar-icon-button"}
        classList={{ active: props.active }}
        aria-label={props.label}
        onClick={() => props.onClick()}
      >
        <SidebarIcon name={props.icon} />
      </button>
    </Tooltip>
  );
}
