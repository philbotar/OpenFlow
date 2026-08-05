import { describe, expect, it } from "vitest";
import {
  CHAT_APPROVAL_MODE_STORAGE_KEY,
  readStoredApprovalMode,
  writeStoredApprovalMode,
} from "./approvalModeOptions";

function mockStorage() {
  const values = new Map<string, string>();
  return {
    storage: {
      getItem: (key: string) => values.get(key) ?? null,
      setItem: (key: string, value: string) => values.set(key, value),
    },
    values,
  };
}

describe("chat approval mode storage", () => {
  it("round-trips a valid approval mode", () => {
    const { storage } = mockStorage();

    writeStoredApprovalMode(storage, "always_ask");

    expect(storage.getItem(CHAT_APPROVAL_MODE_STORAGE_KEY)).toBe("always_ask");
    expect(readStoredApprovalMode(storage)).toBe("always_ask");
  });

  it("ignores an unknown stored value", () => {
    const { storage, values } = mockStorage();
    values.set(CHAT_APPROVAL_MODE_STORAGE_KEY, "unknown");

    expect(readStoredApprovalMode(storage)).toBeNull();
  });
});
