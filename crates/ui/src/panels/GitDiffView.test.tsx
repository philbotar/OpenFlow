// @vitest-environment jsdom
import { render } from "solid-js/web";
import { screen } from "@testing-library/dom";
import { describe, it, expect, afterEach } from "vitest";
import { GitDiffView } from "./GitDiffView";
import type { DiffFile } from "@/lib/diff";

const files: DiffFile[] = [
  {
    path: ".github/workflows/release.yml",
    oldPath: ".github/workflows/release.yml",
    additions: 6,
    deletions: 0,
    binary: false,
    isNew: false,
    hunks: [
      {
        header: "@@ -16,8 +16,10 @@",
        precedingUnmodified: 15,
        lines: [
          { kind: "context", oldNo: 16, newNo: 16, text: "        include:" },
          { kind: "add", oldNo: null, newNo: 19, text: "            rust_target: aarch64-apple-darwin" },
        ],
      },
    ],
  },
];

describe("GitDiffView", () => {
  let dispose: (() => void) | undefined;
  let container: HTMLDivElement | undefined;

  afterEach(() => {
    dispose?.();
    container?.remove();
  });

  it("renders the file path, add count, and a context band", () => {
    container = document.createElement("div");
    document.body.append(container);
    dispose = render(() => <GitDiffView files={files} />, container);

    expect(screen.getByText(".github/workflows/release.yml")).toBeTruthy();
    expect(screen.getByText("+6")).toBeTruthy();
    expect(screen.getByText("15 unmodified lines")).toBeTruthy();
    expect(screen.getByText("rust_target: aarch64-apple-darwin", { exact: false })).toBeTruthy();
  });
});
