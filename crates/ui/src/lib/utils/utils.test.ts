import { describe, expect, test } from "vitest";
import { toastMessageForDebugMode } from "./index";

describe("toastMessageForDebugMode", () => {
  test("preserves raw detail when debug output is enabled", () => {
    const message =
      "Could not reach Amazon Bedrock. Raw AWS SDK error: dispatch failure: connector error. Check AWS region.";

    expect(toastMessageForDebugMode(message, true)).toBe(message);
  });

  test("removes raw detail when debug output is disabled", () => {
    const message =
      "Could not reach Amazon Bedrock. Raw AWS SDK error: dispatch failure: connector error. Check AWS region.";

    expect(toastMessageForDebugMode(message, false)).toBe(
      "Could not reach Amazon Bedrock. Check AWS region.",
    );
  });
});
