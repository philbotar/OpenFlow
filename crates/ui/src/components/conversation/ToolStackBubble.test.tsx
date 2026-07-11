// @vitest-environment jsdom
import { render } from "solid-js/web";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { ToolStackBubble, resetToolStackExpandStateForTests } from "./ToolStackBubble";

describe("ToolStackBubble", () => {
  let container: HTMLDivElement;
  let dispose: (() => void) | undefined;

  beforeEach(() => {
    container = document.createElement("div");
    document.body.appendChild(container);
  });

  afterEach(() => {
    dispose?.();
    container.remove();
    resetToolStackExpandStateForTests();
  });

  it("starts collapsed with summary text and no nested children visible", () => {
    dispose = render(
      () => (
        <ToolStackBubble summaryText="Read 3 files">
          <div data-testid="nested-tool">nested</div>
        </ToolStackBubble>
      ),
      container,
    );
    expect(container.textContent).toContain("Read 3 files");
    expect(container.querySelector("[data-testid='nested-tool']")).toBeNull();
    expect(
      container.querySelector(".tool-stack[data-expanded='true']"),
    ).toBeNull();
  });

  it("expands to show nested children and collapses again", () => {
    dispose = render(
      () => (
        <ToolStackBubble summaryText="Read 3 files">
          <div data-testid="nested-tool">nested</div>
        </ToolStackBubble>
      ),
      container,
    );
    const toggle = container.querySelector<HTMLElement>(".tool-stack-status-row");
    expect(toggle).not.toBeNull();
    toggle!.click();
    expect(container.querySelector("[data-testid='nested-tool']")).not.toBeNull();
    expect(
      container.querySelector(".tool-stack[data-expanded='true']"),
    ).not.toBeNull();
    toggle!.click();
    expect(container.querySelector("[data-testid='nested-tool']")).toBeNull();
  });

  it("restores expanded state after remount when persistKey matches", () => {
    dispose = render(
      () => (
        <ToolStackBubble summaryText="Read 3 files" persistKey="node-1:call-a">
          <div data-testid="nested-tool">nested</div>
        </ToolStackBubble>
      ),
      container,
    );
    container.querySelector<HTMLElement>(".tool-stack-status-row")!.click();
    expect(container.querySelector("[data-testid='nested-tool']")).not.toBeNull();
    dispose();
    dispose = render(
      () => (
        <ToolStackBubble summaryText="Read 4 files" persistKey="node-1:call-a">
          <div data-testid="nested-tool">nested</div>
        </ToolStackBubble>
      ),
      container,
    );
    expect(container.querySelector("[data-testid='nested-tool']")).not.toBeNull();
    expect(
      container.querySelector(".tool-stack[data-expanded='true']"),
    ).not.toBeNull();
  });
});
