// @vitest-environment jsdom
import { render } from "solid-js/web";
import { describe, expect, it, vi } from "vitest";
import { AppContext, type AppContextValue } from "../context/AppContext";
import { EditorScreen } from "./EditorScreen";

vi.mock("../canvas/WorkflowCanvasHost", () => ({
  default: () => <div data-testid="canvas" />,
}));

vi.mock("../components/NodePickerModal", () => ({
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
    selectedNodeId: () => null,
    workflowSettingsOpen: () => false,
    dockOpen: () => true,
    dockHeight: () => 300,
    runState: () => null,
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
    bottomTab: () => "overview",
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
    themePreference: () => "system" as const,
    shortcutsModalOpen: () => false,
    chatFilterNodeId: () => null,
    chatFocusNode: () => null,
    pickedLiveNodeId: () => null,
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
    activeWorkflow: () => undefined,
    activeProject: () => undefined,
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
    handleValidate: async () => {},
    handleRun: async () => {},
    handleStopRun: async () => {},
    handleInterruptNode: async () => {},
    handleRetryNode: async () => {},
    stoppingRun: () => false,
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
    setToolEnabled: () => {},
    applySchemaEditor: () => true,
    persistAll: async () => true,
    handleSelectBottomTab: () => {},
    handleDockResizePointerDown: () => {},
    handleZoomIn: () => {},
    handleZoomOut: () => {},
    handleZoomReset: () => {},
    handleToggleWorkflowSettings: () => {},
    handleToggleRightPanel: () => {},
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

  it("renders InspectorPanel when rightPanelHidden is false and node selected", () => {
    const { container, dispose } = renderWithContext({
      rightPanelHidden: () => false,
      selectedNodeId: () => "n1" as any,
      workflowSettingsOpen: () => false,
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

  it("applies workspace-grid--no-inspector class when rightPanelHidden is true", () => {
    const { container, dispose } = renderWithContext({
      rightPanelHidden: () => true,
      selectedNodeId: () => null,
      workflowSettingsOpen: () => false,
    });

    const grid = container.querySelector(".workspace-grid");
    expect(grid).not.toBeNull();
    expect(grid!.classList.contains("workspace-grid--no-inspector")).toBe(true);
    dispose();
  });

  it("does not apply workspace-grid--no-inspector class when panel is visible with selection", () => {
    const { container, dispose } = renderWithContext({
      rightPanelHidden: () => false,
      selectedNodeId: () => "n1" as any,
      workflowSettingsOpen: () => false,
    });

    const grid = container.querySelector(".workspace-grid");
    expect(grid).not.toBeNull();
    expect(grid!.classList.contains("workspace-grid--no-inspector")).toBe(false);
    dispose();
  });

  it("applies workspace-grid--no-inspector when no selection and panel not hidden", () => {
    const { container, dispose } = renderWithContext({
      rightPanelHidden: () => false,
      selectedNodeId: () => null,
      workflowSettingsOpen: () => false,
    });

    const grid = container.querySelector(".workspace-grid");
    expect(grid).not.toBeNull();
    expect(grid!.classList.contains("workspace-grid--no-inspector")).toBe(true);
    dispose();
  });
});
