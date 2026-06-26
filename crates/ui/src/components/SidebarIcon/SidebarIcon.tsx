import CalendarClock from "lucide-solid/icons/calendar-clock";
import Bot from "lucide-solid/icons/bot";
import PencilLine from "lucide-solid/icons/pencil-line";
import Play from "lucide-solid/icons/play";
import Square from "lucide-solid/icons/square";
import Plus from "lucide-solid/icons/plus";
import Save from "lucide-solid/icons/save";
import Settings2 from "lucide-solid/icons/settings-2";
import Trash2 from "lucide-solid/icons/trash-2";
import PanelRightOpen from "lucide-solid/icons/panel-right-open";
import PanelRightClose from "lucide-solid/icons/panel-right-close";
import PanelLeft from "lucide-solid/icons/panel-left";
import PanelLeftOpen from "lucide-solid/icons/panel-left-open";
import PanelLeftClose from "lucide-solid/icons/panel-left-close";
import CircleHelp from "lucide-solid/icons/circle-help";
import Sparkles from "lucide-solid/icons/sparkles";
import Search from "lucide-solid/icons/search";
import { Match, Switch } from "solid-js";
import { ICON_STROKE_WIDTH } from "../../lib/utils";

export type SidebarIconName =
  | "agents"
  | "schedule"
  | "plus"
  | "sparkles"
  | "edit"
  | "settings"
  | "save"
  | "run"
  | "stop"
  | "trash"
  | "help"
  | "inspector"
  | "panel-right-open"
  | "panel-right-close"
  | "panel-left"
  | "panel-left-open"
  | "panel-left-close";

export function SidebarIcon(props: { name: SidebarIconName }) {
  return (
    <Switch>
      <Match when={props.name === "agents"}>
        <Bot
          class="sidebar-icon"
          aria-hidden="true"
          absoluteStrokeWidth
          strokeWidth={ICON_STROKE_WIDTH}
        />
      </Match>
      <Match when={props.name === "schedule"}>
        <CalendarClock
          class="sidebar-icon"
          aria-hidden="true"
          absoluteStrokeWidth
          strokeWidth={ICON_STROKE_WIDTH}
        />
      </Match>
      <Match when={props.name === "plus"}>
        <Plus
          class="sidebar-icon"
          aria-hidden="true"
          absoluteStrokeWidth
          strokeWidth={ICON_STROKE_WIDTH}
        />
      </Match>
      <Match when={props.name === "sparkles"}>
        <Sparkles
          class="sidebar-icon"
          aria-hidden="true"
          absoluteStrokeWidth
          strokeWidth={ICON_STROKE_WIDTH}
        />
      </Match>
      <Match when={props.name === "edit"}>
        <PencilLine
          class="sidebar-icon"
          aria-hidden="true"
          absoluteStrokeWidth
          strokeWidth={ICON_STROKE_WIDTH}
        />
      </Match>
      <Match when={props.name === "settings"}>
        <Settings2
          class="sidebar-icon"
          aria-hidden="true"
          absoluteStrokeWidth
          strokeWidth={ICON_STROKE_WIDTH}
        />
      </Match>
      <Match when={props.name === "save"}>
        <Save
          class="sidebar-icon"
          aria-hidden="true"
          absoluteStrokeWidth
          strokeWidth={ICON_STROKE_WIDTH}
        />
      </Match>
      <Match when={props.name === "run"}>
        <Play
          class="sidebar-icon"
          aria-hidden="true"
          absoluteStrokeWidth
          strokeWidth={ICON_STROKE_WIDTH}
        />
      </Match>
      <Match when={props.name === "stop"}>
        <Square
          class="sidebar-icon"
          aria-hidden="true"
          absoluteStrokeWidth
          strokeWidth={ICON_STROKE_WIDTH}
        />
      </Match>
      <Match when={props.name === "trash"}>
        <Trash2
          class="sidebar-icon"
          aria-hidden="true"
          absoluteStrokeWidth
          strokeWidth={ICON_STROKE_WIDTH}
        />
      </Match>
      <Match when={props.name === "help"}>
        <CircleHelp
          class="sidebar-icon"
          aria-hidden="true"
          absoluteStrokeWidth
          strokeWidth={ICON_STROKE_WIDTH}
        />
      </Match>
      <Match when={props.name === "inspector"}>
        <Search
          class="sidebar-icon"
          aria-hidden="true"
          absoluteStrokeWidth
          strokeWidth={ICON_STROKE_WIDTH}
        />
      </Match>
      <Match when={props.name === "panel-right-open"}>
        <PanelRightOpen
          class="sidebar-icon"
          aria-hidden="true"
          absoluteStrokeWidth
          strokeWidth={ICON_STROKE_WIDTH}
        />
      </Match>
      <Match when={props.name === "panel-right-close"}>
        <PanelRightClose
          class="sidebar-icon"
          aria-hidden="true"
          absoluteStrokeWidth
          strokeWidth={ICON_STROKE_WIDTH}
        />
      </Match>
      <Match when={props.name === "panel-left"}>
        <PanelLeft
          class="sidebar-icon"
          aria-hidden="true"
          absoluteStrokeWidth
          strokeWidth={ICON_STROKE_WIDTH}
        />
      </Match>
      <Match when={props.name === "panel-left-open"}>
        <PanelLeftOpen
          class="sidebar-icon"
          aria-hidden="true"
          absoluteStrokeWidth
          strokeWidth={ICON_STROKE_WIDTH}
        />
      </Match>
      <Match when={props.name === "panel-left-close"}>
        <PanelLeftClose
          class="sidebar-icon"
          aria-hidden="true"
          absoluteStrokeWidth
          strokeWidth={ICON_STROKE_WIDTH}
        />
      </Match>
    </Switch>
  );
}
