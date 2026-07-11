export const DEFAULT_UI_ZOOM = 1;
export const MIN_UI_ZOOM = 0.7;
export const MAX_UI_ZOOM = 1.4;
const UI_ZOOM_STEP = 0.1;
export const UI_ZOOM_STORAGE_KEY = "openflow.uiZoom";

type StorageLike = Pick<Storage, "getItem" | "setItem"> | null | undefined;

export function clampUiZoom(value: number) {
  const rounded = Math.round(value * 10) / 10;
  return Math.min(MAX_UI_ZOOM, Math.max(MIN_UI_ZOOM, rounded));
}

export const TERMINAL_BASE_FONT_SIZE = 12;

/**
 * xterm.js hit-testing breaks under CSS `zoom`, so the terminal subtree is
 * excluded from UI zoom and its font size is scaled instead.
 */
export function terminalFontSizeForZoom(zoom: number) {
  const normalized = Number.isFinite(zoom) ? clampUiZoom(zoom) : DEFAULT_UI_ZOOM;
  return Math.round(TERMINAL_BASE_FONT_SIZE * normalized);
}

export function zoomInUi(currentZoom: number) {
  return clampUiZoom(currentZoom + UI_ZOOM_STEP);
}

export function zoomOutUi(currentZoom: number) {
  return clampUiZoom(currentZoom - UI_ZOOM_STEP);
}

export function readStoredUiZoom(storage: StorageLike) {
  const rawValue = storage?.getItem(UI_ZOOM_STORAGE_KEY);
  if (rawValue === null || rawValue === undefined) {
    return DEFAULT_UI_ZOOM;
  }

  const parsed = Number(rawValue);
  if (!Number.isFinite(parsed)) {
    return DEFAULT_UI_ZOOM;
  }

  return clampUiZoom(parsed);
}

export function writeStoredUiZoom(storage: StorageLike, currentZoom: number) {
  storage?.setItem(UI_ZOOM_STORAGE_KEY, String(clampUiZoom(currentZoom)));
}
