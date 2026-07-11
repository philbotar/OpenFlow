// @vitest-environment jsdom
import { render } from "solid-js/web";
import { describe, expect, it, vi } from "vitest";
import { AppContext, type AppContextValue } from "../context/AppContext";
import { EditorScreen } from "./EditorScreen";

vi.mock("../canvas/WorkflowCanvasHost", () => ({
  default: () => <div data-testid="canvas" />,
}));

vi.mock("../components", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../components")>()),
  NodePickerModal: () => <div data-testid="node-picker" />,
}));

vi.mock("../panels/DockPanel", () => ({
  DockPanel: () => <div data-testid="dock-panel" />,
}));

vi.mock("../panels/InspectorPanel", () => ({
  InspectorPanel: () => <div data-testid="inspector-panel" />,
}));

vi.mock("../panels/WorkflowSettingsPanel", () => ({
  WorkflowSettingsPanel: () => <div data-testid="workflow-settings-panel" />,
}));

function makeMockContext(overrides: Partial<AppContextValue> = {}): AppContextValue {
  return {
    rightPanelHidden: () => false,
    leftPanelHidden: () => false,
    selectedNodeId: () => null,
    workflowSettingsOpen: () => false,
    inspectorOpen: () => false,
    gitPanelOpen: () => false,
    dockOpen: () => true,
    dockHeight: () => 300,
    chatFocusMode: () => false,
    runState: () => null,
    backendRunWorkflowId: () => null,
    selectedEdgeId: () => null,
    resolvedTheme: () => "light",
    workflows: () => [],
    projects: () => [],
    agents: () => [],
    activeWorkflowId: () => null,
    screen: () => "editor",
    settings: () => ({
      active_provider: "openai",
      providers: {},
    }),
    readiness: () => null,
    bottomTab: () => "chat",
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
    isMaximized: () => false,
    availableSkills: () => [],
    skillById: () => new Map(),
    appReady: () => true,
    startingRun: () => false,
    continuableRun: () => false,
    themePreference: () => "system" as const,
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
    activeWorkflow: () => undefined,
    activeProject: () => undefined,
    gitRepoAvailable: () => false,
    independentWorkflows: () => [],
    executionCwdForActiveWorkflow: () => null,
    selectedAgent: () => null,
    canvasGraph: () => null,
    canvasStatusByNode: () => null,
    canvasSubagentsByNode: () => null,
    currentNode: () => undefined,
    activeProfileMemo: () => ({
      active_provider: "openai",
      providers: {},
    }),
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
    handleRun: async () => {},
    handleContinueRun: async () => {},
    handleStopRun: async () => {},
    handleInterruptNode: async () => {},
    handleRetryNode: async () => {},
    stoppingRun: () => false,
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
    persistAll: async () => true,
    handleSelectBottomTab: () => {},
    handleToggleChatFocusMode: () => {},
    handleDockResizePointerDown: () => {},
    isCompactViewport: () => false,
    sidebarDrawerOpen: () => false,
    openSidebarDrawer: () => {},
    closeSidebarDrawer: () => {},
    toggleSidebarDrawer: () => {},
    handleZoomIn: () => {},
    handleZoomOut: () => {},
    handleZoomReset: () => {},
    handleToggleWorkflowSettings: () => {},
    handleToggleInspector: () => {},
    handleToggleGitPanel: () => {},
    handleToggleRightPanel: () => {},
    handleToggleLeftPanel: () => {},
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
        <EditorScreen />
      </AppContext.Provider>
    ),
    container,
  );
  return { container, dispose };
}

