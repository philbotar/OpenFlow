import { readFileSync } from "node:fs";
import { describe, expect, test } from "vitest";

const indexCss = readFileSync("src/styles/index.css", "utf8");

describe("sidebar collection actions", () => {
  test("keeps add controls visible without hover", () => {
    const rule = indexCss.match(/\.sidebar-section-action\s*\{([^}]*)\}/)?.[1] ?? "";

    expect(rule).toContain("opacity: 1");
    expect(rule).toContain("visibility: visible");
  });
});
