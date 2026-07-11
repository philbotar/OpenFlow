import { describe, expect, test } from "vitest";
import {
  clampUiZoom,
  DEFAULT_UI_ZOOM,
  MAX_UI_ZOOM,
  MIN_UI_ZOOM,
  readStoredUiZoom,
  TERMINAL_BASE_FONT_SIZE,
  terminalFontSizeForZoom,
  UI_ZOOM_STORAGE_KEY,
  writeStoredUiZoom,
  zoomInUi,
  zoomOutUi,
} from ".";

describe("uiZoom helpers", () => {
  test("clampUiZoom rounds to tenths and enforces bounds", () => {
    expect(clampUiZoom(1.04)).toBe(1);
    expect(clampUiZoom(1.06)).toBe(1.1);
    expect(clampUiZoom(0.1)).toBe(MIN_UI_ZOOM);
    expect(clampUiZoom(2)).toBe(MAX_UI_ZOOM);
  });

  test("zoomInUi and zoomOutUi move by one step without crossing bounds", () => {
    expect(zoomInUi(1)).toBe(1.1);
    expect(zoomOutUi(1)).toBe(0.9);
    expect(zoomInUi(MAX_UI_ZOOM)).toBe(MAX_UI_ZOOM);
    expect(zoomOutUi(MIN_UI_ZOOM)).toBe(MIN_UI_ZOOM);
  });

  test("readStoredUiZoom returns default for missing or invalid values", () => {
    expect(readStoredUiZoom(null)).toBe(DEFAULT_UI_ZOOM);
    expect(readStoredUiZoom({ getItem: () => null, setItem: () => undefined })).toBe(DEFAULT_UI_ZOOM);
    expect(readStoredUiZoom({ getItem: () => "abc", setItem: () => undefined })).toBe(DEFAULT_UI_ZOOM);
  });

  test("readStoredUiZoom clamps stored values and writeStoredUiZoom persists clamped values", () => {
    const storage = new Map<string, string>();
    const storageLike = {
      getItem: (key: string) => storage.get(key) ?? null,
      setItem: (key: string, value: string) => {
        storage.set(key, value);
      },
    };

    storage.set(UI_ZOOM_STORAGE_KEY, "1.37");
    expect(readStoredUiZoom(storageLike)).toBe(1.4);

    writeStoredUiZoom(storageLike, 0.64);
    expect(storage.get(UI_ZOOM_STORAGE_KEY)).toBe(String(MIN_UI_ZOOM));
  });

  test("terminalFontSizeForZoom scales the base font size with zoom, rounded to whole px", () => {
    expect(terminalFontSizeForZoom(1)).toBe(TERMINAL_BASE_FONT_SIZE);
    expect(terminalFontSizeForZoom(1.1)).toBe(Math.round(TERMINAL_BASE_FONT_SIZE * 1.1));
    expect(terminalFontSizeForZoom(MIN_UI_ZOOM)).toBe(Math.round(TERMINAL_BASE_FONT_SIZE * MIN_UI_ZOOM));
    expect(terminalFontSizeForZoom(MAX_UI_ZOOM)).toBe(Math.round(TERMINAL_BASE_FONT_SIZE * MAX_UI_ZOOM));
  });

  test("terminalFontSizeForZoom clamps out-of-range or invalid zoom values", () => {
    expect(terminalFontSizeForZoom(0)).toBe(Math.round(TERMINAL_BASE_FONT_SIZE * MIN_UI_ZOOM));
    expect(terminalFontSizeForZoom(9)).toBe(Math.round(TERMINAL_BASE_FONT_SIZE * MAX_UI_ZOOM));
    expect(terminalFontSizeForZoom(Number.NaN)).toBe(TERMINAL_BASE_FONT_SIZE);
  });
});
