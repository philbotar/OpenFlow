import { SidebarIcon, type SidebarIconName } from "../SidebarIcon";

export type SidebarNavButtonProps = {
  icon: SidebarIconName;
  label: string;
  active?: boolean;
  /** Blue dot when an app update is available. */
  updateAvailable?: boolean;
  onClick: () => void;
};

export function SidebarNavButton(props: SidebarNavButtonProps) {
  return (
    <button
      type="button"
      class="sidebar-nav-button"
      classList={{ active: props.active }}
      onClick={() => props.onClick()}
      aria-label={
        props.updateAvailable ? `${props.label} (update available)` : props.label
      }
    >
      <span class="sidebar-nav-button-icon-wrap">
        <SidebarIcon name={props.icon} />
        {props.updateAvailable ? (
          <span class="sidebar-nav-update-badge" aria-hidden="true" />
        ) : null}
      </span>
      <span>{props.label}</span>
    </button>
  );
}
