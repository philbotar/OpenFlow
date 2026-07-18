// @vitest-environment jsdom
import { createSignal } from "solid-js";
import { render } from "solid-js/web";
import { describe, expect, it, vi } from "vitest";
import { AppContext, type AppContextValue } from "../context/AppContext";
import { AppHeader } from "./AppHeader";

vi.mock("./Spinner", () => ({
  Spinner: () => <span data-testid="spinner" />,
}));

function makeMockContext(overrides: Partial<AppContextValue> = {}): AppContextValue {
  return {
    rightPanelHidden: () => false,
    leftPanelHidden: () => false,
    screen: () => "editor",
    handleToggleRightPanel: vi.fn(),
    handleToggleLeftPanel: vi.fn(),
    isCompactViewport: () => false,
    sidebarDrawerOpen: () => false,
    openSidebarDrawer: vi.fn(),
    closeSidebarDrawer: vi.fn(),
    toggleSidebarDrawer: vi.fn(),
    isMaximized: () => false,
    appReady: () => true,
    activeWorkflow: () => ({ id: "w1", name: "My Workflow" } as any),
    readiness: () => ({ ready: true, provider: "OpenAI", message: "Ready", envVar: "" }),
    runState: () => null,
    backendRunWorkflowId: () => null,
    startingRun: () => false,
    continuableRun: () => false,
    stoppingRun: () => false,
    workflowSettingsOpen: () => false,
    inspectorOpen: () => false,
    gitPanelOpen: () => false,
    handleToggleWorkflowSettings: vi.fn(),
    handleToggleInspector: vi.fn(),
    handleToggleGitPanel: vi.fn(),
    persistAll: vi.fn().mockResolvedValue(true),
    handleRun: vi.fn().mockResolvedValue(undefined),
    handleContinueRun: vi.fn().mockResolvedValue(undefined),
    handleStopRun: vi.fn().mockResolvedValue(undefined),
    workflows: () => [],
    projects: () => [],
    agents: () => [],
    activeWorkflowId: () => null,
    selectedNodeId: () => null,
    selectedEdgeId: () => null,
    settings: () => ({ active_provider: "openai", providers: {} }),
    bottomTab: () => "chat",
    dockOpen: () => true,
    dockHeight: () => 300,
    selectedTraceIndex: () => null,
    schemaText: () => "",
    newModelInputByProvider: () => ({} as Record<string, string>),
    providerKeyInputByProvider: () => ({} as Record<string, string>),
    uiZoom: () => 1,
    selectedProjectId: () => null,
    editingWorkflowId: () => null,
    workflowNameDraft: () => "",
    selectedAgentId: () => null,
    editingAgentId: () => null,
    agentNameDraft: () => "",
    editingNodeId: () => null,
    nodeLabelDraft: () => "",
    agentSchemaDraft: () => "",
    addNodePickerOpen: () => false,
    assignWorkflowPickerProjectId: () => null,
    availableSkills: () => [],
    skillById: () => new Map(),
    themePreference: () => "system" as const,
    resolvedTheme: () => "light",
    chatFilterNodeId: () => null,
    chatFocusNode: () => null,
    pickedLiveNodeId: () => null,
    chatSegmentOrder: () => [],
    setWorkflowNameDraft: () => {},
    setAgentNameDraft: () => {},
    setChatFilterNodeId: () => {},
    setPickedLiveNodeId: () => {},
    setChatDraft: () => {},
    setNewModelInputByProvider: () => {},
    setProviderKeyInputByProvider: () => {},
    setNodeLabelDraft: () => {},
    setSchemaText: () => {},
    setSelectedTraceIndex: () => {},
    setSelectedAgentId: () => {},
    setScreen: () => {},
    navigateToScreen: () => {},
    activeProject: () => undefined,
    gitRepoAvailable: () => false,
    independentWorkflows: () => [],
    executionCwdForActiveWorkflow: () => null,
    selectedAgent: () => null,
    canvasGraph: () => null,
    canvasStatusByNode: () => null,
    canvasSubagentsByNode: () => null,
    currentNode: () => undefined,
    activeProfileMemo: () => ({ active_provider: "openai", providers: {} }),
    providerIdsMemo: () => [],
    activeProviderKeyInput: () => "",
    selectedTrace: () => null,
    hasRunTraceMemo: () => false,
    currentNodeOutput: () => null,
    chatLayout: () => ({ settled: [], live: [], liveIds: [] }),
    chatDraft: () => "",
    chatSubmissionFor: () => ({ kind: "idle" }),
    canSendChatFor: () => false,
    composerBusyFor: () => false,
    setWorkflowNameInputRef: () => {},
    setAgentNameInputRef: () => {},
    handleSwitchWorkflow: () => {},
    handleCreateWorkflow: async () => {},
    handleOpenAssignWorkflowPicker: () => {},
    closeAssignWorkflowPicker: () => {},
    workflowsAddableToProject: () => [],
    handleCopyWorkflowToProject: async () => {},
    handleDeleteActiveWorkflow: async () => {},
    handleOpenAgents: () => {},
    handleAddProject: async () => {},
    handleSelectProject: () => {},
    handleToggleProjectExpanded: () => {},
    isProjectExpanded: () => false,
    workflowsForProject: () => [],
    handleCreateAgent: async () => {},
    handleSaveAgents: async () => {},
    handleAgentSchemaInput: () => {},
    updateSelectedAgent: () => {},
    handleStartAgentNameEdit: () => {},
    handleCancelAgentNameEdit: () => {},
    handleAgentNameCommit: () => {},
    handleAgentNameKeyDown: () => {},
    handleSaveSettings: async () => {},
    handleAddKnownModel: () => {},
    handleRemoveKnownModel: () => {},
    handleAddReasoningEffortOption: () => {},
    handleRemoveReasoningEffortOption: () => {},
    handleApiKeyInput: () => {},
    updateSettings: async () => {},
    handleSelectNode: () => {},
    handleSelectEdge: () => {},
    handleCanvasNodePosition: () => {},
    handleAutoLayoutWorkflow: () => {},
    handleCreateEdge: () => {},
    handleReconnectEdge: () => {},
    handleDeleteEdge: () => {},
    handleDeleteSelectedNode: () => {},
    handleOpenAddNodePicker: () => {},
    handleAddNode: async () => {},
    closeAddNodePicker: () => {},
    handleInterruptNode: async () => {},
    handleRetryNode: async () => {},
    handleSetThemePreference: () => {},
    handleClearRunTrace: async () => {},
    handleRefreshRunHistory: async () => {},
    handleReplayRun: async () => {},
    handleExitReplay: async () => {},
    handleResumeDurableRun: async () => {},
    handleSubmitChat: async () => {},
    handleRefreshSkills: async () => {},
    handleToolApproval: async () => {},
    handleUpdateNodeRuntimeConfig: async () => {},
    handleStartNodeLabelEdit: () => {},
    handleCancelNodeLabelEdit: () => {},
    handleCommitNodeLabel: () => {},
    handleStartWorkflowNameEdit: () => {},
    handleCancelWorkflowNameEdit: () => {},
    handleWorkflowNameCommit: () => {},
    handleWorkflowNameKeyDown: () => {},
    handleChatInputKeyDown: () => {},
    updateCurrentNode: () => {},
    updateCurrentNodeToolConfig: () => {},
    applySchemaEditor: () => true,
    handleSelectBottomTab: () => {},
    handleDockResizePointerDown: () => {},
    handleZoomIn: () => {},
    handleZoomOut: () => {},
    handleZoomReset: () => {},
    updateActiveWorkflowSettings: () => {},
    ...overrides,
  } as AppContextValue;
}

