import { createEffect, createSignal, onCleanup, type Accessor } from "solid-js";
import type { BottomTab, ChatRole } from "../types";

export const ICON_STROKE_WIDTH = 1.9;
export const STATUS_TOAST_ID = "app-status";
export const BANNER_DISMISS_MS = 4000;
export const DEFAULT_DOCK_HEIGHT = 188;
export const COLLAPSED_DOCK_HEIGHT = 52;
const RESTORED_CHAT_DOCK_HEIGHT_RATIO = 0.75;
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

/** Pretty-print JSON text when parseable; otherwise return input unchanged. */
export function prettyJsonText(text: string): string {
  const trimmed = text.trim();
  if (!trimmed || (trimmed[0] !== "{" && trimmed[0] !== "[")) {
    return text;
  }
  try {
    return JSON.stringify(JSON.parse(trimmed), null, 2);
  } catch {
    return text;
  }
}

/** Indent-only object dump (no `{}` / `[]`); parses JSON when present. */
export function formatIndentedValue(text: string): string {
  const trimmed = text.trim();
  if (!trimmed) {
    return text;
  }
  if (trimmed[0] === "{" || trimmed[0] === "[") {
    try {
      return formatIndentedJson(JSON.parse(trimmed));
    } catch {
      return text;
    }
  }
  return text;
}

function formatIndentedJson(value: unknown, depth = 0): string {
  const indent = "  ".repeat(depth);
  if (value === null) return "null";
  if (typeof value === "boolean" || typeof value === "number") return String(value);
  if (typeof value === "string") return appendIndentedString(value, depth);
  if (Array.isArray(value)) {
    if (value.length === 0) return "";
    return value
      .map((item) => {
        if (isNonEmptyContainer(item)) {
          return `${indent}- \n${formatIndentedJson(item, depth + 1)}`;
        }
        if (typeof item === "string" && item.includes("\n")) {
          return `${indent}- \n${appendIndentedString(item, depth + 1)}`;
        }
        return `${indent}- ${formatIndentedJson(item, 0)}`;
      })
      .join("\n");
  }
  if (typeof value === "object") {
    const entries = Object.entries(value as Record<string, unknown>);
    if (entries.length === 0) return "";
    return entries
      .map(([key, child]) => {
        if (isNonEmptyContainer(child)) {
          return `${indent}${key}:\n${formatIndentedJson(child, depth + 1)}`;
        }
        if (typeof child === "string" && child.includes("\n")) {
          return `${indent}${key}:\n${appendIndentedString(child, depth + 1)}`;
        }
        return `${indent}${key}: ${formatIndentedJson(child, 0)}`;
      })
      .join("\n");
  }
  return String(value);
}

function isNonEmptyContainer(value: unknown): boolean {
  if (Array.isArray(value)) return value.length > 0;
  return typeof value === "object" && value !== null && Object.keys(value).length > 0;
}

function appendIndentedString(text: string, depth: number): string {
  const indent = "  ".repeat(depth);
  const lines = text.split("\n");
  return lines.map((line, index) => (index === 0 ? line : `${indent}${line}`)).join("\n");
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

/** CSS-px viewport height inside a `zoom`-scaled shell (`clientY` / `innerHeight` are visual). */
export function layoutViewportHeight(
  nextViewportHeight = viewportHeight(),
  uiZoom = 1,
): number {
  const zoom = Number.isFinite(uiZoom) && uiZoom > 0 ? uiZoom : 1;
  return nextViewportHeight / zoom;
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
  uiZoom = 1,
): number {
  const min = minimumDockHeight(tab, compact);
  const max = Math.max(
    min,
    Math.round(layoutViewportHeight(nextViewportHeight, uiZoom) - dockViewportMargin(compact)),
  );
  return Math.min(Math.max(Math.round(height), min), max);
}

export function restoredChatDockHeight(
  nextViewportHeight = viewportHeight(),
  compact = isCompactViewportWidth(),
  uiZoom = 1,
): number {
  return clampDockHeight(
    Math.round(layoutViewportHeight(nextViewportHeight, uiZoom) * RESTORED_CHAT_DOCK_HEIGHT_RATIO),
    "chat",
    nextViewportHeight,
    compact,
    uiZoom,
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

export const COMPOSER_INPUT_MAX_ROWS = 4;

export function resizeComposerTextarea(textarea: HTMLTextAreaElement) {
  const style = getComputedStyle(textarea);
  const lineHeight = Number.parseFloat(style.lineHeight);
  if (!Number.isFinite(lineHeight)) {
    return;
  }

  const padding =
    Number.parseFloat(style.paddingTop) + Number.parseFloat(style.paddingBottom);
  const minHeight = Number.parseFloat(style.minHeight) || lineHeight;
  const maxHeight = padding + lineHeight * COMPOSER_INPUT_MAX_ROWS;

  // ponytail: empty scrollHeight includes wrapped placeholder — keep one row until typed
  if (textarea.value.length === 0) {
    textarea.style.height = "";
    textarea.style.overflowY = "hidden";
    return;
  }

  const placeholder = textarea.placeholder;
  textarea.placeholder = "";
  textarea.style.height = "0px";
  const scrollHeight = textarea.scrollHeight;
  textarea.placeholder = placeholder;

  const nextHeight = Math.max(minHeight, Math.min(scrollHeight, maxHeight));
  textarea.style.height = `${nextHeight}px`;
  textarea.style.overflowY = scrollHeight > maxHeight ? "auto" : "hidden";
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
