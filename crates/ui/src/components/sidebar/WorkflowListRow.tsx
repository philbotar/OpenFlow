import Ellipsis from "lucide-solid/icons/ellipsis";
import { createEffect, createSignal, onCleanup, Show, type JSX } from "solid-js";
import { ICON_STROKE_WIDTH } from "../../lib/utils";
import { Tooltip } from "../Tooltip";
import { SidebarListRow } from "./SidebarListRow";

export type WorkflowListRowProps = {
  title: string;
  active: boolean;
  editing: boolean;
  onSelect: () => void;
  onRename: () => void;
  onDelete: () => void;
  editSlot: JSX.Element;
};

export function WorkflowListRow(props: WorkflowListRowProps) {
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
      editing={props.editing}
      onSelect={props.onSelect}
      editSlot={props.editSlot}
      actionSlot={
        <Show when={!props.editing}>
          <div class="project-folder-menu-anchor" ref={menuAnchor}>
            <Tooltip label={`Workflow options for ${props.title}`}>
              <button
                type="button"
                class="sidebar-icon-button workflow-row-action"
                aria-label={`Workflow options for ${props.title}`}
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
                aria-label={`Workflow options for ${props.title}`}
              >
                <button
                  type="button"
                  class="project-folder-menu-item"
                  role="menuitem"
                  onClick={(event) => {
                    event.stopPropagation();
                    closeMenu();
                    props.onRename();
                  }}
                >
                  Rename
                </button>
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
                  Delete workflow
                </button>
              </div>
            </Show>
          </div>
        </Show>
      }
    />
  );
}
