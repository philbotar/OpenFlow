// @vitest-environment jsdom
import { render } from "solid-js/web";
import { screen, waitFor } from "@testing-library/dom";
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import type { WorkflowRunState } from "@/lib/types";

const mocks = vi.hoisted(() => ({
  gitDiffRepo: vi.fn(),
  gitCurrentBranch: vi.fn(),
  runState: null as WorkflowRunState | null,
  executionCwd: "/tmp/project" as string | null,
}));

vi.mock("../port", () => ({
  createUiDesktopOutboundAdapter: () => mocks,
}));

vi.mock("../context/AppContext", () => ({
  useAppContext: () => ({
    runState: () => mocks.runState,
    executionCwdForActiveWorkflow: () => mocks.executionCwd,
  }),
}));

import { GitPanel } from "./GitPanel";

function emptyRunState(): WorkflowRunState {
  return {
    active: true,
    awaitingNodeId: null,
    activeManualNodeId: null,
    activeToolCallId: null,
    pendingApprovals: [],
    toolCallsByNode: {},
    toolArtifacts: {},
    execApprovalGranted: false,
    statusByNode: {},
    subagentsByNode: {},
    lastReport: null,
    lastError: null,
    chatLogs: {},
    runTrace: [],
    outputs: {},
    changedFiles: [],
    changedFilesByNode: {},
    editBatches: [],
  };
}

describe("GitPanel", () => {
  let dispose: (() => void) | undefined;
  let container: HTMLDivElement | undefined;

  beforeEach(() => {
    mocks.gitDiffRepo.mockReset();
    mocks.gitCurrentBranch.mockReset();
    mocks.gitCurrentBranch.mockResolvedValue("feat/git-pr-diff-panel");
    mocks.runState = emptyRunState();
    mocks.executionCwd = "/tmp/project";
  });

  afterEach(() => {
    dispose?.();
    container?.remove();
  });

  function mount() {
    container = document.createElement("div");
    document.body.append(container);
    dispose = render(() => <GitPanel />, container);
  }

  it("loads full repo git diff for the project cwd", async () => {
    mocks.gitDiffRepo.mockResolvedValue(
      "diff --git a/notes.md b/notes.md\n--- a/notes.md\n+++ b/notes.md\n@@ -1,1 +1,1 @@\n-old\n+new\n",
    );

    mount();
    await waitFor(() => expect(screen.getByText("feat/git-pr-diff-panel")).toBeTruthy());
    await waitFor(() => expect(screen.getByText("1 file · 1 changed")).toBeTruthy());
    await waitFor(() => expect(screen.getByText("notes.md")).toBeTruthy());
    expect(mocks.gitDiffRepo).toHaveBeenCalledWith("/tmp/project");
  });

  it("shows empty state when repo has no uncommitted changes", async () => {
    mocks.gitDiffRepo.mockResolvedValue("");

    mount();
    await waitFor(() => expect(screen.getByText("No uncommitted changes.")).toBeTruthy());
  });
});
