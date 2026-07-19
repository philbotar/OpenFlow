import { createEffect, createResource, createSignal, type Accessor, type Setter } from "solid-js";
import * as desktop from "../../api";
import { EMPTY_SETTINGS } from "../../constants/providers";
import type {
  AppSettings,
  EdgeId,
  NodeId,
  ScheduleStatus,
  Screen,
  Workflow,
  WorkflowRunState,
} from "../../lib/types";
import { isTextInputTarget, normalizeError, clampDockHeight, viewportHeight } from "../../lib/utils";
import type { AppContextValue } from "../AppContext";
import { createRunStateKernel, createToastApi, selectWorkflow } from "./shared";
import { useAppShell } from "./useAppShell";
import { useChatComposer } from "./useChatComposer";
import { useDock } from "./useDock";
import { useRunSession } from "./useRunSession";
import { useSettings } from "./useSettings";
import { useWorkflowAuthoring } from "./useWorkflowAuthoring";
import { useWorkflowEditor } from "./useWorkflowEditor";
import { useWorkspaceCatalog } from "./useWorkspaceCatalog";

export function useAppProviderState(): AppContextValue {
  const [scheduleStatuses, setScheduleStatuses] = createSignal<ScheduleStatus[]>([]);
  const [localDebugLogPath, setLocalDebugLogPath] = createSignal<string | null>(null);

  let settingsAccessor: Accessor<AppSettings> = () => EMPTY_SETTINGS;
  const toastApi = createToastApi(() => settingsAccessor(), setLocalDebugLogPath);
  const settingsState = useSettings({
    showErrorToast: toastApi.showErrorToast,
    showSuccessToast: toastApi.showSuccessToast,
  });
  settingsAccessor = settingsState.settings;

  let activeWorkflowIdAccessor: Accessor<string | null> = () => null;
  const runKernel = createRunStateKernel(() => activeWorkflowIdAccessor());

  let navigateToScreenRef: (screen: Screen) => void = () => undefined;
  let setScreenRef: Setter<Screen> = (() => "editor") as Setter<Screen>;
  let isCompactViewportAccessor: Accessor<boolean> = () => false;
  let uiZoomAccessor: Accessor<number> = () => 1;
  let applySchemaEditorRef = () => true;
  let closeAddNodePickerRef: () => void = () => undefined;
  let revealProjectsSectionRef: () => void = () => undefined;
  let selectWorkflowRef: (workflow: Workflow) => void = () => undefined;
  const setScreenProxy = ((next: Screen | ((prev: Screen) => Screen)) =>
    setScreenRef(next as never)) as Setter<Screen>;

  const workspace = useWorkspaceCatalog({
    applySchemaEditor: () => applySchemaEditorRef(),
    closeAddNodePicker: () => closeAddNodePickerRef(),
    revealProjectsSection: () => revealProjectsSectionRef(),
    navigateToScreen: (screen) => navigateToScreenRef(screen),
    setScreen: setScreenProxy,
    selectWorkflow: (workflow) => selectWorkflowRef(workflow),
    runState: runKernel.runState,
    backendRunWorkflowId: runKernel.backendRunWorkflowId,
    setBackendRunWorkflowId: runKernel.setBackendRunWorkflowId,
    cacheRunStateForWorkflow: runKernel.cacheRunStateForWorkflow,
    setRunStateByWorkflowId: (updater) =>
      runKernel.setRunStateByWorkflowId(updater as never),
    refreshReadiness: settingsState.refreshReadiness,
    settings: settingsState.settings,
    setSettings: settingsState.setSettings,
    showErrorToast: toastApi.showErrorToast,
    showSuccessToast: toastApi.showSuccessToast,
  });
  activeWorkflowIdAccessor = workspace.activeWorkflowId;

  const workflowEditor = useWorkflowEditor({
    workflows: workspace.workflows,
    setWorkflows: workspace.setWorkflows,
    activeWorkflow: workspace.activeWorkflow,
    runState: runKernel.runState,
    settings: settingsState.settings,
    activeProfileMemo: settingsState.activeProfileMemo,
    isCompactViewport: () => isCompactViewportAccessor(),
    showErrorToast: toastApi.showErrorToast,
    showSuccessToast: toastApi.showSuccessToast,
    clearStatusToast: toastApi.clearStatusToast,
  });
  applySchemaEditorRef = workflowEditor.applySchemaEditor;
  closeAddNodePickerRef = workflowEditor.closeAddNodePicker;
  revealProjectsSectionRef = workflowEditor.revealProjectsSection;

  let startingRunAccessor: Accessor<boolean> = () => false;
  let replayRunIdAccessor: Accessor<string | null> = () => null;
  const chatComposer = useChatComposer({
    activeWorkflow: workspace.activeWorkflow,
    activeWorkflowId: workspace.activeWorkflowId,
    runState: runKernel.runState,
    readiness: settingsState.readiness,
    startingRun: () => startingRunAccessor(),
    replayRunId: () => replayRunIdAccessor(),
    availableSkills: workspace.availableSkills,
    executionCwdForActiveWorkflow: workspace.executionCwdForActiveWorkflow,
    publishBackendRunState: runKernel.publishBackendRunState,
    showErrorToast: toastApi.showErrorToast,
  });

  const dock = useDock({
    executionCwdForActiveWorkflow: workspace.executionCwdForActiveWorkflow,
    isCompactViewport: () => isCompactViewportAccessor(),
    uiZoom: () => uiZoomAccessor(),
  });

  let refreshRunHistoryRef: () => Promise<void> = async () => undefined;
  const runSession = useRunSession({
    activeWorkflow: workspace.activeWorkflow,
    activeWorkflowId: workspace.activeWorkflowId,
    settings: settingsState.settings,
    readiness: settingsState.readiness,
    activeProviderKeyInput: settingsState.activeProviderKeyInput,
    executionCwdForActiveWorkflow: workspace.executionCwdForActiveWorkflow,
    applySchemaEditor: workflowEditor.applySchemaEditor,
    runState: runKernel.runState,
    backendRunWorkflowId: runKernel.backendRunWorkflowId,
    setBackendRunWorkflowId: runKernel.setBackendRunWorkflowId,
    publishBackendRunState: runKernel.publishBackendRunState,
    clearStatusToast: toastApi.clearStatusToast,
    showErrorToast: toastApi.showErrorToast,
    setDockOpen: dock.setDockOpen,
    setBottomTab: dock.setBottomTab,
    setDockHeight: dock.setDockHeight,
    uiZoom: () => uiZoomAccessor(),
    isCompactViewport: () => isCompactViewportAccessor(),
    cacheRunStateForWorkflow: runKernel.cacheRunStateForWorkflow,
    applyRunStateSnapshot: runKernel.applyRunStateSnapshot,
    chatSubmissionFor: chatComposer.chatSubmissionFor,
    resolveChatSubmittedText: chatComposer.resolveChatSubmittedText,
    setChatDraft: chatComposer.setChatDraft,
    setPendingKickoff: chatComposer.setPendingKickoff,
    flushPendingKickoff: chatComposer.flushPendingKickoff,
    handleRefreshRunHistoryRef: () => refreshRunHistoryRef(),
    updateActiveWorkflow: workflowEditor.updateActiveWorkflow,
  });
  refreshRunHistoryRef = runSession.handleRefreshRunHistory;
  startingRunAccessor = runSession.startingRun;
  replayRunIdAccessor = runSession.replayRunId;
  chatComposer.bindStartRunFromChat(runSession.handleStartRunFromChat);

  selectWorkflowRef = (workflow: Workflow) => {
    runSession.setReplayRunId(null);
    selectWorkflow({
      workflow,
      activeWorkflowId: workspace.activeWorkflowId,
      backendRunWorkflowId: runKernel.backendRunWorkflowId,
      runState: runKernel.runState,
      runStateByWorkflowId: runKernel.runStateByWorkflowId,
      cacheRunStateForWorkflow: runKernel.cacheRunStateForWorkflow,
      applyRunStateSnapshot: runKernel.applyRunStateSnapshot,
      setActiveWorkflowId: workspace.setActiveWorkflowId,
      setSelectedNodeId: workflowEditor.setSelectedNodeId,
      setSelectedEdgeId: workflowEditor.setSelectedEdgeId,
      setEditingNodeId: workflowEditor.setEditingNodeId,
      setNodeLabelDraft: workflowEditor.setNodeLabelDraft,
      setSelectedTraceIndex: runSession.setSelectedTraceIndex,
      resetWorkflowChatUi: chatComposer.resetWorkflowChatUi,
    });
  };

  let keyDownActions: (event: KeyboardEvent) => void = () => undefined;
  const handleGlobalKeyDown = (event: KeyboardEvent) => keyDownActions(event);

  let unlistenRunState: (() => void) | null = null;
  let unlistenTerminal: (() => void) | undefined;
  let unlistenSchedule: (() => void) | undefined;
  let setAppUpdateAvailableRef: Setter<boolean> = (() => false) as Setter<boolean>;
  let lastNotifiedPendingApprovalId: string | null = null;
  let lastNotifiedAwaitingKey: string | null = null;
  let lastNotifiedRunError: string | null = null;

  const handleShellCleanup = () => {
    if (unlistenRunState) void unlistenRunState();
    if (unlistenTerminal) void unlistenTerminal();
    unlistenSchedule?.();
    void dock.handleStopTerminal();
  };

  const handleShellMount = async () => {
    try {
      unlistenRunState = await desktop.listenToRunState((nextRunState) => {
        if (nextRunState.active) {
          runSession.setReplayRunId(null);
        }
        runKernel.publishBackendRunState(nextRunState);
        void runSession.refreshContinuableRun();
        const activeId = workspace.activeWorkflowId();
        const backendId = runKernel.backendRunWorkflowId();
        if (activeId !== backendId) {
          return;
        }
        void chatComposer.flushPendingKickoff(nextRunState);
        if (!nextRunState.active) {
          lastNotifiedPendingApprovalId = null;
          lastNotifiedAwaitingKey = null;
        }
        if (nextRunState.pendingApprovals.length > 0) {
          const approval = nextRunState.pendingApprovals[0];
          if (approval.approvalId !== lastNotifiedPendingApprovalId) {
            lastNotifiedPendingApprovalId = approval.approvalId;
            lastNotifiedAwaitingKey = null;
            chatComposer.navigateChatToNode(approval.nodeId);
            dock.focusChatDock();
            toastApi.showInfoToast(`${approval.nodeLabel} needs tool approval`, "run-state");
          }
        } else {
          lastNotifiedPendingApprovalId = null;
          const awaitingIds =
            nextRunState.awaitingNodeIds && nextRunState.awaitingNodeIds.length > 0
              ? nextRunState.awaitingNodeIds
              : nextRunState.awaitingNodeId
                ? [nextRunState.awaitingNodeId]
                : [];
          const awaitingKey = awaitingIds.join("\0");
          const focusId = awaitingIds[0];
          if (focusId && awaitingKey !== lastNotifiedAwaitingKey) {
            lastNotifiedAwaitingKey = awaitingKey;
            const label =
              workspace.activeWorkflow()?.nodes.find((node) => node.id === focusId)?.label ??
              "Node";
            chatComposer.navigateChatToNode(focusId);
            dock.focusChatDock();
            const suffix = awaitingIds.length > 1 ? ` (+${awaitingIds.length - 1} more)` : "";
            toastApi.showInfoToast(`${label} is waiting for input${suffix}`, "run-state");
          } else if (awaitingIds.length === 0) {
            lastNotifiedAwaitingKey = null;
          }
        }
        if (nextRunState.lastError && nextRunState.lastError !== lastNotifiedRunError) {
          lastNotifiedRunError = nextRunState.lastError;
          toastApi.showErrorToast(nextRunState.lastError);
        } else if (!nextRunState.lastError) {
          lastNotifiedRunError = null;
        }
      });
      unlistenTerminal = await desktop.listenToTerminalEvent(dock.handleTerminalEvent);
      desktop
        .listenToScheduleStatuses((statuses) => setScheduleStatuses(statuses))
        .then((unlisten) => {
          unlistenSchedule = unlisten;
        })
        .catch(() => undefined);
      const data = await desktop.bootstrapApp();
      workspace.setAvailableSkills(data.skills ?? []);
      setScheduleStatuses(data.scheduleStatuses ?? []);
      settingsState.setDiscoveredMcp(data.discoveredMcp ?? []);
      void desktop.debugLogPath().then(setLocalDebugLogPath).catch(() => undefined);
      await workspace.initializeWorkspace(
        data.workflows,
        data.agents,
        data.projects ?? [],
        data.settings,
        data.runState,
      );
      runSession.setContinuableRunBackend(data.runContinuable ?? false);
      workspace.setAppReady(true);
      void desktop.checkAppUpdateAvailable().then((result) => {
        setAppUpdateAvailableRef(result.available);
      });
    } catch (error) {
      toastApi.showErrorToast(normalizeError(error));
    }
  };

  let handleOpenWorkflowAuthoringRef: () => Promise<void> = async () => undefined;
  const appShell = useAppShell({
    handleKeyDown: handleGlobalKeyDown,
    handlePointerMove: dock.handleDockResizePointerMove,
    handlePointerEnd: dock.clearDockResizeState,
    onMount: handleShellMount,
    onCleanup: handleShellCleanup,
    handleOpenWorkflowAuthoring: () => handleOpenWorkflowAuthoringRef(),
    closeAddNodePicker: workflowEditor.closeAddNodePicker,
  });

  navigateToScreenRef = appShell.navigateToScreen;
  setScreenRef = appShell.setScreen;
  isCompactViewportAccessor = appShell.isCompactViewport;
  uiZoomAccessor = appShell.uiZoom;
  setAppUpdateAvailableRef = appShell.setAppUpdateAvailable;

  const workflowAuthoring = useWorkflowAuthoring({
    screen: appShell.screen,
    navigateToScreen: appShell.navigateToScreen,
    settings: settingsState.settings,
    activeProviderKeyInput: settingsState.activeProviderKeyInput,
    readiness: settingsState.readiness,
    refreshReadiness: settingsState.refreshReadiness,
    workflows: workspace.workflows,
    setWorkflows: (next) => workspace.setWorkflows(next),
    selectWorkflow: (workflow) => selectWorkflowRef(workflow),
    persistWorkflowAuthoringDraft: workspace.handlePersistWorkflowAuthoringDraft,
    showErrorToast: toastApi.showErrorToast,
    showSuccessToast: toastApi.showSuccessToast,
  });
  handleOpenWorkflowAuthoringRef = () => workflowAuthoring.handleOpenWorkflowAuthoring();

  keyDownActions = (event: KeyboardEvent) => {
    if (event.key === "Escape" && appShell.sidebarDrawerOpen()) {
      appShell.closeSidebarDrawer();
      return;
    }
    const command = event.metaKey || event.ctrlKey;
    if (command && event.key === "0") {
      event.preventDefault();
      appShell.handleZoomReset();
      return;
    }
    if (command && (event.key === "=" || event.key === "+")) {
      event.preventDefault();
      appShell.handleZoomIn();
      return;
    }
    if (command && (event.key === "-" || event.key === "_")) {
      event.preventDefault();
      appShell.handleZoomOut();
      return;
    }
    if (command && event.key.toLowerCase() === "s") {
      event.preventDefault();
      if (appShell.screen() === "agents") {
        void workspace.handleSaveAgents();
      } else if (appShell.screen() === "settings") {
        void settingsState.handleSaveSettings();
      } else {
        void workflowEditor.persistAll();
      }
      return;
    }
    if (command && event.key === "Enter" && appShell.screen() === "editor") {
      event.preventDefault();
      if (runSession.continuableRun() && !runKernel.runState()?.active) {
        void runSession.handleContinueRun();
      } else {
        void runSession.handleRun();
      }
      return;
    }
    if (command && event.key === "." && appShell.screen() === "editor") {
      event.preventDefault();
      void runSession.handleStopRun();
      return;
    }
    if (
      command &&
      event.key.toLowerCase() === "j" &&
      !isTextInputTarget(event.target) &&
      appShell.screen() === "editor"
    ) {
      event.preventDefault();
      workflowEditor.handleToggleRightPanel();
      return;
    }
    if (command && event.key.toLowerCase() === "b" && !isTextInputTarget(event.target)) {
      event.preventDefault();
      workflowEditor.handleToggleLeftPanel();
      return;
    }
    if (
      (event.key === "Delete" || event.key === "Backspace") &&
      !isTextInputTarget(event.target) &&
      appShell.screen() === "editor"
    ) {
      event.preventDefault();
      const edgeId = workflowEditor.selectedEdgeId();
      if (edgeId) {
        workflowEditor.handleDeleteEdge(edgeId);
        return;
      }
      workflowEditor.handleDeleteSelectedNode();
    }
  };

  const [gitRepoCheck] = createResource(workspace.executionCwdForActiveWorkflow, (cwd) =>
    cwd ? desktop.gitIsRepo(cwd) : Promise.resolve(false),
  );
  const gitRepoAvailable = () => gitRepoCheck() === true;

  createEffect(() => {
    const node = workflowEditor.currentNode();
    workflowEditor.setSchemaText(node ? JSON.stringify(node.agent.output_schema, null, 2) : "");
  });

  createEffect(() => {
    const tab = dock.bottomTab();
    const zoom = appShell.uiZoom();
    dock.setDockHeight((current) =>
      clampDockHeight(current, tab, viewportHeight(), appShell.isCompactViewport(), zoom),
    );
  });

  createEffect((prevCompact: boolean | undefined) => {
    const compact = appShell.isCompactViewport();
    if (compact && prevCompact === false) {
      dock.setDockOpen(false);
    }
    return compact;
  });

  createEffect(() => {
    appShell.screen();
    workspace.activeWorkflowId();
    if (appShell.isCompactViewport()) {
      appShell.closeSidebarDrawer();
    }
  });

  createEffect(() => {
    if (!workspace.activeProject() || !gitRepoAvailable()) {
      workflowEditor.setGitPanelOpen(false);
    }
  });

  const handleSelectNode = (nodeId: NodeId | null) => {
    workflowEditor.handleSelectNodeBase(nodeId);
    if (nodeId) {
      dock.focusChatDock();
      chatComposer.navigateChatToNode(nodeId, { forceScroll: true });
    }
  };

  const handleChatInputKeyDown = (event: KeyboardEvent, nodeId: NodeId) => {
    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      void chatComposer.handleSubmitChat(nodeId);
    }
  };

  return {
    workflows: workspace.workflows,
    projects: workspace.projects,
    agents: workspace.agents,
    activeWorkflowId: workspace.activeWorkflowId,
    selectedNodeId: workflowEditor.selectedNodeId,
    selectedEdgeId: workflowEditor.selectedEdgeId,
    screen: appShell.screen,
    settingsSection: appShell.settingsSection,
    settings: settingsState.settings,
    discoveredMcp: settingsState.discoveredMcp,
    refreshDiscoveredMcp: settingsState.refreshDiscoveredMcp,
    runState: runKernel.runState,
    backendRunWorkflowId: runKernel.backendRunWorkflowId,
    readiness: settingsState.readiness,
    refreshReadiness: settingsState.refreshReadiness,
    bottomTab: dock.bottomTab,
    dockOpen: dock.dockOpen,
    dockHeight: dock.dockHeight,
    chatFocusMode: dock.chatFocusMode,
    selectedTraceIndex: runSession.selectedTraceIndex,
    schemaText: workflowEditor.schemaText,
    chatFilterNodeId: chatComposer.chatFilterNodeId,
    pickedLiveNodeId: chatComposer.pickedLiveNodeId,
    chatSegmentOrder: chatComposer.chatSegmentOrder,
    chatFocusNode: chatComposer.chatFocusNode,
    newModelInputByProvider: settingsState.newModelInputByProvider,
    providerKeyInputByProvider: settingsState.providerKeyInputByProvider,
    uiZoom: appShell.uiZoom,
    workflowSettingsOpen: workflowEditor.workflowSettingsOpen,
    inspectorOpen: workflowEditor.inspectorOpen,
    gitPanelOpen: workflowEditor.gitPanelOpen,
    selectedProjectId: workspace.selectedProjectId,
    editingWorkflowId: workspace.editingWorkflowId,
    workflowNameDraft: workspace.workflowNameDraft,
    selectedAgentId: workspace.selectedAgentId,
    editingAgentId: workspace.editingAgentId,
    agentNameDraft: workspace.agentNameDraft,
    editingNodeId: workflowEditor.editingNodeId,
    nodeLabelDraft: workflowEditor.nodeLabelDraft,
    agentSchemaDraft: workspace.agentSchemaDraft,
    addNodePickerOpen: workflowEditor.addNodePickerOpen,
    assignWorkflowPickerProjectId: workspace.assignWorkflowPickerProjectId,
    isMaximized: appShell.isMaximized,
    availableSkills: workspace.availableSkills,
    skillById: workspace.skillById,
    appReady: workspace.appReady,
    appUpdateAvailable: appShell.appUpdateAvailable,
    clearAppUpdateAvailable: appShell.clearAppUpdateAvailable,
    startingRun: runSession.startingRun,
    continuableRun: runSession.continuableRun,
    runHistory: runSession.runHistory,
    runHistoryLoading: runSession.runHistoryLoading,
    replayRunId: runSession.replayRunId,
    themePreference: appShell.themePreference,
    resolvedTheme: appShell.resolvedTheme,
    firstRunOnboardingOpen: appShell.firstRunOnboardingOpen,
    isCompactViewport: appShell.isCompactViewport,
    sidebarDrawerOpen: appShell.sidebarDrawerOpen,
    openSidebarDrawer: appShell.openSidebarDrawer,
    closeSidebarDrawer: appShell.closeSidebarDrawer,
    toggleSidebarDrawer: appShell.toggleSidebarDrawer,
    terminalSessions: dock.terminalSessions,
    activeTerminalSessionId: dock.activeTerminalSessionId,
    terminalStarting: dock.terminalStarting,
    terminalError: dock.terminalError,
    terminalOutputFor: dock.terminalOutputFor,
    scheduleStatuses,
    localDebugLogPath,
    setWorkflowNameDraft: workspace.setWorkflowNameDraft,
    setAgentNameDraft: workspace.setAgentNameDraft,
    setChatFilterNodeId: chatComposer.setChatFilterNodeId,
    setPickedLiveNodeId: chatComposer.setPickedLiveNodeId,
    chatDraft: chatComposer.chatDraft,
    setChatDraft: chatComposer.setChatDraft,
    setNewModelInputByProvider: settingsState.setNewModelInputByProvider,
    setProviderKeyInputByProvider: settingsState.setProviderKeyInputByProvider,
    setNodeLabelDraft: workflowEditor.setNodeLabelDraft,
    setSchemaText: workflowEditor.setSchemaText,
    setSelectedTraceIndex: runSession.setSelectedTraceIndex,
    setSelectedAgentId: workspace.setSelectedAgentId,
    setScreen: appShell.setScreen,
    setSettingsSection: appShell.setSettingsSection,
    navigateToScreen: appShell.navigateToScreen,
    activeWorkflow: workspace.activeWorkflow,
    activeProject: workspace.activeProject,
    gitRepoAvailable,
    independentWorkflows: workspace.independentWorkflows,
    executionCwdForActiveWorkflow: workspace.executionCwdForActiveWorkflow,
    selectedAgent: workspace.selectedAgent,
    canvasGraph: workflowEditor.canvasGraph,
    canvasStatusByNode: workflowEditor.canvasStatusByNode,
    canvasSubagentsByNode: workflowEditor.canvasSubagentsByNode,
    currentNode: workflowEditor.currentNode,
    activeProfileMemo: settingsState.activeProfileMemo,
    providerIdsMemo: settingsState.providerIdsMemo,
    activeProviderKeyInput: settingsState.activeProviderKeyInput,
    selectedTrace: runSession.selectedTrace,
    hasRunTraceMemo: runSession.hasRunTraceMemo,
    currentNodeOutput: workflowEditor.currentNodeOutput,
    chatLayout: chatComposer.chatLayout,
    chatSubmissionFor: chatComposer.chatSubmissionFor,
    canSendChatFor: chatComposer.canSendChatFor,
    composerBusyFor: chatComposer.composerBusyFor,
    setWorkflowNameInputRef: workspace.setWorkflowNameInputRef,
    setAgentNameInputRef: workspace.setAgentNameInputRef,
    handleSwitchWorkflow: workspace.handleSwitchWorkflow,
    handleCreateWorkflow: workspace.handleCreateWorkflow,
    handleOpenAssignWorkflowPicker: workspace.handleOpenAssignWorkflowPicker,
    closeAssignWorkflowPicker: workspace.closeAssignWorkflowPicker,
    workflowsAddableToProject: workspace.workflowsAddableToProject,
    handleCopyWorkflowToProject: workspace.handleCopyWorkflowToProject,
    handleDeleteActiveWorkflow: workspace.handleDeleteActiveWorkflow,
    handleOpenAgents: workspace.handleOpenAgents,
    handleOpenSchedule: workspace.handleOpenSchedule,
    handleSaveWorkflowSchedule: workspace.handleSaveWorkflowSchedule,
    scheduleFromPreset: workspace.scheduleFromPreset,
    scheduleDraftFromSchedule: workspace.scheduleDraftFromSchedule,
    describeWorkflowSchedule: workspace.describeWorkflowSchedule,
    handleAddProject: workspace.handleAddProject,
    handleSelectProject: workspace.handleSelectProject,
    handleToggleProjectExpanded: workspace.handleToggleProjectExpanded,
    isProjectExpanded: workspace.isProjectExpanded,
    workflowsForProject: workspace.workflowsForProject,
    handleCreateAgent: workspace.handleCreateAgent,
    handleSaveAgents: workspace.handleSaveAgents,
    handleAgentSchemaInput: workspace.handleAgentSchemaInput,
    updateSelectedAgent: workspace.updateSelectedAgent,
    handleStartAgentNameEdit: workspace.handleStartAgentNameEdit,
    handleCancelAgentNameEdit: workspace.handleCancelAgentNameEdit,
    handleAgentNameCommit: workspace.handleAgentNameCommit,
    handleAgentNameKeyDown: workspace.handleAgentNameKeyDown,
    handleSaveSettings: settingsState.handleSaveSettings,
    handleAddKnownModel: settingsState.handleAddKnownModel,
    handleRemoveKnownModel: settingsState.handleRemoveKnownModel,
    handleAddReasoningEffortOption: settingsState.handleAddReasoningEffortOption,
    handleRemoveReasoningEffortOption: settingsState.handleRemoveReasoningEffortOption,
    handleApiKeyInput: settingsState.handleApiKeyInput,
    updateSettings: settingsState.updateSettings,
    showErrorToast: toastApi.showErrorToast,
    showSuccessToast: toastApi.showSuccessToast,
    showInfoToast: toastApi.showInfoToast,
    handleSelectNode,
    handleSelectEdge: workflowEditor.handleSelectEdge,
    handleCanvasNodePosition: workflowEditor.handleCanvasNodePosition,
    handleAutoLayoutWorkflow: workflowEditor.handleAutoLayoutWorkflow,
    handleCreateEdge: workflowEditor.handleCreateEdge,
    handleReconnectEdge: workflowEditor.handleReconnectEdge,
    handleDeleteEdge: workflowEditor.handleDeleteEdge,
    handleDeleteSelectedNode: workflowEditor.handleDeleteSelectedNode,
    handleOpenAddNodePicker: workflowEditor.handleOpenAddNodePicker,
    handleAddNode: workflowEditor.handleAddNode,
    closeAddNodePicker: workflowEditor.closeAddNodePicker,
    workflowAuthoringBusy: workflowAuthoring.workflowAuthoringBusy,
    workflowAuthoringThinkingContent: workflowAuthoring.workflowAuthoringThinkingContent,
    workflowAuthoringSessionReady: workflowAuthoring.workflowAuthoringSessionReady,
    workflowAuthoringMessages: workflowAuthoring.workflowAuthoringMessages,
    workflowAuthoringValidation: workflowAuthoring.workflowAuthoringValidation,
    workflowAuthoringDraft: workflowAuthoring.workflowAuthoringDraft,
    handleOpenWorkflowAuthoring: workflowAuthoring.handleOpenWorkflowAuthoring,
    handleCloseWorkflowAuthoring: workflowAuthoring.handleCloseWorkflowAuthoring,
    handleWorkflowAuthoringSend: workflowAuthoring.handleWorkflowAuthoringSend,
    handleApplyWorkflowAuthoringDraft: workflowAuthoring.handleApplyWorkflowAuthoringDraft,
    handleRun: runSession.handleRun,
    handleContinueRun: runSession.handleContinueRun,
    handleStopRun: runSession.handleStopRun,
    handleInterruptNode: runSession.handleInterruptNode,
    handleRetryNode: runSession.handleRetryNode,
    stoppingRun: runSession.stoppingRun,
    handleSetThemePreference: appShell.handleSetThemePreference,
    dismissFirstRunOnboarding: appShell.dismissFirstRunOnboarding,
    handleOnboardingBuildWorkflow: appShell.handleOnboardingBuildWorkflow,
    handleOnboardingSetupProvider: appShell.handleOnboardingSetupProvider,
    handleClearRunTrace: runSession.handleClearRunTrace,
    handleRefreshRunHistory: runSession.handleRefreshRunHistory,
    handleReplayRun: runSession.handleReplayRun,
    handleExitReplay: runSession.handleExitReplay,
    handleResumeDurableRun: runSession.handleResumeDurableRun,
    handleSubmitChat: chatComposer.handleSubmitChat,
    handleRefreshSkills: workspace.handleRefreshSkills,
    searchProjectFileReferences: runSession.searchProjectFileReferences,
    handleToolApproval: runSession.handleToolApproval,
    handleUpdateNodeRuntimeConfig: runSession.handleUpdateNodeRuntimeConfig,
    handleStartNodeLabelEdit: workflowEditor.handleStartNodeLabelEdit,
    handleCancelNodeLabelEdit: workflowEditor.handleCancelNodeLabelEdit,
    handleCommitNodeLabel: workflowEditor.handleCommitNodeLabel,
    handleStartWorkflowNameEdit: workspace.handleStartWorkflowNameEdit,
    handleCancelWorkflowNameEdit: workspace.handleCancelWorkflowNameEdit,
    handleWorkflowNameCommit: workspace.handleWorkflowNameCommit,
    handleWorkflowNameKeyDown: workspace.handleWorkflowNameKeyDown,
    handleChatInputKeyDown,
    handleToggleWorkflowsSection: workflowEditor.handleToggleWorkflowsSection,
    handleToggleProjectsSection: workflowEditor.handleToggleProjectsSection,
    handleToggleWorkflowSettings: workflowEditor.handleToggleWorkflowSettings,
    handleToggleInspector: workflowEditor.handleToggleInspector,
    handleToggleGitPanel: workflowEditor.handleToggleGitPanel,
    handleToggleRightPanel: workflowEditor.handleToggleRightPanel,
    handleToggleLeftPanel: workflowEditor.handleToggleLeftPanel,
    updateActiveWorkflowSettings: workflowEditor.updateActiveWorkflowSettings,
    updateCurrentNode: workflowEditor.updateCurrentNode,
    updateCurrentNodeToolConfig: workflowEditor.updateCurrentNodeToolConfig,
    applySchemaEditor: workflowEditor.applySchemaEditor,
    persistAll: workflowEditor.persistAll,
    handleOpenTerminal: dock.handleOpenTerminal,
    handleSelectTerminalSession: dock.handleSelectTerminalSession,
    handleTerminalInput: dock.handleTerminalInput,
    handleTerminalResize: dock.handleTerminalResize,
    handleStopTerminal: dock.handleStopTerminal,
    handleTerminalEvent: dock.handleTerminalEvent,
    handleSelectBottomTab: dock.handleSelectBottomTab,
    handleToggleChatFocusMode: dock.handleToggleChatFocusMode,
    handleDockResizePointerDown: dock.handleDockResizePointerDown,
    handleZoomIn: appShell.handleZoomIn,
    handleZoomOut: appShell.handleZoomOut,
    handleZoomReset: appShell.handleZoomReset,
    rightPanelHidden: workflowEditor.rightPanelHidden,
    leftPanelHidden: workflowEditor.leftPanelHidden,
    workflowsSectionExpanded: workflowEditor.workflowsSectionExpanded,
    projectsSectionExpanded: workflowEditor.projectsSectionExpanded,
  };
}
