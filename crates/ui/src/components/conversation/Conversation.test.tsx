// @vitest-environment jsdom
import { readFileSync } from "node:fs";
import { render } from "solid-js/web";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { Conversation, ConversationContent } from "./Conversation";

class ResizeObserverStub {
  observe() {}
  disconnect() {}
}

describe("Conversation", () => {
  beforeEach(() => {
    vi.stubGlobal("ResizeObserver", ResizeObserverStub);
    Element.prototype.scrollTo = vi.fn();
  });

  afterEach(() => {
    document.body.replaceChildren();
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  it("marks only the transcript edges with hidden content", () => {
    const container = document.createElement("div");
    document.body.append(container);
    const dispose = render(
      () => (
        <Conversation>
          {(conversation) => (
            <ConversationContent conversation={conversation}>
              <p>Transcript</p>
            </ConversationContent>
          )}
        </Conversation>
      ),
      container,
    );
    const conversation = container.querySelector(".conversation");
    const transcript = container.querySelector(
      ".conversation-content",
    ) as HTMLDivElement;
    let scrollTop = 0;
    Object.defineProperties(transcript, {
      clientHeight: { configurable: true, value: 100 },
      scrollHeight: { configurable: true, value: 500 },
      scrollTop: {
        configurable: true,
        get: () => scrollTop,
        set: (value: number) => {
          scrollTop = value;
        },
      },
    });

    transcript.dispatchEvent(new Event("scroll"));
    expect(conversation?.classList.contains("has-content-above")).toBe(false);
    expect(conversation?.classList.contains("has-content-below")).toBe(true);

    transcript.scrollTop = 200;
    transcript.dispatchEvent(new Event("scroll"));
    expect(conversation?.classList.contains("has-content-above")).toBe(true);
    expect(conversation?.classList.contains("has-content-below")).toBe(true);

    transcript.scrollTop = 400;
    transcript.dispatchEvent(new Event("scroll"));
    expect(conversation?.classList.contains("has-content-above")).toBe(true);
    expect(conversation?.classList.contains("has-content-below")).toBe(false);

    dispose();
  });

  it("softens clipped edges without blocking transcript interaction", () => {
    const chatCss = readFileSync("src/styles/chat.css", "utf8");

    expect(chatCss).toMatch(
      /\.conversation::before,\s*\.conversation::after\s*\{[^}]*pointer-events:\s*none;[^}]*backdrop-filter:\s*blur\(3px\);/s,
    );
    expect(chatCss).toMatch(
      /\.conversation\.has-content-above::before,\s*\.conversation\.has-content-below::after\s*\{[^}]*opacity:\s*1;/s,
    );
  });
});
