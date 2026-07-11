import { describe, expect, test } from "vitest";
import { relativizeDisplayPath } from "./index";

const ROOT = "/Users/philipbotar/Developer/DailyPlanner";

describe("relativizeDisplayPath", () => {
  test("strips cwd prefix", () => {
    expect(relativizeDisplayPath(`${ROOT}/package.json`, ROOT)).toBe("package.json");
  });

  test("root equals dot", () => {
    expect(relativizeDisplayPath(ROOT, ROOT)).toBe(".");
    expect(relativizeDisplayPath(`${ROOT}/`, ROOT)).toBe(".");
  });

  test("leaves relative, outside, url, artifact", () => {
    expect(relativizeDisplayPath("src/a.ts", ROOT)).toBe("src/a.ts");
    expect(relativizeDisplayPath("/tmp/x", ROOT)).toBe("/tmp/x");
    expect(relativizeDisplayPath("https://x.com/a", ROOT)).toBe("https://x.com/a");
    expect(relativizeDisplayPath("artifact:abc", ROOT)).toBe("artifact:abc");
  });

  test("no cwd is noop", () => {
    expect(relativizeDisplayPath(`${ROOT}/x`, null)).toBe(`${ROOT}/x`);
    expect(relativizeDisplayPath(`${ROOT}/x`, "")).toBe(`${ROOT}/x`);
  });
});
