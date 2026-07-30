// @vitest-environment jsdom
import { render } from "solid-js/web";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";
import { MarkdownContent } from "../conversation/MarkdownContent";
import { ExternalLinkGuard } from "./ExternalLinkGuard";

const apiMocks = vi.hoisted(() => ({
  openExternalUrl: vi.fn().mockResolvedValue(undefined),
}));

vi.mock("../../api", () => ({
  openExternalUrl: apiMocks.openExternalUrl,
}));

describe("ExternalLinkGuard", () => {
  let container: HTMLDivElement;
  let dispose: () => void;

  beforeEach(() => {
    container = document.createElement("div");
    document.body.append(container);
  });

  afterEach(() => {
    dispose?.();
    container.remove();
    vi.clearAllMocks();
  });

  test("opens web links outside the OpenFlow WebView", () => {
    dispose = render(
      () => (
        <ExternalLinkGuard>
          <MarkdownContent
            content="[Read the docs](https://example.com/docs)"
          />
        </ExternalLinkGuard>
      ),
      container,
    );

    const link = container.querySelector("a")!;
    const click = new MouseEvent("click", { bubbles: true, cancelable: true });
    link.dispatchEvent(click);

    expect(click.defaultPrevented).toBe(true);
    expect(apiMocks.openExternalUrl).toHaveBeenCalledWith(
      "https://example.com/docs",
    );
  });
});
