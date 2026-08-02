// @vitest-environment jsdom
import { render } from "solid-js/web";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { AppContext, type AppContextValue } from "../../context/AppContext";
import type { FileChangeRecord, ToolCallSummary, WorkflowRunState } from "../../lib/types";
import {
  FileChangesPanel,
  resetFileChangesPanelExpandStateForTests,
} from "./FileChangesPanel";

const loadFileChangeDiff = vi.hoisted(() => vi.fn());

vi.mock("../../api", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../../api")>()),
  loadFileChangeDiff,
}));

function record(
  path: string,
  timestampMs: number,
  overrides: Partial<FileChangeRecord> = {},
): FileChangeRecord {
  return {
    path,
    op: "update",
    timestampMs,
    ...overrides,
  };
}

function toolCall(toolName: string): ToolCallSummary {
  return {
    toolCallId: `call-${toolName}`,
    toolName,
    status: "completed",
    arguments: {},
    lastOutput: "ok",
    isError: false,
    streaming: false,
  };
}

function context(
  changedFiles: FileChangeRecord[],
  toolCalls: ToolCallSummary[] = [],
): AppContextValue {
  const runState = {
    runId: "run-1",
    changedFilesByNode: { "node-1": changedFiles },
    toolCallsByNode: { "node-1": toolCalls },
  } as unknown as WorkflowRunState;
  return { runState: () => runState } as unknown as AppContextValue;
}

describe("FileChangesPanel", () => {
  let container: HTMLDivElement;
  let dispose: (() => void) | undefined;

  beforeEach(() => {
    container = document.createElement("div");
    document.body.append(container);
  });

  afterEach(() => {
    dispose?.();
    container.remove();
    loadFileChangeDiff.mockReset();
    resetFileChangesPanelExpandStateForTests();
  });

  function renderPanel(
    changedFiles: FileChangeRecord[],
    toolCalls: ToolCallSummary[] = [],
  ) {
    dispose = render(
      () => (
        <AppContext.Provider value={context(changedFiles, toolCalls)}>
          <FileChangesPanel nodeId="node-1" />
        </AppContext.Provider>
      ),
      container,
    );
  }

  function expandPanel() {
    (
      container.querySelector(".file-changes-panel-header") as HTMLButtonElement
    ).click();
  }

  it("starts collapsed and preserves repeated edits in chronological order", () => {
    renderPanel([
      record("same.ts", 20, { diffSummary: "+2|second" }),
      record("same.ts", 10, { op: "create", diffSummary: "+1|first" }),
    ]);

    expect(container.textContent).toContain("1 file changed · 2 edits");
    expect(container.querySelector(".file-changes-list")).toBeNull();

    expandPanel();

    const rows = [...container.querySelectorAll(".file-change-row")];
    expect(rows).toHaveLength(2);
    expect(rows[0]?.textContent).toContain("Created");
    expect(rows[1]?.textContent).toContain("Updated");
  });

  it("loads exact diffs lazily and retries a failed load", async () => {
    loadFileChangeDiff
      .mockRejectedValueOnce(new Error("artifact unavailable"))
      .mockResolvedValueOnce("-1|old\n+1|new");
    renderPanel([
      record("src/main.ts", 1, {
        diffArtifactId: "artifact-1",
        diffSizeBytes: 18,
      }),
    ]);
    expandPanel();

    (container.querySelector(".file-change-action") as HTMLButtonElement).click();
    await vi.waitFor(() => {
      expect(container.textContent).toContain("artifact unavailable");
    });
    expect(loadFileChangeDiff).toHaveBeenCalledWith("run-1", "artifact-1");

    (container.querySelector(".file-change-retry") as HTMLButtonElement).click();
    await vi.waitFor(() => {
      expect(container.querySelector(".file-edit-diff")?.textContent).toContain(
        "+1|new",
      );
    });
    expect(loadFileChangeDiff).toHaveBeenCalledTimes(2);
  });

  it("shows persisted legacy summaries without loading an artifact", () => {
    renderPanel([record("legacy.rs", 1, { diffSummary: "+1|legacy" })]);
    expandPanel();

    expect(container.textContent).toContain("Summary only");
    (container.querySelector(".file-change-action") as HTMLButtonElement).click();
    expect(container.querySelector(".file-edit-diff")?.textContent).toContain(
      "+1|legacy",
    );
    expect(loadFileChangeDiff).not.toHaveBeenCalled();
  });

  it("warns when bash may have changed untracked files", () => {
    renderPanel([], [toolCall("bash")]);
    expect(container.textContent).toContain("0 files changed · 0 edits");

    expandPanel();

    expect(container.textContent).toContain(
      "Shell, external tool, or MCP file writes may not appear here.",
    );
  });

  it("preserves expansion when a transcript update remounts the node panel", () => {
    const changes = [record("src/main.ts", 1, { diffSummary: "+1|new" })];
    renderPanel(changes);
    expandPanel();

    expect(
      container
        .querySelector(".file-changes-panel-header")
        ?.getAttribute("aria-expanded"),
    ).toBe("true");

    dispose?.();
    dispose = undefined;
    container.replaceChildren();
    renderPanel(changes);

    expect(
      container
        .querySelector(".file-changes-panel-header")
        ?.getAttribute("aria-expanded"),
    ).toBe("true");
    expect(container.querySelector(".file-changes-list")).not.toBeNull();
  });
});
