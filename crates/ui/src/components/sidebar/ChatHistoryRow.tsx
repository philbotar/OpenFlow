import Ellipsis from "lucide-solid/icons/ellipsis";
import { createEffect, createSignal, onCleanup, Show } from "solid-js";
import { ICON_STROKE_WIDTH } from "../../lib/utils";
import { Tooltip } from "../Tooltip";
import { SidebarListRow } from "./SidebarListRow";

export type ChatHistoryRowProps = {
  title: string;
  active: boolean;
  onSelect: () => void;
  onDelete: () => void;
};

export function ChatHistoryRow(props: ChatHistoryRowProps) {
  const [menuOpen, setMenuOpen] = createSignal(false);
  let menuAnchor: HTMLDivElement | undefined;

  const closeMenu = () => setMenuOpen(false);

  createEffect(() => {
    if (!menuOpen()) return;

    const handlePointerDown = (event: PointerEvent) => {
      const target = event.target;
      if (!(target instanceof Node) || menuAnchor?.contains(target)) return;
      closeMenu();
    };
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") closeMenu();
    };

    document.addEventListener("pointerdown", handlePointerDown);
    document.addEventListener("keydown", handleKeyDown);
    onCleanup(() => {
      document.removeEventListener("pointerdown", handlePointerDown);
      document.removeEventListener("keydown", handleKeyDown);
    });
  });

  return (
    <SidebarListRow
      title={props.title}
      active={props.active}
      onSelect={props.onSelect}
      actionSlot={
        <div class="project-folder-menu-anchor" ref={menuAnchor}>
          <Tooltip label={`Chat options for ${props.title}`}>
            <button
              type="button"
              class="sidebar-icon-button workflow-row-action"
              aria-label={`Chat options for ${props.title}`}
              aria-haspopup="menu"
              aria-expanded={menuOpen()}
              onClick={(event) => {
                event.stopPropagation();
                setMenuOpen((open) => !open);
              }}
            >
              <Ellipsis
                class="sidebar-icon"
                aria-hidden="true"
                absoluteStrokeWidth
                strokeWidth={ICON_STROKE_WIDTH}
              />
            </button>
          </Tooltip>
          <Show when={menuOpen()}>
            <div
              class="project-folder-menu"
              role="menu"
              aria-label={`Chat options for ${props.title}`}
            >
              <button
                type="button"
                class="project-folder-menu-item"
                role="menuitem"
                onClick={(event) => {
                  event.stopPropagation();
                  closeMenu();
                  props.onDelete();
                }}
              >
                Delete chat
              </button>
            </div>
          </Show>
        </div>
      }
    />
  );
}
