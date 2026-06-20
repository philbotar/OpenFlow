// @vitest-environment jsdom
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
    screen: () => "editor",
    handleToggleRightPanel: vi.fn(),
    isMaximized: () => false,
    appReady: () => true,
    activeWorkflow: () => ({ id: "w1", name: "My Workflow" } as any),
    readiness: () => ({ ready: true, provider: "OpenAI", message: "Ready", envVar: "" }),
    runState: () => null,
    startingRun: () => false,
    continuableRun: () => false,
    stoppingRun: () => false,
    workflowSettingsOpen: () => false,
    handleToggleWorkflowSettings: vi.fn(),
    persistAll: vi.fn().mockResolvedValue(true),
    handleValidate: vi.fn().mockResolvedValue(undefined),
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
    bottomTab: () => "overview",
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
    shortcutsModalOpen: () => false,
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
    activeProject: () => undefined,
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
    handleAssignWorkflowToProject: async () => {},
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
    handleApiKeyInput: () => {},
    updateSettings: async () => {},
    handleSelectNode: () => {},
    handleSelectEdge: () => {},
    handleCanvasNodePosition: () => {},
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
    openShortcutsModal: () => {},
    closeShortcutsModal: () => {},
    handleClearRunTrace: async () => {},
    handleSubmitChat: async () => {},
    handleRefreshSkills: async () => {},
    handleToolApproval: async () => {},
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

function getToggleButton(container: HTMLElement) {
  return container.querySelector("button[aria-label*='panel']") as HTMLButtonElement | null;
}

describe("AppHeader", () => {
  it("renders toggle button on editor screen", () => {
    const { container, dispose } = renderWithContext({
      screen: () => "editor",
      rightPanelHidden: () => false,
    });
    const btn = getToggleButton(container);
    expect(btn).not.toBeNull();
    dispose();
  });

  it("toggle button aria-label includes 'panel'", () => {
    const { container, dispose } = renderWithContext({
      screen: () => "editor",
      rightPanelHidden: () => false,
    });
    const btn = getToggleButton(container);
    expect(btn!.getAttribute("aria-label")).toContain("panel");
    dispose();
  });

  it("aria-pressed is true when panel is visible", () => {
    const { container, dispose } = renderWithContext({
      screen: () => "editor",
      rightPanelHidden: () => false,
    });
    const btn = getToggleButton(container);
    expect(btn!.getAttribute("aria-pressed")).toBe("true");
    dispose();
  });

  it("aria-pressed is false when panel is hidden", () => {
    const { container, dispose } = renderWithContext({
      screen: () => "editor",
      rightPanelHidden: () => true,
    });
    const btn = getToggleButton(container);
    expect(btn!.getAttribute("aria-pressed")).toBe("false");
    dispose();
  });

  it("click calls handleToggleRightPanel", () => {
    const handleToggleRightPanel = vi.fn();
    const { container, dispose } = renderWithContext({
      screen: () => "editor",
      rightPanelHidden: () => false,
      handleToggleRightPanel,
    });
    const btn = getToggleButton(container);
    btn!.click();
    expect(handleToggleRightPanel).toHaveBeenCalledTimes(1);
    dispose();
  });

  it("no toggle button on non-editor screens", () => {
    const { container, dispose } = renderWithContext({
      screen: () => "settings",
      rightPanelHidden: () => false,
    });
    const btn = getToggleButton(container);
    expect(btn).toBeNull();
    dispose();
  });

  it("shows continue and fresh run when a stopped run is continuable", () => {
    const handleContinueRun = vi.fn().mockResolvedValue(undefined);
    const { container, dispose } = renderWithContext({
      continuableRun: () => true,
      handleContinueRun,
    });
    const continueBtn = container.querySelector(
      "button[aria-label='Continue workflow']",
    ) as HTMLButtonElement | null;
    const freshBtn = container.querySelector(
      "button[aria-label='Start fresh workflow run']",
    ) as HTMLButtonElement | null;
    expect(continueBtn).not.toBeNull();
    expect(freshBtn).not.toBeNull();
    continueBtn!.click();
    expect(handleContinueRun).toHaveBeenCalledTimes(1);
    dispose();
  });
});
