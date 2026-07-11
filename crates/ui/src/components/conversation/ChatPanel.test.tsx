// @vitest-environment jsdom
import { createSignal } from "solid-js";
import { render } from "solid-js/web";
import { beforeAll, describe, expect, it, vi } from "vitest";
import { AppContext, type AppContextValue } from "../../context/AppContext";
import { ChatPanel } from "./ChatPanel";

beforeAll(() => {
  Element.prototype.scrollTo = vi.fn() as unknown as typeof Element.prototype.scrollTo;
  class ResizeObserverStub {
    observe() {}
    unobserve() {}
    disconnect() {}
  }
  vi.stubGlobal("ResizeObserver", ResizeObserverStub);
});

function renderChatPanel(overrides: Partial<AppContextValue> = {}) {
  const [replayRunId, setReplayRunId] = createSignal<string | null>("run-1");
  const handleExitReplay = vi.fn(async () => {
    setReplayRunId(null);
  });
  const ctx = {
    replayRunId,
    runHistory: () => [
      {
        runId: "run-1",
        workflowId: "w1",
        workflowName: "Workflow",
        status: "completed",
        updatedAtMs: 1,
      },
    ],
    handleExitReplay,
    handleResumeDurableRun: vi.fn(),
    chatLayout: () => ({ settled: [], live: [], liveIds: [] }),
    chatFilterNodeId: () => null,
    setChatFilterNodeId: () => {},
    pickedLiveNodeId: () => null,
    setPickedLiveNodeId: () => {},
    chatSegmentOrder: () => [],
    chatFocusNode: () => null,
    runState: () => ({
      active: false,
      pendingApprovals: [],
      statusByNode: {},
      chatLogs: {},
      toolCallsByNode: {},
      awaitingNodeIds: [],
    }),
    startingRun: () => false,
    chatDraft: () => "",
    chatSubmissionFor: () => ({ kind: "idle", submittedText: "", invokedSkills: [] }),
    canSendChatFor: () => false,
    composerBusyFor: () => false,
    readiness: () => ({ ready: true }),
    availableSkills: () => [],
    skillById: () => new Map(),
    setChatDraft: () => {},
    handleSubmitChat: async () => {},
    handleChatInputKeyDown: () => {},
    searchProjectFileReferences: async () => [],
    handleInterruptNode: async () => {},
    handleRetryNode: async () => {},
    handleUpdateNodeRuntimeConfig: async () => {},
    activeWorkflow: () => ({ id: "w1", name: "Workflow", nodes: [], edges: [] }),
    ...overrides,
  } as unknown as AppContextValue;

  const container = document.createElement("div");
  document.body.appendChild(container);
  const dispose = render(
    () => (
      <AppContext.Provider value={ctx}>
        <ChatPanel />
      </AppContext.Provider>
    ),
    container,
  );
  return { container, dispose, handleExitReplay, replayRunId };
}

describe("ChatPanel replay mode", () => {
  it("shows Exit replay for completed runs", () => {
    const { container, dispose, handleExitReplay } = renderChatPanel();
    try {
      expect(container.textContent).toContain("Viewing saved run");
      expect(container.textContent).toContain("Exit replay");
      expect(container.textContent).not.toContain("Resume run");
      expect(container.querySelector("textarea")).toBeNull();

      const exit = Array.from(container.querySelectorAll("button")).find((button) =>
        button.textContent?.includes("Exit replay"),
      );
      expect(exit).toBeTruthy();
      exit!.click();
      expect(handleExitReplay).toHaveBeenCalled();
    } finally {
      dispose();
      container.remove();
    }
  });

  it("shows kickoff composer when not in replay", () => {
    const { container, dispose } = renderChatPanel({
      replayRunId: () => null,
    });
    try {
      expect(container.textContent).not.toContain("Viewing saved run");
      expect(container.querySelector("textarea")).not.toBeNull();
    } finally {
      dispose();
      container.remove();
    }
  });
});
