import { describe, expect, it } from "vitest";
import {
  LEFT_PANEL_VISIBILITY_STORAGE_KEY,
  PANEL_VISIBILITY_STORAGE_KEY,
  PROJECTS_SECTION_STORAGE_KEY,
  WORKFLOWS_SECTION_STORAGE_KEY,
  readProjectsSectionHidden,
  readStoredBoolean,
  readStoredLeftPanelHidden,
  readStoredRightPanelHidden,
  readWorkflowsSectionHidden,
  writeProjectsSectionHidden,
  writeStoredBoolean,
  writeStoredLeftPanelHidden,
  writeStoredRightPanelHidden,
  writeWorkflowsSectionHidden,
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

  it("returns false when storage is undefined", () => {
    expect(readStoredBoolean(undefined, "key")).toBe(false);
  });

  it("returns false when getItem returns null", () => {
    const storage = { getItem: () => null, setItem: () => undefined };
    expect(readStoredBoolean(storage, "key")).toBe(false);
  });

  it("returns true when stored value is 'true'", () => {
    const storage = { getItem: () => "true", setItem: () => undefined };
    expect(readStoredBoolean(storage, "key")).toBe(true);
  });

  it("returns false when stored value is 'false'", () => {
    const storage = { getItem: () => "false", setItem: () => undefined };
    expect(readStoredBoolean(storage, "key")).toBe(false);
  });

  it("returns false for non-boolean stored values", () => {
    const storage = { getItem: () => "not-a-bool", setItem: () => undefined };
    expect(readStoredBoolean(storage, "key")).toBe(false);
  });
});

describe("writeStoredBoolean", () => {
  it("persists boolean as string", () => {
    const { storage, getStored } = mockStorage();
    writeStoredBoolean(storage, "key", true);
    expect(getStored()).toBe("true");
    writeStoredBoolean(storage, "key", false);
    expect(getStored()).toBe("false");
  });
});

describe("panel visibility", () => {
  it("round-trips right panel hidden", () => {
    const { storage } = mockStorage();
    writeStoredRightPanelHidden(storage, true);
    expect(readStoredRightPanelHidden(storage)).toBe(true);
  });

  it("round-trips left panel hidden", () => {
    const { storage } = mockStorage();
    writeStoredLeftPanelHidden(storage, true);
    expect(readStoredLeftPanelHidden(storage)).toBe(true);
  });

  it("uses expected storage keys", () => {
    expect(PANEL_VISIBILITY_STORAGE_KEY).toBe("openflow.rightPanelHidden");
    expect(LEFT_PANEL_VISIBILITY_STORAGE_KEY).toBe("openflow.leftPanelHidden");
  });
});

describe("sidebar section visibility", () => {
  it("round-trips workflows section hidden", () => {
    const { storage } = mockStorage();
    writeWorkflowsSectionHidden(storage, true);
    expect(readWorkflowsSectionHidden(storage)).toBe(true);
  });

  it("round-trips projects section hidden", () => {
    const { storage } = mockStorage();
    writeProjectsSectionHidden(storage, true);
    expect(readProjectsSectionHidden(storage)).toBe(true);
  });

  it("uses expected storage keys", () => {
    expect(WORKFLOWS_SECTION_STORAGE_KEY).toBe("openflow.workflowsSectionHidden");
    expect(PROJECTS_SECTION_STORAGE_KEY).toBe("openflow.projectsSectionHidden");
  });
});
