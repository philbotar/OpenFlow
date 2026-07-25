import { isMacOS } from "../utils";

export type ShortcutId =
  | "save"
  | "run"
  | "stop"
  | "toggleLeftSidebar"
  | "toggleRightPanel"
  | "zoomIn"
  | "zoomOut"
  | "zoomReset"
  | "toggleInspector"
  | "toggleChatFocus";

type Chord = { key: string; shift?: boolean };

const CHORDS: Record<ShortcutId, Chord> = {
  save: { key: "s" },
  run: { key: "Enter" },
  stop: { key: "." },
  toggleLeftSidebar: { key: "b" },
  toggleRightPanel: { key: "j" },
  zoomIn: { key: "=" },
  zoomOut: { key: "-" },
  zoomReset: { key: "0" },
  toggleInspector: { key: "i" },
  toggleChatFocus: { key: "f", shift: true },
};

const DISPLAY_KEY: Record<string, string> = {
  Enter: "↵",
  "=": "+",
  "-": "−",
};

export function formatShortcutParts(id: ShortcutId, mac = isMacOS()): string[] {
  const chord = CHORDS[id];
  const parts = [mac ? "⌘" : "Ctrl"];
  if (chord.shift) parts.push(mac ? "⇧" : "Shift");
  const key = chord.key.length === 1 ? chord.key.toUpperCase() : (DISPLAY_KEY[chord.key] ?? chord.key);
  parts.push(key);
  return parts;
}

export function eventMatchesShortcut(event: KeyboardEvent, id: ShortcutId): boolean {
  const chord = CHORDS[id];
  const mod = event.metaKey || event.ctrlKey;
  if (!mod) return false;
  if (Boolean(chord.shift) !== event.shiftKey) return false;
  const key = event.key;
  if (chord.key === "Enter") return key === "Enter";
  if (chord.key === "=") return key === "=" || key === "+";
  if (chord.key === "-") return key === "-" || key === "_";
  if (chord.key === ".") return key === ".";
  return key.toLowerCase() === chord.key.toLowerCase();
}
