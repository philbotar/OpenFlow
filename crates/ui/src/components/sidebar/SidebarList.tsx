import type { JSX } from "solid-js";

export type SidebarListProps = {
  children: JSX.Element;
};

export function SidebarList(props: SidebarListProps) {
  return <div class="sidebar-list">{props.children}</div>;
}
