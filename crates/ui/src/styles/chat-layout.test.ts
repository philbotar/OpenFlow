// @vitest-environment jsdom
import { readFileSync } from "node:fs";
import { afterEach, describe, expect, it } from "vitest";

const indexCss = readFileSync("src/styles/index.css", "utf8");
const chatCss = readFileSync("src/styles/chat.css", "utf8");

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
});
