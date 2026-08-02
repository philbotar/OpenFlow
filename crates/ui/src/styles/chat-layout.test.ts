// @vitest-environment jsdom
import { readFileSync } from "node:fs";
import { afterEach, describe, expect, it } from "vitest";

const indexCss = readFileSync("src/styles/index.css", "utf8");
const chatCss = readFileSync("src/styles/chat.css", "utf8");
const tokensCss = readFileSync("src/styles/tokens.css", "utf8");

describe("chat layout", () => {
  afterEach(() => {
    document.head.replaceChildren();
    document.body.replaceChildren();
  });

  it("keeps workflow message text flush with the composer lane", () => {
    const style = document.createElement("style");
    style.textContent = `${indexCss}\n${chatCss}`;
    document.head.append(style);

    const segment = document.createElement("section");
    segment.className = "chat-segment";
    document.body.append(segment);

    const computed = getComputedStyle(segment);
    expect(computed.marginLeft).toBe("0px");
    expect(computed.marginRight).toBe("0px");
  });

  it("keeps the composer caret layer aligned through ten visible rows", () => {
    const style = document.createElement("style");
    style.textContent = `${tokensCss}\n${indexCss}\n${chatCss}`;
    document.head.append(style);

    const stack = document.createElement("div");
    stack.className = "composer-input-stack";
    const highlight = document.createElement("div");
    highlight.className = "composer-input-highlight";
    const textarea = document.createElement("textarea");
    textarea.className = "text-area composer-input composer-input-mirror";
    stack.append(highlight, textarea);
    document.body.append(stack);

    const highlightStyle = getComputedStyle(highlight);
    const textareaStyle = getComputedStyle(textarea);
    expect(
      getComputedStyle(document.documentElement)
        .getPropertyValue("--composer-input-max-rows")
        .trim(),
    ).toBe("10");
    expect(highlightStyle.whiteSpace).toBe("pre-wrap");
    expect(textareaStyle.whiteSpace).toBe(highlightStyle.whiteSpace);
    expect(highlightStyle.overflowWrap).toBe("break-word");
    expect(textareaStyle.overflowWrap).toBe(highlightStyle.overflowWrap);
    expect(highlightStyle.wordBreak).toBe("normal");
    expect(textareaStyle.wordBreak).toBe(highlightStyle.wordBreak);
    expect(highlightStyle.scrollbarGutter).toBe("stable");
    expect(textareaStyle.scrollbarGutter).toBe(highlightStyle.scrollbarGutter);
  });
});
