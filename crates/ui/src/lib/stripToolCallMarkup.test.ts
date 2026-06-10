import { describe, expect, it } from "vitest";
import { stripToolCallMarkup } from "./stripToolCallMarkup";

describe("stripToolCallMarkup", () => {
  it("removes xml tool_call blocks", () => {
    expect(
      stripToolCallMarkup(
        "<tool_call>\n<function=search>\n<parameter=pattern>TODO</parameter>\n</function>\n</tool_call>",
      ),
    ).toBe("");
  });

  it("keeps leading human text", () => {
    expect(
      stripToolCallMarkup(
        "Checking README.<tool_call><function=read></function></tool_call>",
      ),
    ).toBe("Checking README.");
  });

  it("strips unclosed tool_call tails during streaming", () => {
    expect(
      stripToolCallMarkup("Now searching.<tool_call>\n<function=search>\n"),
    ).toBe("Now searching.");
  });

  it("strips partial tool_call prefixes while streaming", () => {
    expect(stripToolCallMarkup("Planning.<tool_cal")).toBe("Planning.");
    expect(stripToolCallMarkup("<tool")).toBe("");
  });
});
