import { describe, expect, it } from "vitest";
import {
  PANEL_VISIBILITY_STORAGE_KEY,
  readStoredRightPanelHidden,
  writeStoredRightPanelHidden,
} from "./panelVisibility";

describe("readStoredRightPanelHidden", () => {
  it("returns false when storage is null", () => {
    expect(readStoredRightPanelHidden(null)).toBe(false);
  });

  it("returns false when storage is undefined", () => {
    expect(readStoredRightPanelHidden(undefined)).toBe(false);
  });

  it("returns false when getItem returns null (no stored value)", () => {
    const storage = { getItem: () => null, setItem: () => undefined };
    expect(readStoredRightPanelHidden(storage)).toBe(false);
  });

  it("returns false for non-boolean stored values", () => {
    const storage = {
      getItem: () => "not-a-bool",
      setItem: () => undefined,
    };
    expect(readStoredRightPanelHidden(storage)).toBe(false);
  });

  it("returns true when stored value is 'true'", () => {
    const storage = {
      getItem: () => "true",
      setItem: () => undefined,
    };
    expect(readStoredRightPanelHidden(storage)).toBe(true);
  });

  it("returns false when stored value is 'false'", () => {
    const storage = {
      getItem: () => "false",
      setItem: () => undefined,
    };
    expect(readStoredRightPanelHidden(storage)).toBe(false);
  });
});

describe("writeStoredRightPanelHidden", () => {
  it("persists 'true' string when hidden is true", () => {
    let stored: string | undefined;
    const storage = {
      getItem: () => stored ?? null,
      setItem: (_key: string, value: string) => {
        stored = value;
      },
    };
    writeStoredRightPanelHidden(storage, true);
    expect(stored).toBe("true");
  });

  it("persists 'false' string when hidden is false", () => {
    let stored: string | undefined;
    const storage = {
      getItem: () => stored ?? null,
      setItem: (_key: string, value: string) => {
        stored = value;
      },
    };
    writeStoredRightPanelHidden(storage, false);
    expect(stored).toBe("false");
  });
});

describe("round-trip persistence", () => {
  it("write true then read returns true", () => {
    let stored: string | null = null;
    const storage = {
      getItem: () => stored,
      setItem: (_key: string, value: string) => {
        stored = value;
      },
    };
    writeStoredRightPanelHidden(storage, true);
    expect(readStoredRightPanelHidden(storage)).toBe(true);
  });

  it("write false then read returns false", () => {
    let stored: string | null = null;
    const storage = {
      getItem: () => stored,
      setItem: (_key: string, value: string) => {
        stored = value;
      },
    };
    writeStoredRightPanelHidden(storage, false);
    expect(readStoredRightPanelHidden(storage)).toBe(false);
  });
});

describe("PANEL_VISIBILITY_STORAGE_KEY", () => {
  it("uses openflow.rightPanelHidden key", () => {
    expect(PANEL_VISIBILITY_STORAGE_KEY).toBe("openflow.rightPanelHidden");
  });
});
