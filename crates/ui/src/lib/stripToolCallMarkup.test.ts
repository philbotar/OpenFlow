import { describe, expect, it } from "vitest";
import { displayChatContent, stripToolCallMarkup } from "./stripToolCallMarkup";

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

describe("displayChatContent", () => {
  const toolCallXml =
    "<tool_call>\n<function=search>\n</function>\n</tool_call>";

  it("preserves tool_call markup in user messages", () => {
    expect(displayChatContent("user", toolCallXml)).toBe(toolCallXml);
  });

  it("strips tool_call markup from assistant messages", () => {
    expect(displayChatContent("assistant", toolCallXml)).toBe("");
  });
});
