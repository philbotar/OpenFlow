import Bot from "lucide-solid/icons/bot";
import CircleCheck from "lucide-solid/icons/circle-check";
import PencilLine from "lucide-solid/icons/pencil-line";
import Play from "lucide-solid/icons/play";
import Square from "lucide-solid/icons/square";
import Plus from "lucide-solid/icons/plus";
import Save from "lucide-solid/icons/save";
import Settings2 from "lucide-solid/icons/settings-2";
import Trash2 from "lucide-solid/icons/trash-2";
import CircleHelp from "lucide-solid/icons/circle-help";
import { ICON_STROKE_WIDTH } from "../lib/utils";

export type SidebarIconName =
  | "agents"
  | "plus"
  | "edit"
  | "settings"
  | "save"
  | "validate"
  | "run"
  | "stop"
  | "trash"
  | "help";

export function SidebarIcon(props: { name: SidebarIconName }) {
  switch (props.name) {
    case "agents":
      return (
        <Bot
          class="sidebar-icon"
          aria-hidden="true"
          absoluteStrokeWidth
          strokeWidth={ICON_STROKE_WIDTH}
        />
      );
    case "plus":
      return (
        <Plus
          class="sidebar-icon"
          aria-hidden="true"
          absoluteStrokeWidth
          strokeWidth={ICON_STROKE_WIDTH}
        />
      );
    case "edit":
      return (
        <PencilLine
          class="sidebar-icon"
          aria-hidden="true"
          absoluteStrokeWidth
          strokeWidth={ICON_STROKE_WIDTH}
        />
      );
    case "settings":
      return (
        <Settings2
          class="sidebar-icon"
          aria-hidden="true"
          absoluteStrokeWidth
          strokeWidth={ICON_STROKE_WIDTH}
        />
      );
    case "save":
      return (
        <Save
          class="sidebar-icon"
          aria-hidden="true"
          absoluteStrokeWidth
          strokeWidth={ICON_STROKE_WIDTH}
        />
      );
    case "validate":
      return (
        <CircleCheck
          class="sidebar-icon"
          aria-hidden="true"
          absoluteStrokeWidth
          strokeWidth={ICON_STROKE_WIDTH}
        />
      );
    case "run":
      return (
        <Play
          class="sidebar-icon"
          aria-hidden="true"
          absoluteStrokeWidth
          strokeWidth={ICON_STROKE_WIDTH}
        />
      );
    case "stop":
      return (
        <Square
          class="sidebar-icon"
          aria-hidden="true"
          absoluteStrokeWidth
          strokeWidth={ICON_STROKE_WIDTH}
        />
      );
    case "trash":
      return (
        <Trash2
          class="sidebar-icon"
          aria-hidden="true"
          absoluteStrokeWidth
          strokeWidth={ICON_STROKE_WIDTH}
        />
      );
    case "help":
      return (
        <CircleHelp
          class="sidebar-icon"
          aria-hidden="true"
          absoluteStrokeWidth
          strokeWidth={ICON_STROKE_WIDTH}
        />
      );
  }
}
