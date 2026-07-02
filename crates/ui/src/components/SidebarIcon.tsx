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
import GitBranch from "lucide-solid/icons/git-branch";
import { ICON_STROKE_WIDTH } from "../lib/utils";

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
  | "git-branch"
  | "panel-right-open"
  | "panel-right-close"
  | "panel-left"
  | "panel-left-open"
  | "panel-left-close";

const SIDEBAR_ICONS = {
  agents: Bot,
  schedule: CalendarClock,
  plus: Plus,
  sparkles: Sparkles,
  edit: PencilLine,
  settings: Settings2,
  save: Save,
  run: Play,
  stop: Square,
  trash: Trash2,
  help: CircleHelp,
  inspector: Search,
  "git-branch": GitBranch,
  "panel-right-open": PanelRightOpen,
  "panel-right-close": PanelRightClose,
  "panel-left": PanelLeft,
  "panel-left-open": PanelLeftOpen,
  "panel-left-close": PanelLeftClose,
} satisfies Record<SidebarIconName, typeof Bot>;

export function SidebarIcon(props: { name: SidebarIconName }) {
  const Icon = SIDEBAR_ICONS[props.name];
  return (
    <Icon
      class="sidebar-icon"
      aria-hidden="true"
      absoluteStrokeWidth
      strokeWidth={ICON_STROKE_WIDTH}
    />
  );
}
