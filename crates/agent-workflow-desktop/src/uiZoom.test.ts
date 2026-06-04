import { describe, expect, test } from "vitest";
import {
  clampUiZoom,
  DEFAULT_UI_ZOOM,
  formatUiZoomLabel,
  MAX_UI_ZOOM,
  MIN_UI_ZOOM,
  readStoredUiZoom,
  UI_ZOOM_STORAGE_KEY,
  writeStoredUiZoom,
  zoomInUi,
  zoomOutUi,
} from "./uiZoom";

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

  test("formatUiZoomLabel renders rounded percentages", () => {
    expect(formatUiZoomLabel(1)).toBe("100%");
    expect(formatUiZoomLabel(0.9)).toBe("90%");
    expect(formatUiZoomLabel(1.37)).toBe("140%");
  });
});
