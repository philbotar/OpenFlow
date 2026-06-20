import { describe, expect, it } from "vitest";
import {
  WORKFLOWS_SECTION_STORAGE_KEY,
  readWorkflowsSectionHidden,
  writeWorkflowsSectionHidden,
} from ".";

describe("readWorkflowsSectionHidden", () => {
  it("returns false when storage is null", () => {
    expect(readWorkflowsSectionHidden(null)).toBe(false);
  });

  it("returns false when storage is undefined", () => {
    expect(readWorkflowsSectionHidden(undefined)).toBe(false);
  });

  it("returns false when getItem returns null (no stored value)", () => {
    const storage = { getItem: () => null, setItem: () => undefined };
    expect(readWorkflowsSectionHidden(storage)).toBe(false);
  });

  it("returns true when stored value is 'true'", () => {
    const storage = {
      getItem: () => "true",
      setItem: () => undefined,
    };
    expect(readWorkflowsSectionHidden(storage)).toBe(true);
  });

  it("returns false when stored value is 'false'", () => {
    const storage = {
      getItem: () => "false",
      setItem: () => undefined,
    };
    expect(readWorkflowsSectionHidden(storage)).toBe(false);
  });

  it("returns false for non-boolean stored values", () => {
    const storage = {
      getItem: () => "not-a-bool",
      setItem: () => undefined,
    };
    expect(readWorkflowsSectionHidden(storage)).toBe(false);
  });
});

describe("writeWorkflowsSectionHidden", () => {
  it("persists 'true' string when hidden is true", () => {
    let stored: string | undefined;
    const storage = {
      getItem: () => stored ?? null,
      setItem: (_key: string, value: string) => {
        stored = value;
      },
    };
    writeWorkflowsSectionHidden(storage, true);
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
    writeWorkflowsSectionHidden(storage, false);
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
    writeWorkflowsSectionHidden(storage, true);
    expect(readWorkflowsSectionHidden(storage)).toBe(true);
  });

  it("write false then read returns false", () => {
    let stored: string | null = null;
    const storage = {
      getItem: () => stored,
      setItem: (_key: string, value: string) => {
        stored = value;
      },
    };
    writeWorkflowsSectionHidden(storage, false);
    expect(readWorkflowsSectionHidden(storage)).toBe(false);
  });
});

describe("WORKFLOWS_SECTION_STORAGE_KEY", () => {
  it("uses openflow.workflowsSectionHidden key", () => {
    expect(WORKFLOWS_SECTION_STORAGE_KEY).toBe(
      "openflow.workflowsSectionHidden",
    );
  });
});
