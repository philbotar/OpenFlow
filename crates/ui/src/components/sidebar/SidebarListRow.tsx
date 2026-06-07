import { Show, type JSX } from "solid-js";
import { SidebarIconButton } from "./SidebarIconButton";

export type SidebarListRowProps = {
  title: string;
  active?: boolean;
  editing?: boolean;
  onSelect: () => void;
  onRename?: () => void;
  editSlot?: JSX.Element;
};

export function SidebarListRow(props: SidebarListRowProps) {
  return (
    <div
      class="workflow-row"
      classList={{ active: props.active, editing: props.editing }}
    >
      <Show
        when={!props.editing}
        fallback={<div class="workflow-row-main">{props.editSlot}</div>}
      >
        <button type="button" class="workflow-row-main" onClick={() => props.onSelect()}>
          <div class="workflow-row-details">
            <span class="workflow-row-title">{props.title}</span>
          </div>
        </button>
      </Show>
      <Show when={props.onRename}>
        <SidebarIconButton
          icon="edit"
          label={`Rename ${props.title}`}
          class="workflow-row-action hover-show"
          onClick={() => props.onRename!()}
        />
      </Show>
    </div>
  );
}