describe("EditorScreen", () => {
  it("hides both panels when rightPanelHidden is true", () => {
    const { container, dispose } = renderWithContext({
      rightPanelHidden: () => true,
      selectedNodeId: () => "n1" as any,
      workflowSettingsOpen: () => true,
    });

    expect(container.querySelector('[data-testid="inspector-panel"]')).toBeNull();
    expect(container.querySelector('[data-testid="workflow-settings-panel"]')).toBeNull();
    dispose();
  });

  it("renders InspectorPanel when inspector is open", () => {
    const { container, dispose } = renderWithContext({
      rightPanelHidden: () => false,
      selectedNodeId: () => "n1" as any,
      workflowSettingsOpen: () => false,
      inspectorOpen: () => true,
    });

    expect(container.querySelector('[data-testid="inspector-panel"]')).not.toBeNull();
    expect(container.querySelector('[data-testid="workflow-settings-panel"]')).toBeNull();
    dispose();
  });

  it("renders WorkflowSettingsPanel when rightPanelHidden is false and settings open", () => {
    const { container, dispose } = renderWithContext({
      rightPanelHidden: () => false,
      selectedNodeId: () => null,
      workflowSettingsOpen: () => true,
    });

    expect(container.querySelector('[data-testid="inspector-panel"]')).toBeNull();
    expect(container.querySelector('[data-testid="workflow-settings-panel"]')).not.toBeNull();
    dispose();
  });

  it("applies editor-screen--no-right-panel class when rightPanelHidden is true", () => {
    const { container, dispose } = renderWithContext({
      rightPanelHidden: () => true,
      selectedNodeId: () => null,
      workflowSettingsOpen: () => false,
    });

    const screen = container.querySelector(".editor-screen");
    expect(screen).not.toBeNull();
    expect(screen!.classList.contains("editor-screen--no-right-panel")).toBe(true);
    dispose();
  });

  it("does not apply editor-screen--no-right-panel class when inspector is open", () => {
    const { container, dispose } = renderWithContext({
      rightPanelHidden: () => false,
      selectedNodeId: () => "n1" as any,
      workflowSettingsOpen: () => false,
      inspectorOpen: () => true,
    });

    const screen = container.querySelector(".editor-screen");
    expect(screen).not.toBeNull();
    expect(screen!.classList.contains("editor-screen--no-right-panel")).toBe(false);
    dispose();
  });

  it("hides inspector panel when inspector is open without a selected node", () => {
    const { container, dispose } = renderWithContext({
      rightPanelHidden: () => false,
      selectedNodeId: () => null,
      workflowSettingsOpen: () => false,
      inspectorOpen: () => true,
    });

    expect(container.querySelector('[data-testid="inspector-panel"]')).toBeNull();
    expect(container.querySelector(".editor-screen--no-right-panel")).not.toBeNull();
    dispose();
  });

  it("applies editor-screen--no-right-panel when no selection and panel not hidden", () => {
    const { container, dispose } = renderWithContext({
      rightPanelHidden: () => false,
      selectedNodeId: () => null,
      workflowSettingsOpen: () => false,
    });

    const screen = container.querySelector(".editor-screen");
    expect(screen).not.toBeNull();
    expect(screen!.classList.contains("editor-screen--no-right-panel")).toBe(true);
    dispose();
  });

  it("applies dock focus class when dock focus mode is active", () => {
    const { container, dispose } = renderWithContext({
      chatFocusMode: () => true,
      bottomTab: () => "terminal",
      dockOpen: () => true,
    });

    const screen = container.querySelector(".editor-screen");
    expect(screen?.classList.contains("editor-screen--chat-focus")).toBe(true);
    dispose();
  });

  it("keeps inspector visible in chat focus when open", () => {
    const { container, dispose } = renderWithContext({
      chatFocusMode: () => true,
      dockOpen: () => true,
      rightPanelHidden: () => false,
      selectedNodeId: () => "n1" as any,
      inspectorOpen: () => true,
    });

    const screen = container.querySelector(".editor-screen");
    expect(screen?.classList.contains("editor-screen--chat-focus")).toBe(true);
    expect(screen?.classList.contains("editor-screen--no-right-panel")).toBe(false);
    expect(container.querySelector('[data-testid="inspector-panel"]')).not.toBeNull();
    dispose();
  });

  it("keeps canvas visible beside workflow settings panel", () => {
    const { container, dispose } = renderWithContext({
      workflowSettingsOpen: () => true,
      rightPanelHidden: () => false,
    });

    const screen = container.querySelector(".editor-screen");
    expect(screen?.classList.contains("editor-screen--settings-focus")).toBe(false);
    expect(container.querySelector(".editor-main")).not.toBeNull();
    expect(container.querySelector('[data-testid="workflow-settings-panel"]')).not.toBeNull();
    dispose();
  });

  it("uses collapsed dock height when dock is closed", () => {
    const { container, dispose } = renderWithContext({
      dockOpen: () => false,
      dockHeight: () => 300,
    });

    const screen = container.querySelector(".editor-screen") as HTMLElement;
    expect(screen.style.getPropertyValue("--dock-height")).toBe("52px");
    dispose();
  });
});
