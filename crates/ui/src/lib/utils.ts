import type { BottomTab, ChatRole } from "./types";

export const ICON_STROKE_WIDTH = 1.9;
export const STATUS_TOAST_ID = "app-status";
export const BANNER_DISMISS_MS = 4000;
export const DEFAULT_DOCK_HEIGHT = 188;
export const COLLAPSED_DOCK_HEIGHT = 52;
export const DOCK_VIEWPORT_MARGIN = 160;

export function normalizeError(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }
  if (typeof error === "string") {
    return error;
  }
  return JSON.stringify(error);
}

export function viewportHeight(): number {
  return typeof globalThis.innerHeight === "number" ? globalThis.innerHeight : 900;
}

export function minimumDockHeight(tab: BottomTab): number {
  return tab === "chat" ? 116 : 168;
}

export function clampDockHeight(
  height: number,
  tab: BottomTab,
  nextViewportHeight = viewportHeight(),
): number {
  const min = minimumDockHeight(tab);
  const max = Math.max(min, nextViewportHeight - DOCK_VIEWPORT_MARGIN);
  return Math.min(Math.max(Math.round(height), min), max);
}

export function shouldCollapseDock(height: number, tab: BottomTab): boolean {
  return height <= Math.max(COLLAPSED_DOCK_HEIGHT + 16, minimumDockHeight(tab) - 32);
}

export function chatRoleLabel(
  role: ChatRole,
  nodeLabel: string | null | undefined,
): string {
  switch (role) {
    case "System":
      return "System";
    case "Thinking":
    case "Assistant":
      return nodeLabel?.trim() || "Node";
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
