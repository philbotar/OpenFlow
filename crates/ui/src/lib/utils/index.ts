import { createEffect, createSignal, onCleanup, type Accessor } from "solid-js";
import type { BottomTab, ChatRole } from "../types";

export const ICON_STROKE_WIDTH = 1.9;
export const STATUS_TOAST_ID = "app-status";
export const BANNER_DISMISS_MS = 4000;
export const DEFAULT_DOCK_HEIGHT = 188;
export const COLLAPSED_DOCK_HEIGHT = 52;
const RESTORED_CHAT_DOCK_HEIGHT_RATIO = 0.7;
const DOCK_VIEWPORT_MARGIN = 160;
const COMPACT_VIEWPORT_MAX = 980;
const COMPACT_DOCK_VIEWPORT_MARGIN = 240;

export function isCompactViewportWidth(width = globalThis.innerWidth ?? 1280): boolean {
  return width <= COMPACT_VIEWPORT_MAX;
}

export function dockViewportMargin(compact = isCompactViewportWidth()): number {
  return compact ? COMPACT_DOCK_VIEWPORT_MARGIN : DOCK_VIEWPORT_MARGIN;
}

export function normalizeError(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }
  if (typeof error === "string") {
    return error;
  }
  return JSON.stringify(error);
}

export function toastMessageForDebugMode(message: string, debugOutput: boolean): string {
  if (debugOutput) {
    return message;
  }
  const marker = "Raw AWS SDK error:";
  const markerIndex = message.indexOf(marker);
  if (markerIndex < 0) {
    return message;
  }
  const before = message.slice(0, markerIndex).trimEnd();
  const after = message.slice(markerIndex + marker.length);
  const checkIndex = after.indexOf(". Check ");
  if (checkIndex >= 0) {
    return `${before} ${after.slice(checkIndex + 2).trimStart()}`.trim();
  }
  return before.replace(/[.:;\s]+$/, ".");
}

export function viewportHeight(): number {
  return typeof globalThis.innerHeight === "number" ? globalThis.innerHeight : 900;
}

export function minimumDockHeight(tab: BottomTab, compact = isCompactViewportWidth()): number {
  if (compact) {
    return tab === "chat" ? 104 : 132;
  }
  return tab === "chat" ? 116 : 168;
}

export function clampDockHeight(
  height: number,
  tab: BottomTab,
  nextViewportHeight = viewportHeight(),
  compact = isCompactViewportWidth(),
): number {
  const min = minimumDockHeight(tab, compact);
  const max = Math.max(min, nextViewportHeight - dockViewportMargin(compact));
  return Math.min(Math.max(Math.round(height), min), max);
}

export function restoredChatDockHeight(
  nextViewportHeight = viewportHeight(),
  compact = isCompactViewportWidth(),
): number {
  return clampDockHeight(
    Math.round(nextViewportHeight * RESTORED_CHAT_DOCK_HEIGHT_RATIO),
    "chat",
    nextViewportHeight,
    compact,
  );
}

export function shouldCollapseDock(
  height: number,
  tab: BottomTab,
  compact = isCompactViewportWidth(),
): boolean {
  return height <= Math.max(COLLAPSED_DOCK_HEIGHT + 16, minimumDockHeight(tab, compact) - 32);
}

export function chatRoleLabel(
  role: ChatRole,
  nodeLabel: string | null | undefined,
  options?: { segmentHeaderShowsNode?: boolean },
): string {
  switch (role) {
    case "system":
    case "System":
      return "System";
    case "thinking":
    case "Thinking":
      return "Thinking";
    case "assistant":
    case "Assistant":
      if (options?.segmentHeaderShowsNode) {
        return "Assistant";
      }
      return nodeLabel?.trim() || "Node";
    case "user":
    case "User":
      return "You";
  }
}

export function isTextInputTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) {
    return false;
  }
  return (
    ["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName) ||
    target.isContentEditable
  );
}

export function isMacOS(): boolean {
  return typeof navigator === "object" && /Mac/i.test(navigator.userAgent);
}

export function createDebounced<T>(source: Accessor<T>, delayMs: number): Accessor<T> {
  const [value, setValue] = createSignal(source());
  createEffect(() => {
    const next = source();
    const handle = setTimeout(() => setValue(() => next), delayMs);
    onCleanup(() => clearTimeout(handle));
  });
  return value;
}
