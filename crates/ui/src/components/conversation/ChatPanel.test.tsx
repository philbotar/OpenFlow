// @vitest-environment jsdom
import { createSignal } from "solid-js";
import { render } from "solid-js/web";
import { beforeAll, describe, expect, it, vi } from "vitest";
import { AppContext, type AppContextValue } from "../../context/AppContext";
import { GLOBAL_RUN_ENTRY_NODE_ID } from "../../lib/workflow";
import { createEmptyToolConfig } from "../../lib/workflow/testHelpers";
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
    handleRefreshRunHistory: vi.fn(),
    handleReplayRun: vi.fn(),
    handleSelectBottomTab: vi.fn(),
    runHistoryLoading: () => false,
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
    pendingChatAttachments: () => [],
    chatSubmissionFor: () => ({ kind: "idle", submittedText: "", invokedSkills: [] }),
    canSendChatFor: () => false,
    composerBusyFor: () => false,
    readiness: () => ({ ready: true }),
    availableSkills: () => [],
    skillById: () => new Map(),
    setChatDraft: () => {},
    handleSubmitChat: async () => {},
    handleSubmitStructuredInput: async () => {},
    handlePickChatAttachments: async () => {},
    handleStageChatAttachments: async () => {},
    handleRemovePendingChatAttachment: async () => {},
    handleChatInputKeyDown: () => {},
    searchProjectFileReferences: async () => [],
    handleStopRun: async () => {},
    stoppingRun: () => false,
    handleInterruptNode: async () => {},
    handleRetryNode: async () => {},
    handleUpdateNodeRuntimeConfig: async () => {},
    screen: () => "editor",
    activeProfileMemo: () => ({ reasoning_effort_options: [] }),
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
  it("shows a composer instead of replay actions and submits a continuation message", () => {
    const handleSubmitChat = vi.fn(async () => {});
    const { container, dispose } = renderChatPanel({
      runHistory: () => [
        {
          runId: "run-1",
          name: "Stopped workflow run",
          workflowId: "w1",
          workflowName: "Workflow",
          projectId: null,
          startedAtMs: 0,
          status: "stopped",
          updatedAtMs: 1,
        },
      ],
      activeWorkflow: () =>
        ({
          id: "w1",
          name: "Workflow",
          nodes: [{ id: "node-1", label: "Plan" }],
          edges: [],
          settings: {},
        }) as unknown as ReturnType<AppContextValue["activeWorkflow"]>,
      runState: () =>
        ({
          active: false,
          runId: "run-1",
          pendingApprovals: [],
          statusByNode: { "node-1": "stopped" },
          chatLogs: { "node-1": [] },
          toolCallsByNode: {},
          awaitingNodeIds: [],
        }) as unknown as ReturnType<AppContextValue["runState"]>,
      chatDraft: () => "Continue with verification",
      canSendChatFor: () => true,
      handleSubmitChat,
    });
    try {
      expect(container.textContent).not.toContain("Viewing saved run");
      expect(container.textContent).not.toContain("Exit replay");
      expect(container.textContent).not.toContain("Resume run");
      expect(container.querySelector("textarea")).not.toBeNull();
      expect(container.querySelector('[aria-label="Previous runs"]')).toBeNull();
      expect(
        container.querySelector<HTMLButtonElement>('button[aria-label="Attach files"]')
          ?.disabled,
      ).toBe(true);

      const send = container.querySelector<HTMLButtonElement>(
        'button[aria-label="Continue saved run with message"]',
      );
      expect(send).not.toBeNull();
      send!.click();
      expect(handleSubmitChat).toHaveBeenCalledWith(GLOBAL_RUN_ENTRY_NODE_ID);
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

  it("hides previous runs while replay is opening", () => {
    let finishReplay!: () => void;
    const handleReplayRun = vi.fn(
      () =>
        new Promise<void>((resolve) => {
          finishReplay = resolve;
        }),
    );
    const { container, dispose } = renderChatPanel({
      replayRunId: () => null,
      handleReplayRun,
      runHistory: () => [
        {
          runId: "run-1",
          name: "Audit provider retry behavior",
          workflowId: "w1",
          workflowName: "Workflow",
          projectId: null,
          startedAtMs: 1,
          updatedAtMs: 2,
          status: "completed",
        },
      ],
    });

    try {
      const viewRun = container.querySelector<HTMLButtonElement>(
        'button[aria-label="View saved run run-1"]',
      );
      expect(viewRun).not.toBeNull();

      viewRun!.click();

      expect(handleReplayRun).toHaveBeenCalledWith("run-1");
      expect(container.querySelector('[aria-label="Previous runs"]')).toBeNull();
      finishReplay();
    } finally {
      dispose();
      container.remove();
    }
  });

  it("hides previous runs when a saved run is loaded", () => {
    const { container, dispose } = renderChatPanel({
      replayRunId: () => null,
      runState: () =>
        ({
          active: false,
          runId: "run-1",
          pendingApprovals: [],
          statusByNode: {},
          chatLogs: {},
          toolCallsByNode: {},
          awaitingNodeIds: [],
        }) as unknown as ReturnType<AppContextValue["runState"]>,
      runHistory: () => [
        {
          runId: "run-2",
          name: "Previous workflow run",
          workflowId: "w1",
          workflowName: "Workflow",
          projectId: null,
          startedAtMs: 1,
          updatedAtMs: 2,
          status: "completed",
        },
      ],
    });

    try {
      expect(container.querySelector('[aria-label="Previous runs"]')).toBeNull();
    } finally {
      dispose();
      container.remove();
    }
  });

  it("shows three previous runs and hides them when a message is sent", async () => {
    const handleRefreshRunHistory = vi.fn(async () => {});
    const handleResumeDurableRun = vi.fn(async () => {});
    const handleSubmitChat = vi.fn(async () => {});
    const run = (
      runId: string,
      status: "completed" | "paused",
      updatedAtMs: number,
    ) => ({
      runId,
      name:
        runId === "run-1"
          ? "Audit provider retry behavior"
          : `Previous workflow run ${runId}`,
      workflowId: "w1",
      workflowName: "Workflow",
      projectId: null,
      startedAtMs: updatedAtMs - 100,
      updatedAtMs,
      status,
    });
    const { container, dispose } = renderChatPanel({
      replayRunId: () => null,
      runHistory: () => [
        run("run-1", "completed", 700),
        run("run-2", "paused", 600),
        run("run-3", "completed", 500),
        run("run-4", "completed", 400),
        run("run-5", "completed", 300),
        run("run-6", "completed", 200),
      ],
      chatDraft: () => "Continue from here",
      canSendChatFor: () => true,
      handleRefreshRunHistory,
      handleResumeDurableRun,
      handleSubmitChat,
    });

    try {
      await Promise.resolve();

      const picker = container.querySelector('[aria-label="Previous runs"]');
      expect(picker).not.toBeNull();
      expect(picker!.querySelectorAll("[data-run-id]")).toHaveLength(3);
      expect(picker!.querySelector('[data-run-id="run-4"]')).toBeNull();
      expect(picker!.querySelector('[data-run-id="run-6"]')).toBeNull();
      expect(picker!.textContent).toContain("Audit provider retry behavior");
      expect(handleRefreshRunHistory).toHaveBeenCalled();

      const completedRow = picker!.querySelector('[data-run-id="run-1"]')!;
      const completedStatus = completedRow.querySelector(".recent-run-status")!;
      expect(
        completedRow.querySelector(".recent-run-view")!.contains(completedStatus),
      ).toBe(false);
      expect(completedStatus.classList).toContain("recent-run-action");

      const pausedRow = picker!.querySelector('[data-run-id="run-2"]')!;
      expect(pausedRow.querySelector(".recent-run-continue")!.classList).toContain(
        "recent-run-action",
      );

      (
        picker!.querySelector(
          'button[aria-label="Continue saved run run-2"]',
        ) as HTMLButtonElement
      ).click();
      expect(handleResumeDurableRun).toHaveBeenCalledWith("run-2");

      (
        container.querySelector(
          'button[aria-label="Start workflow with message"]',
        ) as HTMLButtonElement
      ).click();
      expect(handleSubmitChat).toHaveBeenCalled();
      expect(container.querySelector('[aria-label="Previous runs"]')).toBeNull();
    } finally {
      dispose();
      container.remove();
    }
  });

  it("renders a structured ask and submits the selected answer", async () => {
    const handleSubmitStructuredInput = vi.fn(async () => {});
    const { container, dispose } = renderChatPanel({
      replayRunId: () => null,
      handleSubmitStructuredInput,
      chatLayout: () => ({
        settled: [
          {
            nodeId: "builder",
            label: "Builder",
            status: "awaiting_input",
            messages: [],
          },
        ],
        live: [],
        liveIds: [],
      }),
      runState: () =>
        ({
          active: true,
          pendingApprovals: [],
          statusByNode: { builder: "awaiting_input" },
          chatLogs: { builder: [] },
          toolCallsByNode: {},
          awaitingNodeId: "builder",
          awaitingNodeIds: ["builder"],
          structuredInputByNode: {
            builder: {
              questions: [
                {
                  id: "target_env",
                  header: "Target",
                  question: "Which environment should I target?",
                  options: [
                    {
                      label: "Staging",
                      description: "Use the shared staging environment.",
                    },
                    {
                      label: "Production",
                      description: "Use the live production environment.",
                    },
                  ],
                },
              ],
            },
          },
        }) as unknown as ReturnType<AppContextValue["runState"]>,
    });

    try {
      const production = container.querySelector<HTMLInputElement>(
        'input[type="radio"][value="Production"]',
      );
      production?.click();

      const submit = Array.from(container.querySelectorAll("button")).find(
        (button) => button.textContent?.includes("Submit answers"),
      );
      submit?.click();
      await Promise.resolve();

      expect(container.textContent).toContain("Which environment should I target?");
      expect(container.textContent).not.toContain("Other");
      expect(container.querySelector("textarea")).not.toBeNull();
      expect(handleSubmitStructuredInput).toHaveBeenCalledWith(
        "builder",
        "Structured answers:\n- target_env: Production",
      );
    } finally {
      dispose();
      container.remove();
    }
  });

  it("renders a submitted answer cleanly while showing a new structured request", () => {
    const answer = {
      role: "user" as const,
      content: "Structured answers:\n- next_step: Just chatting",
    };
    const { container, dispose } = renderChatPanel({
      replayRunId: () => null,
      chatLayout: () => ({
        settled: [
          {
            nodeId: "builder",
            label: "Builder",
            status: "awaiting_input",
            messages: [answer],
          },
        ],
        live: [],
        liveIds: [],
      }),
      runState: () =>
        ({
          active: true,
          pendingApprovals: [],
          statusByNode: { builder: "awaiting_input" },
          chatLogs: { builder: [answer] },
          toolCallsByNode: {},
          awaitingNodeId: "builder",
          awaitingNodeIds: ["builder"],
          structuredInputByNode: {
            builder: {
              questions: [
                {
                  id: "topic",
                  header: "Topic",
                  question: "What should we chat about?",
                  options: [
                    {
                      label: "Something personal",
                      description: "Talk about your day.",
                    },
                    {
                      label: "Something random",
                      description: "Share an interesting thought.",
                    },
                  ],
                },
              ],
            },
          },
        }) as unknown as ReturnType<AppContextValue["runState"]>,
    });

    try {
      expect(container.textContent).toContain("Just chatting");
      expect(container.textContent).toContain("What should we chat about?");
      expect(container.textContent).not.toContain("Structured answers");
      expect(container.textContent).not.toContain("next_step");
    } finally {
      dispose();
      container.remove();
    }
  });

  it("opens the current workflow in AI authoring with the selected suggestion", () => {
    const handleOpenWorkflowAuthoring =
      vi.fn<AppContextValue["handleOpenWorkflowAuthoring"]>(async () => {});
    const workflow = {
      id: "w1",
      name: "Workflow",
      nodes: [{ id: "builder", label: "Builder" }],
      edges: [],
      settings: {},
    } as unknown as ReturnType<AppContextValue["activeWorkflow"]>;
    const { container, dispose } = renderChatPanel({
      handleOpenWorkflowAuthoring,
      runState: () =>
        ({
          active: false,
          pendingApprovals: [],
          statusByNode: { builder: "completed" },
          chatLogs: {},
          toolCallsByNode: {},
          awaitingNodeIds: [],
          lastReport: {
            workflow_id: "w1",
            outputs: [],
            suggestions: [
              {
                id: "suggestion-1",
                category: "prompt",
                targetNodeId: "builder",
                title: "Require verification",
                evidence: "The agent skipped tests.",
                recommendation: "Add the focused test command to its prompt.",
              },
            ],
          },
        }) as unknown as ReturnType<AppContextValue["runState"]>,
      activeWorkflow: () => workflow,
      activeProject: () =>
        ({
          id: "project-1",
          name: "Project",
          path: "/tmp/project",
          default_execution_cwd: "/tmp/project",
          workflow_ids: ["w1"],
        }) as ReturnType<AppContextValue["activeProject"]>,
    });
    try {
      expect(container.textContent).toContain("Suggestions");
      expect(container.textContent).toContain("Require verification");
      expect(container.textContent).toContain("Builder");

      container
        .querySelector<HTMLButtonElement>(
          'button[aria-label="Apply Require verification with AI"]',
        )
        ?.click();

      expect(handleOpenWorkflowAuthoring).toHaveBeenCalledTimes(1);
      const [baseWorkflow, projectId, initialMessage] =
        handleOpenWorkflowAuthoring.mock.calls[0] ?? [];
      expect(baseWorkflow).toBe(workflow);
      expect(projectId).toBe("project-1");
      expect(initialMessage).toContain("Target node: Builder (builder)");
      expect(initialMessage).toContain("The agent skipped tests.");
      expect(initialMessage).toContain(
        "Add the focused test command to its prompt.",
      );
    } finally {
      dispose();
      container.remove();
    }
  });

  it("lets the user retry a failed workflow node", () => {
    const handleRetryNode = vi.fn(async () => {});
    const handleSubmitChat = vi.fn(async () => {});
    const { container, dispose } = renderChatPanel({
      replayRunId: () => null,
      handleRetryNode,
      handleSubmitChat,
      chatDraft: (nodeId) => (nodeId === "node-1" ? "Try again" : ""),
      canSendChatFor: (nodeId) => nodeId === "node-1",
      activeWorkflow: () =>
        ({
          id: "w1",
          name: "Workflow",
          nodes: [
            {
              id: "node-1",
              label: "Grill",
              kind: "Agent",
              position: { x: 0, y: 0 },
              agent: {
                system_prompt: "",
                task_prompt: "",
                model: "gpt-5",
                output_schema: { type: "object" },
                auto_start: true,
                tools: createEmptyToolConfig(),
                callable_agents: [],
                allow_all_callable_agents: false,
              },
            },
          ],
          edges: [],
          settings: {},
        }) as unknown as ReturnType<AppContextValue["activeWorkflow"]>,
      runState: () =>
        ({
          active: true,
          pendingApprovals: [],
          statusByNode: { "node-1": "failed" },
          chatLogs: {},
          toolCallsByNode: {},
          awaitingNodeIds: [],
        }) as unknown as ReturnType<AppContextValue["runState"]>,
    });
    try {
      expect(container.textContent).toContain("Grill failed");
      expect(container.textContent).not.toContain("Starting workflow…");
      const retry = Array.from(container.querySelectorAll("button")).find(
        (button) => button.textContent?.trim() === "Retry",
      );
      expect(retry).toBeTruthy();
      retry!.click();
      expect(handleRetryNode).toHaveBeenCalledWith("node-1");

      expect(container.querySelector("textarea")).not.toBeNull();
      const send = container.querySelector<HTMLButtonElement>(
        'button[aria-label="Send to paused node"]',
      );
      expect(send).not.toBeNull();
      send!.click();
      expect(handleSubmitChat).toHaveBeenCalledWith("node-1");
    } finally {
      dispose();
      container.remove();
    }
  });

  it("shows one reply composer when multiple nodes fail", () => {
    const agentNode = (id: string, label: string) =>
      ({
        id,
        label,
        kind: "Agent",
        position: { x: 0, y: 0 },
        agent: {
          system_prompt: "",
          task_prompt: "",
          model: "gpt-5",
          output_schema: { type: "object" },
          auto_start: true,
          tools: createEmptyToolConfig(),
          callable_agents: [],
          allow_all_callable_agents: false,
        },
      }) as const;

    const { container, dispose } = renderChatPanel({
      replayRunId: () => null,
      canSendChatFor: () => true,
      activeWorkflow: () =>
        ({
          id: "w1",
          name: "Workflow",
          nodes: [agentNode("node-1", "Architecture Design"), agentNode("node-2", "Test Plan")],
          edges: [],
          settings: {},
        }) as unknown as ReturnType<AppContextValue["activeWorkflow"]>,
      runState: () =>
        ({
          active: true,
          pendingApprovals: [],
          statusByNode: { "node-1": "failed", "node-2": "failed" },
          chatLogs: {},
          toolCallsByNode: {},
          awaitingNodeIds: [],
        }) as unknown as ReturnType<AppContextValue["runState"]>,
    });
    try {
      expect(container.textContent).toContain("Architecture Design failed");
      expect(container.textContent).toContain("Test Plan failed");
      expect(container.querySelectorAll("textarea")).toHaveLength(1);
      expect(container.querySelector(".composer-input-placeholder")?.textContent ?? "").toContain(
        "Architecture Design",
      );
    } finally {
      dispose();
      container.remove();
    }
  });

  it("shows review progress after every workflow node completes", () => {
    const { container, dispose } = renderChatPanel({
      replayRunId: () => null,
      runState: () =>
        ({
          active: true,
          pendingApprovals: [],
          statusByNode: { "node-1": "completed" },
          chatLogs: {},
          toolCallsByNode: {},
          awaitingNodeIds: [],
        }) as unknown as ReturnType<AppContextValue["runState"]>,
    });
    try {
      expect(container.textContent).toContain("Reviewing run…");
      expect(container.textContent).not.toContain("Starting workflow…");
    } finally {
      dispose();
      container.remove();
    }
  });

  it("shows the Plan Mode lock until the configured review node completes", () => {
    const { container, dispose } = renderChatPanel({
      replayRunId: () => null,
      activeWorkflow: () => ({
        id: "w1",
        name: "Workflow",
        nodes: [{ id: "freeze", label: "Review & freeze" }],
        edges: [],
        settings: { planMode: { evidenceSourceNodeId: "freeze" } },
      }) as unknown as ReturnType<AppContextValue["activeWorkflow"]>,
      runState: () =>
        ({
          active: true,
          pendingApprovals: [],
          statusByNode: { freeze: "awaiting_input" },
          chatLogs: {},
          toolCallsByNode: {},
          awaitingNodeIds: [],
        }) as unknown as ReturnType<AppContextValue["runState"]>,
    });
    try {
      expect(container.textContent).toContain("Plan mode");
      expect(container.textContent).toContain("Planning in progress");
      expect(container.textContent).toContain("Review & freeze");
    } finally {
      dispose();
      container.remove();
    }
  });

  it("uses the run-pinned Plan Mode phase in replay after workflow settings change", () => {
    const { container, dispose } = renderChatPanel({
      replayRunId: () => null,
      activeWorkflow: () => ({
        id: "w1",
        name: "Workflow",
        nodes: [{ id: "freeze", label: "Review & freeze" }],
        edges: [],
        settings: {},
      }) as unknown as ReturnType<AppContextValue["activeWorkflow"]>,
      runState: () =>
        ({
          active: false,
          pendingApprovals: [],
          statusByNode: { freeze: "awaiting_input" },
          chatLogs: {},
          toolCallsByNode: {},
          awaitingNodeIds: [],
          planMode: { evidenceSourceNodeId: "freeze", phase: "execution" },
        }) as unknown as ReturnType<AppContextValue["runState"]>,
    });
    try {
      expect(container.textContent).toContain("Plan mode");
      expect(container.textContent).toContain("approved the plan");
      expect(container.textContent).toContain("Review & freeze");
    } finally {
      dispose();
      container.remove();
    }
  });
});
