import { test, expect } from "../fixtures.attachments.js";

type ComposerLayout = {
  textareaHeight: number;
  highlightHeight: number;
  maxHeight: number;
  scrollHeight: number;
  highlightScrollHeight: number;
  textareaWidth: number;
  highlightWidth: number;
  textareaScrollTop: number;
  highlightScrollTop: number;
  textareaWhiteSpace: string;
  highlightWhiteSpace: string;
  textareaOverflowWrap: string;
  highlightOverflowWrap: string;
  textareaWordBreak: string;
  highlightWordBreak: string;
  textareaScrollbarGutter: string;
  highlightScrollbarGutter: string;
};

test.describe("composer layout", () => {
  test.beforeEach(async ({ context }) => {
    await context.addInitScript(() => {
      localStorage.setItem("openflow.firstRunOnboardingDismissed", "true");
    });
  });

  test("keeps long text and its caret layer aligned through ten visible rows", async ({
    tauriPage,
  }) => {
    await tauriPage.waitForSelector(".sidebar", 15_000);
    await tauriPage
      .getByRole("button", { name: "Attachment chat", exact: true })
      .click();

    const textarea = tauriPage.locator("textarea.composer-input");
    const longText = Array.from(
      { length: 12 },
      (_, index) => `Wrapped composer line ${index + 1} ${"x".repeat(120)}`,
    ).join("\n");
    await textarea.fill(longText);
    await expect(textarea).toHaveValue(longText);

    const layout = (await tauriPage.evaluate(`
      (() => {
        const textarea = document.querySelector("textarea.composer-input");
        const highlight = document.querySelector(".composer-input-highlight");
        if (!(textarea instanceof HTMLTextAreaElement) || !(highlight instanceof HTMLElement)) {
          throw new Error("Composer input layers not found");
        }

        textarea.scrollTop = textarea.scrollHeight;
        textarea.dispatchEvent(new Event("scroll"));

        const textareaStyle = getComputedStyle(textarea);
        const highlightStyle = getComputedStyle(highlight);
        const lineHeight = Number.parseFloat(textareaStyle.lineHeight);
        const padding =
          Number.parseFloat(textareaStyle.paddingTop) +
          Number.parseFloat(textareaStyle.paddingBottom);

        return {
          textareaHeight: textarea.clientHeight,
          highlightHeight: highlight.clientHeight,
          maxHeight: padding + lineHeight * 10,
          scrollHeight: textarea.scrollHeight,
          highlightScrollHeight: highlight.scrollHeight,
          textareaWidth: textarea.clientWidth,
          highlightWidth: highlight.clientWidth,
          textareaScrollTop: textarea.scrollTop,
          highlightScrollTop: highlight.scrollTop,
          textareaWhiteSpace: textareaStyle.whiteSpace,
          highlightWhiteSpace: highlightStyle.whiteSpace,
          textareaOverflowWrap: textareaStyle.overflowWrap,
          highlightOverflowWrap: highlightStyle.overflowWrap,
          textareaWordBreak: textareaStyle.wordBreak,
          highlightWordBreak: highlightStyle.wordBreak,
          textareaScrollbarGutter: textareaStyle.scrollbarGutter,
          highlightScrollbarGutter: highlightStyle.scrollbarGutter,
        };
      })()
    `)) as ComposerLayout;

    expect(Math.abs(layout.textareaHeight - layout.maxHeight)).toBeLessThanOrEqual(1);
    expect(layout.highlightHeight).toBe(layout.textareaHeight);
    expect(layout.scrollHeight).toBeGreaterThan(layout.textareaHeight);
    expect(
      Math.abs(layout.highlightScrollHeight - layout.scrollHeight),
    ).toBeLessThanOrEqual(1);
    expect(layout.highlightWidth).toBe(layout.textareaWidth);
    expect(layout.highlightScrollTop).toBe(layout.textareaScrollTop);
    expect(layout.textareaScrollTop).toBeGreaterThan(0);
    expect(layout.highlightWhiteSpace).toBe(layout.textareaWhiteSpace);
    expect(layout.highlightOverflowWrap).toBe(layout.textareaOverflowWrap);
    expect(layout.highlightWordBreak).toBe(layout.textareaWordBreak);
    expect(layout.highlightScrollbarGutter).toBe(layout.textareaScrollbarGutter);
  });
});
