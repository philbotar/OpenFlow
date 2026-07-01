import { describe, expect, it } from "vitest";
import {
  LEFT_PANEL_VISIBILITY_STORAGE_KEY,
  PANEL_VISIBILITY_STORAGE_KEY,
  PROJECTS_SECTION_STORAGE_KEY,
  WORKFLOWS_SECTION_STORAGE_KEY,
  readStoredBoolean,
  writeStoredBoolean,
} from ".";

function mockStorage() {
  let stored: string | null = null;
  return {
    storage: {
      getItem: () => stored,
      setItem: (_key: string, value: string) => {
        stored = value;
      },
    },
    getStored: () => stored,
  };
}

describe("readStoredBoolean", () => {
  it("returns false when storage is null", () => {
    expect(readStoredBoolean(null, "key")).toBe(false);
  });

  it("returns true when stored value is 'true'", () => {
    const storage = { getItem: () => "true", setItem: () => undefined };
    expect(readStoredBoolean(storage, "key")).toBe(true);
  });
});

describe("panel visibility keys", () => {
  it("round-trips right panel hidden", () => {
    const { storage } = mockStorage();
    writeStoredBoolean(storage, PANEL_VISIBILITY_STORAGE_KEY, true);
    expect(readStoredBoolean(storage, PANEL_VISIBILITY_STORAGE_KEY)).toBe(true);
  });
});