function renderWithContext(overrides: Partial<AppContextValue> = {}) {
  const ctx = makeMockContext(overrides);
  const container = document.createElement("div");
  document.body.append(container);
  const dispose = render(
    () => (
      <AppContext.Provider value={ctx}>
        <AppHeader />
      </AppContext.Provider>
    ),
    container,
  );
  return { container, dispose, ctx };
}

function getLeftSidebarToggle(container: HTMLElement) {
  return container.querySelector(
    "button[aria-label='Hide left sidebar'], button[aria-label='Show left sidebar']",
  ) as HTMLButtonElement | null;
}

describe("AppHeader", () => {
  it("does not render right panel toggle on editor screen", () => {
    const { container, dispose } = renderWithContext({
      screen: () => "editor",
      rightPanelHidden: () => false,
    });
    expect(container.querySelector("button[aria-label='Show right panel']")).toBeNull();
    expect(container.querySelector("button[aria-label='Hide right panel']")).toBeNull();
    dispose();
  });

  it("renders left sidebar toggle on editor screen", () => {
    const { container, dispose } = renderWithContext({
      screen: () => "editor",
      rightPanelHidden: () => false,
    });
    const btn = getLeftSidebarToggle(container);
    expect(btn).not.toBeNull();
    dispose();
  });

  it("left sidebar toggle aria-label includes sidebar", () => {
    const { container, dispose } = renderWithContext({
      screen: () => "editor",
      rightPanelHidden: () => false,
    });
    const btn = getLeftSidebarToggle(container);
    expect(btn!.getAttribute("aria-label")).toContain("sidebar");
    dispose();
  });

  it("hides editor utility buttons on non-editor screens", () => {
    const { container, dispose } = renderWithContext({
      screen: () => "settings",
      rightPanelHidden: () => false,
    });
    expect(container.querySelector("button[aria-label='Inspector']")).toBeNull();
    expect(container.querySelector("button[aria-label='Workflow settings']")).toBeNull();
    dispose();
  });

  it("does not render a topbar run action", () => {
    const { container, dispose } = renderWithContext({
      screen: () => "editor",
    });
    expect(container.querySelector("button[aria-label='Run workflow']")).toBeNull();
    dispose();
  });

  it("shows stop action while a run is active", () => {
    const { container, dispose } = renderWithContext({
      runState: () => ({ active: true } as any),
    });
    expect(container.querySelector("button[aria-label='Stop workflow']")).not.toBeNull();
    expect(container.querySelector("button[aria-label='Run workflow']")).toBeNull();
    dispose();
  });

  it("shows compact nav trigger only in compact viewport", () => {
    const { container, dispose } = renderWithContext({
      isCompactViewport: () => true,
    });
    expect(container.querySelector("button[aria-label='Open navigation']")).not.toBeNull();
    dispose();
  });

  it("hides compact nav trigger on desktop viewport", () => {
    const { container, dispose } = renderWithContext({
      isCompactViewport: () => false,
    });
    expect(container.querySelector("button[aria-label='Open navigation']")).toBeNull();
    dispose();
  });

  it("hides sidebar toggles on settings screen", () => {
    const { container, dispose } = renderWithContext({
      screen: () => "settings",
      isCompactViewport: () => false,
    });
    expect(container.querySelector("button[aria-label='Hide left sidebar']")).toBeNull();
    dispose();
  });

  it("hides compact nav trigger on settings screen", () => {
    const { container, dispose } = renderWithContext({
      screen: () => "settings",
      isCompactViewport: () => true,
    });
    expect(container.querySelector("button[aria-label='Open navigation']")).toBeNull();
    dispose();
  });

  it("shows desktop sidebar toggle on non-compact viewport", () => {
    const { container, dispose } = renderWithContext({
      isCompactViewport: () => false,
    });
    expect(container.querySelector("button[aria-label='Hide left sidebar']")).not.toBeNull();
    dispose();
  });

  it("does not highlight left sidebar toggle when sidebar is open", () => {
    const { container, dispose } = renderWithContext({
      isCompactViewport: () => false,
      leftPanelHidden: () => false,
    });
    const toggle = container.querySelector(
      "button[aria-label='Hide left sidebar']",
    ) as HTMLButtonElement;
    expect(toggle.classList.contains("topbar-icon-button-active")).toBe(false);
    expect(toggle.hasAttribute("aria-pressed")).toBe(false);
    dispose();
  });

  it("click calls handleToggleLeftPanel on desktop", () => {
    const handleToggleLeftPanel = vi.fn();
    const { container, dispose } = renderWithContext({
      isCompactViewport: () => false,
      leftPanelHidden: () => false,
      handleToggleLeftPanel,
    });
    const toggle = container.querySelector(
      "button[aria-label='Hide left sidebar']",
    ) as HTMLButtonElement;
    toggle.click();
    expect(handleToggleLeftPanel).toHaveBeenCalledTimes(1);
    dispose();
  });

  it("swaps left sidebar icon when toggled", () => {
    const [hidden, setHidden] = createSignal(false);
    const { container, dispose } = renderWithContext({
      isCompactViewport: () => false,
      leftPanelHidden: hidden,
      handleToggleLeftPanel: () => setHidden((value) => !value),
    });
    expect(container.querySelector("button[aria-label='Hide left sidebar']")).not.toBeNull();
    (
      container.querySelector("button[aria-label='Hide left sidebar']") as HTMLButtonElement
    ).click();
    expect(container.querySelector("button[aria-label='Show left sidebar']")).not.toBeNull();
    dispose();
  });

  it("shows Schedule in topbar on schedule screen", () => {
    const { container, dispose } = renderWithContext({
      screen: () => "schedule",
      activeWorkflow: () => ({ id: "w1", name: "Workflow One" } as any),
    });
    expect(container.querySelector(".topbar-title span")?.textContent).toBe("Schedule");
    dispose();
  });

  it("renders inspector toggle on editor screen", () => {
    const { container, dispose } = renderWithContext({
      screen: () => "editor",
    });
    expect(container.querySelector("button[aria-label='Inspector']")).not.toBeNull();
    dispose();
  });

  it("hides git toggle when no active project", () => {
    const { container, dispose } = renderWithContext({
      screen: () => "editor",
      activeProject: () => undefined,
    });
    expect(container.querySelector("button[aria-label='Git']")).toBeNull();
    dispose();
  });

  it("shows git toggle for project workflows with a git repo", () => {
    const { container, dispose } = renderWithContext({
      screen: () => "editor",
      activeProject: () =>
        ({
          id: "p1",
          name: "Demo",
          path: "/tmp/demo",
          workflow_ids: ["w1"],
        }) as any,
      gitRepoAvailable: () => true,
    });
    expect(container.querySelector("button[aria-label='Git']")).not.toBeNull();
    dispose();
  });

  it("hides git toggle when project cwd is not a git repo", () => {
    const { container, dispose } = renderWithContext({
      screen: () => "editor",
      activeProject: () =>
        ({
          id: "p1",
          name: "Demo",
          path: "/tmp/demo",
          workflow_ids: ["w1"],
        }) as any,
      gitRepoAvailable: () => false,
    });
    expect(container.querySelector("button[aria-label='Git']")).toBeNull();
    dispose();
  });

  it("click calls handleToggleInspector", () => {
    const handleToggleInspector = vi.fn();
    const { container, dispose } = renderWithContext({
      screen: () => "editor",
      handleToggleInspector,
    });
    const btn = container.querySelector("button[aria-label='Inspector']") as HTMLButtonElement;
    btn.click();
    expect(handleToggleInspector).toHaveBeenCalledTimes(1);
    dispose();
  });

  it("does not render continue or fresh run actions in the topbar", () => {
    const { container, dispose } = renderWithContext({
      continuableRun: () => true,
    });
    expect(container.querySelector("button[aria-label='Continue workflow']")).toBeNull();
    expect(container.querySelector("button[aria-label='Start fresh workflow run']")).toBeNull();
    dispose();
  });
});
