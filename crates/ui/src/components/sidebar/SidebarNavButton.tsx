import { SidebarIcon, type SidebarIconName } from "../SidebarIcon";

export type SidebarNavButtonProps = {
  icon: SidebarIconName;
  label: string;
  active?: boolean;
  ariaHasPopup?: "menu";
  ariaExpanded?: boolean;
  /** Trailing update action shown when an app update is available. */
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
      aria-haspopup={props.ariaHasPopup}
      aria-expanded={props.ariaExpanded}
      aria-label={
        props.updateAvailable ? `${props.label} (update available)` : props.label
      }
    >
      <span class="sidebar-nav-button-icon-wrap">
        <SidebarIcon name={props.icon} />
      </span>
      <span>{props.label}</span>
      {props.updateAvailable ? (
        <span class="sidebar-nav-update-action" aria-hidden="true">
          Update
        </span>
      ) : null}
    </button>
  );
}
