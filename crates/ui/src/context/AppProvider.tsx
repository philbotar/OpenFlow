import {
  createEffect,
  createMemo,
  createSignal,
  onCleanup,
  onMount,
} from "solid-js";
import type { ParentProps } from "solid-js";
import { createStore, reconcile } from "solid-js/store";
import { toast } from "solid-sonner";
import { getAppWindow, openNativeDialog } from "../api";
import { bindRunStateEvents, createUiDesktopOutboundAdapter } from "../port";
import { resolveChatSubmission } from "../lib/chatCommands";
import {
  extractReferencedFilePaths,
  formatSubmissionWithFileReferences,
} from "../lib/fileReferences";
import type {
  AgentDefinition,
  AiProviderKind,
  AppSettings,
  BottomTab,
  EdgeId,
  NodeId,
  Project,
  ProjectFileReference,
  SkillSummary,
  TerminalEvent,
  TerminalStart,
  Workflow,
  WorkflowRunState,
} from "../lib/types";
import {
  activeProfile,
  cloneSettings,
  cloneWorkflow,
  canSendChat,
  canSendIdleRunKickoff,
  createIdleRunState,
  GLOBAL_RUN_ENTRY_NODE_ID,
  isGlobalRunEntryNodeId,
  nextNodePlacement,
  isChatComposerBusy,
  isLiveTranscriptSegment,
  statusForNode,
  nodeOutput,
  prettyJson,
  normalizeRunState,
  projectChatLayout,
  projectWorkflowCanvasGraph,
  projectWorkflowCanvasStatusByNode,
  projectWorkflowCanvasSubagentsByNode,
  providerDisplayOrder,
  removeSelectedNode,
  replaceWorkflow,
  selectedNode,
  withDefaultReasoningFromProfile,
  type WorkflowCanvasGraph,
  type WorkflowCanvasStatusByNode,
  type WorkflowCanvasSubagentsByNode,
} from "../lib/workflow";
import {
  executionCwdForWorkflow,
  findProjectForWorkflow,
  independentWorkflows,
  readExpandedProjectIds,
  workflowsAddableToProject,
  workflowsForProject,
  writeExpandedProjectIds,
} from "../lib/projects";
import {
  clampUiZoom,
  DEFAULT_UI_ZOOM,
  readStoredUiZoom,
  writeStoredUiZoom,
  zoomInUi,
  zoomOutUi,
} from "../lib/uiZoom";
import {
  readStoredRightPanelHidden,
  writeStoredRightPanelHidden,
} from "../lib/panelVisibility";
import {
  readWorkflowsSectionHidden,
  writeWorkflowsSectionHidden,
} from "../lib/workflowsSectionVisibility";
import { resolveCommittedNodeLabel } from "../lib/nodeLabel";
import { EMPTY_SETTINGS } from "../constants/providers";
import {
  clampDockHeight,
  COLLAPSED_DOCK_HEIGHT,
  DEFAULT_DOCK_HEIGHT,
  isTextInputTarget,
  normalizeError,
  shouldCollapseDock,
  STATUS_TOAST_ID,
} from "../lib/utils";
import {
  applyTheme,
  readStoredTheme,
  resolveTheme,
  writeStoredTheme,
  type ThemePreference,
} from "../lib/theme";
import { AppContext } from "./AppContext";

export function AppProvider(props: ParentProps) {
  const desktop = createUiDesktopOutboundAdapter();

  // ── Signals ───────────────────────────────────────────────────────────────
  const [workflows, setWorkflows] = createSignal<Workflow[]>([]);
  const [projects, setProjects] = createSignal<Project[]>([]);
  const [expandedProjectIds, setExpandedProjectIds] = createSignal(
    readExpandedProjectIds(globalThis.localStorage),
  );
  const [selectedProjectId, setSelectedProjectId] = createSignal<string | null>(null);
  const [agents, setAgents] = createSignal<AgentDefinition[]>([]);
  const [activeWorkflowId, setActiveWorkflowId] = createSignal<string | null>(null);
  const [selectedNodeId, setSelectedNodeId] = createSignal<NodeId | null>(null);
  const [selectedEdgeId, setSelectedEdgeId] = createSignal<EdgeId | null>(null);
  const [screen, setScreen] = createSignal<"editor" | "settings" | "agents">("editor");
  const [settings, setSettings] = createSignal<AppSettings>(cloneSettings(EMPTY_SETTINGS));
  // Run state arrives as a freshly-deserialized snapshot on every execution
  // event (including each streaming token). Holding it in a store and applying
  // updates with `reconcile` preserves object identity for unchanged messages,
  // so the conversation <For> reuses rows instead of re-parsing every
  // message's markdown per token.
  const [runStateStore, setRunStateStore] = createStore<{
    current: WorkflowRunState | null;
  }>({ current: null });
  const runState = () => runStateStore.current;
  const setRunState = (next: WorkflowRunState | null) => {
    const normalized = next === null ? null : normalizeRunState(next);
    if (normalized === null || runStateStore.current === null) {
      setRunStateStore("current", normalized);
      return;
    }
    setRunStateStore("current", reconcile(normalized, { key: "id" }));
  };
  const [readiness, setReadiness] = createSignal<{
    ready: boolean;
    provider: string;
    message: string;
    envVar: string;
  } | null>(null);
  const [bottomTab, setBottomTab] = createSignal<BottomTab>("overview");
  const [dockOpen, setDockOpen] = createSignal(true);
  const [dockHeight, setDockHeight] = createSignal(DEFAULT_DOCK_HEIGHT);
  const [chatFocusMode, setChatFocusMode] = createSignal(false);
  const [terminalSession, setTerminalSession] = createSignal<TerminalStart | null>(null);
  const [terminalStarting, setTerminalStarting] = createSignal(false);
  const [terminalError, setTerminalError] = createSignal<string | null>(null);
  const [terminalOutput, setTerminalOutput] = createSignal("");
  const [selectedTraceIndex, setSelectedTraceIndex] = createSignal<number | null>(null);
  const [schemaText, setSchemaText] = createSignal("");
  const [chatDrafts, setChatDrafts] = createStore<Record<string, string>>({});
  const [chatFilterNodeId, setChatFilterNodeId] = createSignal<NodeId | null>(null);
  const [pickedLiveNodeId, setPickedLiveNodeId] = createSignal<NodeId | null>(null);
  const [chatSegmentOrder, setChatSegmentOrder] = createSignal<NodeId[]>([]);
  const [chatFocusNode, setChatFocusNode] = createSignal<{
    nodeId: NodeId;
    tick: number;
  } | null>(null);
  let chatFocusTick = 0;
  const [newModelInputByProvider, setNewModelInputByProvider] = createSignal<
    Record<AiProviderKind, string>
  >({} as Record<AiProviderKind, string>);
  const [providerKeyInputByProvider, setProviderKeyInputByProvider] = createSignal<
    Record<AiProviderKind, string>
  >({} as Record<AiProviderKind, string>);
  const [uiZoom, setUiZoom] = createSignal(readStoredUiZoom(globalThis.localStorage));
  const [rightPanelHidden, setRightPanelHidden] = createSignal(
    readStoredRightPanelHidden(globalThis.localStorage),
  );
  const [workflowsSectionHidden, setWorkflowsSectionHidden] = createSignal(
    readWorkflowsSectionHidden(globalThis.localStorage),
  );
  const workflowsSectionExpanded = createMemo(() => !workflowsSectionHidden());
  const [workflowSettingsOpen, setWorkflowSettingsOpen] = createSignal(false);
  const [editingWorkflowId, setEditingWorkflowId] = createSignal<string | null>(null);
  const [workflowNameDraft, setWorkflowNameDraft] = createSignal("");
  const [selectedAgentId, setSelectedAgentId] = createSignal<string | null>(null);
  const [editingAgentId, setEditingAgentId] = createSignal<string | null>(null);
  const [agentNameDraft, setAgentNameDraft] = createSignal("");
  const [editingNodeId, setEditingNodeId] = createSignal<NodeId | null>(null);
  const [nodeLabelDraft, setNodeLabelDraft] = createSignal("");
  const [agentSchemaDraft, setAgentSchemaDraft] = createSignal("");
  const [addNodePickerOpen, setAddNodePickerOpen] = createSignal(false);
  const [assignWorkflowPickerProjectId, setAssignWorkflowPickerProjectId] =
    createSignal<string | null>(null);
  const [isMaximized, setIsMaximized] = createSignal(false);
  const [availableSkills, setAvailableSkills] = createSignal<SkillSummary[]>([]);
  const [appReady, setAppReady] = createSignal(false);
  const [startingRun, setStartingRun] = createSignal(false);
  const [continuableRun, setContinuableRun] = createSignal(false);
  const [themePreference, setThemePreference] = createSignal<ThemePreference>(
    readStoredTheme(globalThis.localStorage),
  );
  const [shortcutsModalOpen, setShortcutsModalOpen] = createSignal(false);
  const resolvedTheme = createMemo(() => resolveTheme(themePreference()));

  // ── Mutable refs (not signals) ────────────────────────────────────────────
  let workflowNameInput: HTMLInputElement | undefined;
  let agentNameInput: HTMLInputElement | undefined;
  let dockResizeState: { startY: number; startHeight: number } | null = null;

  const setWorkflowNameInputRef = (el: HTMLInputElement | undefined) => {
    workflowNameInput = el;
  };

  const setAgentNameInputRef = (el: HTMLInputElement | undefined) => {
    agentNameInput = el;
  };

  // ── Memos ─────────────────────────────────────────────────────────────────
  const activeWorkflow = createMemo(() =>
    workflows().find((workflow) => workflow.id === activeWorkflowId()),
  );
  const independentWorkflowsMemo = createMemo(() =>
    independentWorkflows(workflows(), projects()),
  );
  const activeProject = createMemo(() => {
    const workflowId = activeWorkflowId();
    if (!workflowId) return undefined;
    const selected = selectedProjectId();
    if (selected) {
      const project = projects().find((item) => item.id === selected);
      if (project?.workflow_ids.includes(workflowId)) return project;
    }
    return findProjectForWorkflow(projects(), workflowId);
  });
  const executionCwdForActiveWorkflow = createMemo(() => {
    const workflowId = activeWorkflowId();
    if (!workflowId) return null;
    return executionCwdForWorkflow(projects(), workflowId, selectedProjectId());
  });
  const selectedAgent = createMemo(
    () => agents().find((agent) => agent.id === selectedAgentId()) ?? null,
  );
  const canvasGraph = createMemo<WorkflowCanvasGraph | null>(
    (previous) => projectWorkflowCanvasGraph(activeWorkflow(), previous),
    null,
  );
  const canvasStatusByNode = createMemo<WorkflowCanvasStatusByNode | null>(
    (previous) => projectWorkflowCanvasStatusByNode(runState(), previous),
    null,
  );
  const canvasSubagentsByNode = createMemo<WorkflowCanvasSubagentsByNode | null>(
    (previous) => projectWorkflowCanvasSubagentsByNode(runState(), previous),
    null,
  );
  const currentNode = createMemo(() => selectedNode(activeWorkflow(), selectedNodeId()));
  const activeProfileMemo = createMemo(() => activeProfile(settings()));
  const providerIdsMemo = createMemo(() => providerDisplayOrder(settings()));
  const activeProviderKeyInput = createMemo(
    () => providerKeyInputByProvider()[settings().active_provider] ?? "",
  );
  const selectedTrace = createMemo(() => {
    const index = selectedTraceIndex();
    if (index === null) return null;
    return runState()?.runTrace[index] ?? null;
  });
  const hasRunTraceMemo = createMemo(() => (runState()?.runTrace.length ?? 0) > 0);
  const currentNodeOutput = createMemo(() => nodeOutput(runState(), selectedNodeId()));
  const chatLayout = createMemo(() =>
    projectChatLayout(
      activeWorkflow(),
      runState(),
      pickedLiveNodeId(),
      chatSegmentOrder(),
    ),
  );
  // Preserve the order nodes first appeared in global chat (append-only).
  createEffect(() => {
    const state = runState();
    if (!state?.active) {
      setChatSegmentOrder([]);
      return;
    }
    const baseLayout = projectChatLayout(activeWorkflow(), state, pickedLiveNodeId());
    const order = chatSegmentOrder();
    let next = order;
    for (const segment of baseLayout.settled) {
      if (!next.includes(segment.nodeId)) {
        next = [...next, segment.nodeId];
      }
    }
    if (next.length !== order.length) {
      setChatSegmentOrder(next);
    }
  });
  // Drop the pick once the node settles (or the run ends) so the next parallel
  // group blocks again until the user picks.
  createEffect(() => {
    const picked = pickedLiveNodeId();
    if (!picked) {
      return;
    }
    const state = runState();
    if (!state || !state.active) {
      setPickedLiveNodeId(null);
      return;
    }
    const status = statusForNode(state.statusByNode, picked);
    if (!isLiveTranscriptSegment(state, { status })) {
      setPickedLiveNodeId(null);
    }
  });
  const chatDraft = (nodeId: NodeId) => chatDrafts[nodeId] ?? "";
  const setChatDraft = (nodeId: NodeId, text: string) => {
    setChatDrafts(nodeId, text);
  };
  const awaitingNodeIdsMemo = createMemo(() => {
    const state = runState();
    if (!state) {
      return [] as string[];
    }
    if (state.awaitingNodeIds && state.awaitingNodeIds.length > 0) {
      return state.awaitingNodeIds;
    }
    return state.awaitingNodeId ? [state.awaitingNodeId] : [];
  });
  const skillIdsMemo = createMemo(
    () => new Set(availableSkills().map((skill) => skill.id)),
  );
  const skillById = createMemo(() => {
    const map = new Map<string, SkillSummary>();
    for (const skill of availableSkills()) {
      map.set(skill.id, skill);
    }
    return map;
  });
  const chatSubmissionFor = (nodeId: NodeId) =>
    resolveChatSubmission(chatDraft(nodeId), skillIdsMemo());
  const resolveChatSubmittedText = async (nodeId: NodeId): Promise<string> => {
    const submission = chatSubmissionFor(nodeId);
    const paths = extractReferencedFilePaths(chatDraft(nodeId));
    if (paths.length === 0) {
      return submission.submittedText;
    }

    const executionCwd = executionCwdForActiveWorkflow();
    if (!executionCwd) {
      throw new Error("File references require a project execution folder.");
    }

    const references = await desktop.readProjectFileReferences(executionCwd, paths);
    return formatSubmissionWithFileReferences(submission.submittedText, references);
  };
  let pendingKickoffText: string | null = null;

  const flushPendingKickoff = async (state: WorkflowRunState) => {
    const text = pendingKickoffText;
    if (!text || !state.active) {
      return;
    }
    const awaitingIds =
      state.awaitingNodeIds && state.awaitingNodeIds.length > 0
        ? state.awaitingNodeIds
        : state.awaitingNodeId
          ? [state.awaitingNodeId]
          : [];
    if (awaitingIds.length === 1) {
      pendingKickoffText = null;
      try {
        const next = await desktop.submitUserInput(awaitingIds[0], text);
        setRunState(next);
      } catch (error) {
        setError(normalizeError(error));
      }
      return;
    }
    if (awaitingIds.length === 0 && !state.active) {
      pendingKickoffText = null;
    }
    if (awaitingIds.length > 1) {
      pendingKickoffText = null;
    }
  };

  const canSendChatFor = (nodeId: NodeId) => {
    if (isGlobalRunEntryNodeId(nodeId)) {
      return canSendIdleRunKickoff(
        runState(),
        readiness()?.ready ?? false,
        !!activeWorkflow(),
        startingRun(),
        chatSubmissionFor(nodeId).submittedText,
      );
    }
    return canSendChat(
      runState(),
      nodeId,
      readiness()?.ready ?? false,
      chatSubmissionFor(nodeId).submittedText,
    );
  };
  const composerBusyFor = (nodeId: NodeId) =>
    isChatComposerBusy(runState(), nodeId);

  // ── Toast helpers ─────────────────────────────────────────────────────────
  const clearStatusToast = () => toast.dismiss(STATUS_TOAST_ID);
  const setError = (text: string) => toast.error(text, { id: STATUS_TOAST_ID });
  const setSuccess = (text: string) => toast.success(text, { id: STATUS_TOAST_ID });

  // ── Zoom ──────────────────────────────────────────────────────────────────
  const applyUiZoom = (nextZoom: number) => {
    const normalized = clampUiZoom(nextZoom);
    setUiZoom(normalized);
    writeStoredUiZoom(globalThis.localStorage, normalized);
    document.documentElement.style.setProperty("--ui-zoom", String(normalized));
  };

  const handleZoomIn = () => applyUiZoom(zoomInUi(uiZoom()));
  const handleZoomOut = () => applyUiZoom(zoomOutUi(uiZoom()));
  const handleZoomReset = () => applyUiZoom(DEFAULT_UI_ZOOM);

  // ── Dock ──────────────────────────────────────────────────────────────────
  const handleSelectBottomTab = (tab: BottomTab) => {
    setBottomTab(tab);
    setDockOpen(true);
    if (tab !== "chat") {
      setChatFocusMode(false);
    }
    setDockHeight((current) => clampDockHeight(current, tab));
  };

  const handleToggleChatFocusMode = () => {
    setBottomTab("chat");
    setDockOpen(true);
    setChatFocusMode((current) => !current);
  };

  const handleDockResizePointerDown = (event: PointerEvent) => {
    if (event.button !== 0) return;
    event.preventDefault();
    dockResizeState = {
      startY: event.clientY,
      startHeight: dockOpen() ? dockHeight() : COLLAPSED_DOCK_HEIGHT,
    };
    document.body.classList.add("is-resizing-dock");
  };

  const handleDockResizePointerMove = (event: PointerEvent) => {
    if (!dockResizeState) return;
    const nextHeight =
      dockResizeState.startHeight + (dockResizeState.startY - event.clientY);
    if (shouldCollapseDock(nextHeight, bottomTab())) {
      setDockOpen(false);
      return;
    }
    setDockOpen(true);
    setDockHeight(clampDockHeight(nextHeight, bottomTab()));
  };

  const clearDockResizeState = () => {
    if (!dockResizeState) return;
    dockResizeState = null;
    document.body.classList.remove("is-resizing-dock");
  };

  const handleOpenTerminal = async (cols: number, rows: number) => {
    if (terminalSession() || terminalStarting()) return;
    setTerminalStarting(true);
    setTerminalError(null);
    try {
      const session = await desktop.startTerminal(
        executionCwdForActiveWorkflow(),
        cols,
        rows,
      );
      setTerminalOutput("");
      setTerminalSession(session);
    } catch (error) {
      setTerminalError(normalizeError(error));
    } finally {
      setTerminalStarting(false);
    }
  };

  const handleTerminalInput = async (data: string) => {
    const session = terminalSession();
    if (!session) return;
    try {
      await desktop.writeTerminal(session.sessionId, data);
    } catch (error) {
      setTerminalError(normalizeError(error));
    }
  };

  const handleTerminalResize = async (cols: number, rows: number) => {
    const session = terminalSession();
    if (!session) return;
    try {
      await desktop.resizeTerminal(session.sessionId, cols, rows);
    } catch (error) {
      setTerminalError(normalizeError(error));
    }
  };

  const handleStopTerminal = async () => {
    const session = terminalSession();
    if (!session) return;
    try {
      await desktop.stopTerminal(session.sessionId);
    } catch (error) {
      setTerminalError(normalizeError(error));
    } finally {
      setTerminalSession(null);
    }
  };

  const handleTerminalEvent = (event: TerminalEvent) => {
    const session = terminalSession();
    if (!session || event.sessionId !== session.sessionId) return;
    const { kind } = event;
    switch (kind.type) {
      case "output":
        setTerminalOutput((current) => current + kind.data);
        return;
      case "error":
        setTerminalError(kind.message);
        return;
      case "exit":
        setTerminalSession(null);
    }
  };

  // ── Provider / settings ───────────────────────────────────────────────────
  const refreshReadiness = async (nextSettings = settings()) => {
    try {
      setReadiness(
        await desktop.resolveProviderReadiness(
          nextSettings,
          providerKeyInputByProvider()[nextSettings.active_provider] ?? null,
        ),
      );
    } catch (error) {
      setError(normalizeError(error));
    }
  };

  const updateSettings = async (mutator: (draft: AppSettings) => void) => {
    const next = cloneSettings(settings());
    mutator(next);
    setSettings(next);
    await refreshReadiness(next);
  };

  const handleApiKeyInput = (key: string) => {
    const providerId = settings().active_provider;
    setProviderKeyInputByProvider((current) => ({ ...current, [providerId]: key }));
    void desktop.resolveProviderReadiness(settings(), key || null)
      .then(setReadiness)
      .catch((error) => setError(normalizeError(error)));
  };

  const handleSaveSettings = async () => {
    const providerId = settings().active_provider;
    const apiKey = activeProviderKeyInput().trim();
    try {
      if (apiKey) {
        await desktop.saveProviderApiKey(providerId, apiKey);
      } else {
        await desktop.deleteProviderApiKey(providerId);
      }
      await desktop.saveSettings(settings());
      await refreshReadiness();
      setSuccess("Settings saved successfully.");
    } catch (error) {
      setError(normalizeError(error));
    }
  };

  const handleAddKnownModel = () => {
    const provider = settings().active_provider;
    const nextName = (newModelInputByProvider()[provider] ?? "").trim();
    if (nextName === "") return;
    void updateSettings((draft) => {
      const profile = activeProfile(draft);
      if (!profile.known_models.includes(nextName)) {
        profile.known_models = [...profile.known_models, nextName];
      }
    });
    setNewModelInputByProvider((current) => ({ ...current, [provider]: "" }));
  };

  const handleRemoveKnownModel = (model: string) => {
    void updateSettings((draft) => {
      const profile = activeProfile(draft);
      profile.known_models = profile.known_models.filter((item) => item !== model);
    });
  };

  // ── Workspace init ────────────────────────────────────────────────────────
  const initializeWorkspace = async (
    initialWorkflows: Workflow[],
    initialAgents: AgentDefinition[],
    initialProjects: Project[],
    initialSettings: AppSettings,
    initialRunState: WorkflowRunState | null,
  ) => {
    let nextWorkflows = initialWorkflows;
    if (nextWorkflows.length === 0) {
      nextWorkflows = [await desktop.createWorkflow("Workflow 1")];
    }
    const firstWorkflow = nextWorkflows[0];
    setWorkflows(nextWorkflows);
    setProjects(initialProjects);
    setAgents(initialAgents);
    setSelectedAgentId(initialAgents[0]?.id ?? null);
    setAgentSchemaDraft(initialAgents[0] ? prettyJson(initialAgents[0].output_schema) : "");
    setActiveWorkflowId(firstWorkflow.id);
    setSelectedNodeId(firstWorkflow.nodes[0]?.id ?? null);
    setSelectedEdgeId(null);
    setEditingNodeId(null);
    setNodeLabelDraft("");
    setRunState(initialRunState ?? createIdleRunState(firstWorkflow));
    setSettings(cloneSettings(initialSettings));
    await refreshReadiness(initialSettings);
  };

  // ── Workflow handlers ─────────────────────────────────────────────────────
  const closeAddNodePicker = () => setAddNodePickerOpen(false);

  const handleSwitchWorkflow = (workflowId: string) => {
    if (!applySchemaEditor()) return;
    const workflow = workflows().find((item) => item.id === workflowId);
    if (!workflow) return;
    closeAddNodePicker();
    setActiveWorkflowId(workflow.id);
    setSelectedNodeId(workflow.nodes[0]?.id ?? null);
    setSelectedEdgeId(null);
    setEditingNodeId(null);
    setNodeLabelDraft("");
    setScreen("editor");
    setSelectedTraceIndex(null);
  };

  const expandProject = (projectId: string) => {
    setExpandedProjectIds((current) => {
      const next = new Set(current);
      next.add(projectId);
      writeExpandedProjectIds(globalThis.localStorage, next);
      return next;
    });
  };

  const handleCreateWorkflow = async (projectId?: string) => {
    try {
      const workflow = await desktop.createWorkflow(`Workflow ${workflows().length + 1}`);
      setWorkflows([...workflows(), workflow]);
      if (!workflowsSectionExpanded()) {
        setWorkflowsSectionHidden(false);
        writeWorkflowsSectionHidden(globalThis.localStorage, false);
      }
      if (projectId) {
        const nextProjects = await desktop.assignWorkflowToProject(projectId, workflow.id);
        setProjects(nextProjects);
        expandProject(projectId);
        setSelectedProjectId(projectId);
      }
      setActiveWorkflowId(workflow.id);
      setSelectedNodeId(workflow.nodes[0]?.id ?? null);
      setSelectedEdgeId(null);
      setEditingNodeId(null);
      setNodeLabelDraft("");
      setScreen("editor");
      setSuccess("Created workflow");
    } catch (error) {
      setError(normalizeError(error));
    }
  };

  const handleOpenAssignWorkflowPicker = (projectId: string) => {
    closeAddNodePicker();
    setSelectedProjectId(projectId);
    expandProject(projectId);
    setAssignWorkflowPickerProjectId(projectId);
  };

  const closeAssignWorkflowPicker = () => setAssignWorkflowPickerProjectId(null);

  const workflowsAddableToProjectMemo = (projectId: string) =>
    workflowsAddableToProject(workflows(), projects(), projectId);

  const handleAssignWorkflowToProject = async (projectId: string, workflowId: string) => {
    try {
      const nextProjects = await desktop.assignWorkflowToProject(projectId, workflowId);
      setProjects(nextProjects);
      expandProject(projectId);
      setSelectedProjectId(projectId);
      closeAssignWorkflowPicker();
      setSuccess("Added workflow to project");
    } catch (error) {
      setError(normalizeError(error));
    }
  };

  const handleAddProject = async () => {
    try {
      const selected = await openNativeDialog({
        directory: true,
        multiple: false,
        title: "Select project folder",
      });
      if (!selected || Array.isArray(selected)) return;
      const project = await desktop.createProjectFromDirectory(selected);
      setProjects([...projects(), project]);
      setSelectedProjectId(project.id);
      setExpandedProjectIds((current) => {
        const next = new Set(current);
        next.add(project.id);
        writeExpandedProjectIds(globalThis.localStorage, next);
        return next;
      });
      setSuccess(`Added project ${project.name}`);
    } catch (error) {
      setError(normalizeError(error));
    }
  };

  const handleSelectProject = (projectId: string) => {
    setSelectedProjectId(projectId);
  };

  const handleToggleProjectExpanded = (projectId: string) => {
    setExpandedProjectIds((current) => {
      const next = new Set(current);
      if (next.has(projectId)) {
        next.delete(projectId);
      } else {
        next.add(projectId);
      }
      writeExpandedProjectIds(globalThis.localStorage, next);
      return next;
    });
  };

  const isProjectExpanded = (projectId: string) => expandedProjectIds().has(projectId);

  const workflowsForProjectMemo = (project: Project) =>
    workflowsForProject(workflows(), project);

  const handleOpenAgents = () => {
    closeAddNodePicker();
    setScreen("agents");
    if (!selectedAgentId() && agents().length > 0) {
      setSelectedAgentId(agents()[0].id);
    }
  };

  // ── Agent handlers ────────────────────────────────────────────────────────
  const updateSelectedAgent = (mutator: (draft: AgentDefinition) => void) => {
    const current = selectedAgent();
    if (!current) return;
    const next = { ...current, output_schema: structuredClone(current.output_schema) };
    mutator(next);
    setAgents(agents().map((agent) => (agent.id === next.id ? next : agent)));
  };

  const handleAgentSchemaInput = (text: string) => {
    setAgentSchemaDraft(text);
    try {
      const parsed = JSON.parse(text);
      updateSelectedAgent((draft) => {
        draft.output_schema = parsed;
      });
      clearStatusToast();
    } catch {
      // preserve draft until save
    }
  };

  const handleCreateAgent = async () => {
    try {
      const agent = await desktop.createAgentDefinition(`Agent ${agents().length + 1}`);
      const defaultModel = activeProfileMemo().default_model;
      if (defaultModel && !agent.model) {
        agent.model = defaultModel;
      }
      setAgents([...agents(), agent]);
      setSelectedAgentId(agent.id);
      setAgentSchemaDraft(prettyJson(agent.output_schema));
      setScreen("agents");
      setSuccess("Created agent");
    } catch (error) {
      setError(normalizeError(error));
    }
  };

  const handleStartAgentNameEdit = (agentId: string, currentName: string) => {
    setEditingAgentId(agentId);
    setAgentNameDraft(currentName);
  };

  const handleCancelAgentNameEdit = () => {
    setEditingAgentId(null);
    setAgentNameDraft("");
  };

  const handleAgentNameCommit = () => {
    const agentId = editingAgentId();
    if (!agentId) return;
    const nextName = agentNameDraft().trim();
    if (nextName !== "") {
      setAgents(
        agents().map((agent) => (agent.id === agentId ? { ...agent, name: nextName } : agent)),
      );
    }
    handleCancelAgentNameEdit();
  };

  const handleAgentNameKeyDown = (event: KeyboardEvent) => {
    if (event.key === "Enter") {
      event.preventDefault();
      handleAgentNameCommit();
      return;
    }
    if (event.key === "Escape") {
      event.preventDefault();
      handleCancelAgentNameEdit();
    }
  };

  const handleSaveAgents = async () => {
    if (selectedAgent()) {
      try {
        const parsed = JSON.parse(agentSchemaDraft());
        updateSelectedAgent((draft) => {
          draft.output_schema = parsed;
        });
      } catch (error) {
        setError(`agent output schema JSON invalid: ${normalizeError(error)}`);
        return;
      }
    }
    try {
      await desktop.saveAgents(agents());
      setSuccess("Saved agents");
    } catch (error) {
      setError(normalizeError(error));
    }
  };

  // ── Node mutation helpers ─────────────────────────────────────────────────
  const updateActiveWorkflow = (mutator: (draft: Workflow) => void) => {
    const workflow = activeWorkflow();
    if (!workflow) return;
    const next = cloneWorkflow(workflow);
    mutator(next);
    setWorkflows(replaceWorkflow(workflows(), next));
  };

  const updateActiveWorkflowSettings = (
    mutator: (settings: Workflow["settings"]) => void,
  ) => {
    updateActiveWorkflow((draft) => {
      mutator(draft.settings);
    });
  };

  const handleToggleWorkflowSettings = () => {
    const opening = !workflowSettingsOpen();
    setWorkflowSettingsOpen((open) => !open);
    if (opening) {
      setRightPanelHidden(false);
      writeStoredRightPanelHidden(globalThis.localStorage, false);
    }
  };

  const handleToggleRightPanel = () => {
    const currentlyHidden = rightPanelHidden();
    if (currentlyHidden) {
      setRightPanelHidden(false);
      writeStoredRightPanelHidden(globalThis.localStorage, false);
    } else {
      setRightPanelHidden(true);
      writeStoredRightPanelHidden(globalThis.localStorage, true);
    }
  };

  const handleToggleWorkflowsSection = () => {
    const next = !workflowsSectionExpanded();
    setWorkflowsSectionHidden(!next);
    writeWorkflowsSectionHidden(globalThis.localStorage, !next);
  };

  const updateCurrentNode = (mutator: (node: Workflow["nodes"][number]) => void) => {
    const nodeId = selectedNodeId();
    if (!nodeId) return;
    updateActiveWorkflow((draft) => {
      const nextNode = draft.nodes.find((item) => item.id === nodeId);
      if (nextNode) mutator(nextNode);
    });
  };

  const updateCurrentNodeToolConfig = (
    mutator: (tools: Workflow["nodes"][number]["agent"]["tools"]) => void,
  ) => {
    updateCurrentNode((node) => mutator(node.agent.tools));
  };

  const setToolEnabled = (
    tools: { catalog: { tools: { name: string }[] } },
    toolName: string,
    enabled: boolean,
  ) => {
    const nextTools = tools.catalog.tools.filter((tool) => tool.name !== toolName);
    tools.catalog.tools = enabled
      ? [...nextTools, { name: toolName }].sort((l, r) => l.name.localeCompare(r.name))
      : nextTools;
  };

  const applySchemaEditor = () => {
    const nodeId = selectedNodeId();
    const workflow = activeWorkflow();
    if (!nodeId || !workflow) return true;
    try {
      const parsed = JSON.parse(schemaText());
      updateActiveWorkflow((draft) => {
        const node = draft.nodes.find((item) => item.id === nodeId);
        if (node) node.agent.output_schema = parsed;
      });
      clearStatusToast();
      return true;
    } catch (error) {
      setError(`output schema JSON invalid: ${normalizeError(error)}`);
      return false;
    }
  };

  const persistAll = async (successText = "Saved") => {
    if (!applySchemaEditor()) return false;
    try {
      await desktop.saveWorkflows(workflows());
      await desktop.saveSettings(settings());
      setSuccess(successText);
      return true;
    } catch (error) {
      setError(normalizeError(error));
      return false;
    }
  };

  // ── Canvas handlers ───────────────────────────────────────────────────────
  const handleOpenAddNodePicker = () => {
    if (!activeWorkflow()) return;
    setScreen("editor");
    setSelectedEdgeId(null);
    setAddNodePickerOpen(true);
  };

  const handleAddNode = async (agentId: string | null) => {
    const workflow = activeWorkflow();
    if (!workflow) return;
    const placement = nextNodePlacement(workflow);
    try {
      const node = await desktop.createAgentNode(
        placement.index,
        placement.x,
        placement.y,
        agentId,
      );
      const profile = activeProfileMemo();
      let nextAgent = withDefaultReasoningFromProfile(node.agent, profile);
      if (profile.default_model && !nextAgent.model) {
        nextAgent = { ...nextAgent, model: profile.default_model };
      }
      const nextNode = { ...node, agent: nextAgent };
      updateActiveWorkflow((draft) => {
        draft.nodes.push(nextNode);
      });
      closeAddNodePicker();
      setSelectedNodeId(nextNode.id);
      setSelectedEdgeId(null);
      setEditingNodeId(null);
      setNodeLabelDraft("");
      setSuccess(agentId ? "Added saved agent to workflow" : "Added node");
    } catch (error) {
      setError(normalizeError(error));
    }
  };

  const handleDeleteSelectedNode = () => {
    const workflow = activeWorkflow();
    const nodeId = selectedNodeId();
    if (!workflow || !nodeId) return;
    const next = removeSelectedNode(workflow, nodeId);
    setWorkflows(replaceWorkflow(workflows(), next));
    setSelectedNodeId(next.nodes[0]?.id ?? null);
    setSelectedEdgeId(null);
    setEditingNodeId(null);
    setNodeLabelDraft("");
  };

  const handleDeleteEdge = (edgeId: EdgeId) => {
    updateActiveWorkflow((draft) => {
      draft.edges = draft.edges.filter((edge) => edge.id !== edgeId);
    });
    if (selectedEdgeId() === edgeId) setSelectedEdgeId(null);
  };

  const handleValidate = async () => {
    const workflow = activeWorkflow();
    if (!workflow || !applySchemaEditor()) return;
    try {
      const summary = await desktop.validateWorkflow(activeWorkflow()!);
      setSuccess(
        `Valid DAG · ${summary.layerCount} layer${summary.layerCount === 1 ? "" : "s"}`,
      );
    } catch (error) {
      setError(normalizeError(error));
    }
  };

  const [stoppingRun, setStoppingRun] = createSignal(false);

  const beginRunSession = (nextRunState: WorkflowRunState) => {
    setRunState(nextRunState);
    setContinuableRun(false);
    setSelectedTraceIndex(null);
    setDockOpen(true);
    setBottomTab("chat");
    setDockHeight((current) => clampDockHeight(current, "chat"));
    clearStatusToast();
  };

  const refreshContinuableRun = async () => {
    try {
      setContinuableRun(await desktop.isRunContinuable());
    } catch {
      setContinuableRun(false);
    }
  };

  const handleRun = async () => {
    const workflow = activeWorkflow();
    if (!workflow || !applySchemaEditor() || stoppingRun() || startingRun()) return;
    setStartingRun(true);
    try {
      const nextRunState = await desktop.startRun(
        activeWorkflow()!,
        settings(),
        executionCwdForActiveWorkflow(),
        activeProviderKeyInput() || null,
        null,
      );
      beginRunSession(nextRunState);
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setStartingRun(false);
    }
  };

  const handleStartRunFromChat = async (nodeId: NodeId) => {
    const workflow = activeWorkflow();
    if (
      !workflow ||
      !isGlobalRunEntryNodeId(nodeId) ||
      !applySchemaEditor() ||
      stoppingRun() ||
      startingRun()
    ) {
      return;
    }
    const submission = chatSubmissionFor(nodeId);
    let submittedText = submission.submittedText;
    if (
      !canSendIdleRunKickoff(
        runState(),
        readiness()?.ready ?? false,
        true,
        startingRun(),
        submission.submittedText,
      )
    ) {
      return;
    }
    try {
      submittedText = await resolveChatSubmittedText(nodeId);
    } catch (error) {
      setError(normalizeError(error));
      return;
    }
    setStartingRun(true);
    pendingKickoffText = submittedText;
    try {
      const nextRunState = await desktop.startRun(
        workflow,
        settings(),
        executionCwdForActiveWorkflow(),
        activeProviderKeyInput() || null,
        submittedText,
      );
      setChatDraft(nodeId, "");
      beginRunSession(nextRunState);
      await flushPendingKickoff(nextRunState);
    } catch (error) {
      pendingKickoffText = null;
      setError(normalizeError(error));
    } finally {
      setStartingRun(false);
    }
  };

  const handleContinueRun = async () => {
    const workflow = activeWorkflow();
    if (!workflow || !continuableRun() || stoppingRun() || startingRun()) return;
    setStartingRun(true);
    try {
      const nextRunState = await desktop.continueRun(
        activeWorkflow()!,
        settings(),
        activeProviderKeyInput() || null,
      );
      beginRunSession(nextRunState);
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setStartingRun(false);
    }
  };

  const handleSetThemePreference = (preference: ThemePreference) => {
    setThemePreference(preference);
    writeStoredTheme(globalThis.localStorage, preference);
    applyTheme(resolveTheme(preference));
  };

  const openShortcutsModal = () => setShortcutsModalOpen(true);
  const closeShortcutsModal = () => setShortcutsModalOpen(false);

  const handleStopRun = async () => {
    if (!runState()?.active || stoppingRun()) return;
    setStoppingRun(true);
    try {
      const nextRunState = await desktop.stopRun();
      setRunState(nextRunState);
      await refreshContinuableRun();
      clearStatusToast();
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setStoppingRun(false);
    }
  };

  const handleInterruptNode = async (nodeId: NodeId) => {
    if (!runState()?.active) return;
    try {
      await desktop.interruptNode(nodeId);
    } catch (error) {
      setError(normalizeError(error));
    }
  };

  const handleRetryNode = async (nodeId: NodeId) => {
    if (!runState()?.active) return;
    try {
      await desktop.retryNode(nodeId);
    } catch (error) {
      setError(normalizeError(error));
    }
  };

  const handleClearRunTrace = async () => {
    try {
      const nextRunState = await desktop.clearRunTrace();
      if (nextRunState) setRunState(nextRunState);
      setContinuableRun(false);
      setSelectedTraceIndex(null);
    } catch (error) {
      setError(normalizeError(error));
    }
  };

  const handleSubmitChat = async (nodeId: NodeId) => {
    if (!canSendChatFor(nodeId)) return;
    if (isGlobalRunEntryNodeId(nodeId)) {
      await handleStartRunFromChat(nodeId);
      return;
    }
    try {
      const submittedText = await resolveChatSubmittedText(nodeId);
      const nextRunState = await desktop.submitUserInput(nodeId, submittedText);
      setRunState(nextRunState);
      setChatDraft(nodeId, "");
    } catch (error) {
      setError(normalizeError(error));
    }
  };

  const handleRefreshSkills = async () => {
    try {
      setAvailableSkills(await desktop.listSkills());
    } catch (error) {
      setError(normalizeError(error));
    }
  };

  const searchProjectFileReferences = async (
    query: string,
  ): Promise<ProjectFileReference[]> => {
    const executionCwd = executionCwdForActiveWorkflow();
    if (!executionCwd) {
      return [];
    }
    return desktop.listProjectFileReferences(executionCwd, query, 30);
  };

  const handleToolApproval = async (approvalId: string, allow: boolean) => {
    try {
      const nextRunState = await desktop.submitToolApproval(approvalId, allow);
      setRunState(nextRunState);
    } catch (error) {
      setError(normalizeError(error));
    }
  };

  const focusChatNode = (nodeId: NodeId) => {
    chatFocusTick += 1;
    setChatFocusNode({ nodeId, tick: chatFocusTick });
  };

  const handleSelectNode = (nodeId: NodeId | null) => {
    setSelectedEdgeId(null);
    setSelectedNodeId(nodeId);
    setEditingNodeId(null);
    setNodeLabelDraft("");
    if (nodeId && bottomTab() === "chat") {
      focusChatNode(nodeId);
    }
  };

  const handleSelectEdge = (edgeId: EdgeId | null) => {
    setSelectedEdgeId(edgeId);
    if (edgeId) setSelectedNodeId(null);
    setEditingNodeId(null);
    setNodeLabelDraft("");
  };

  const handleCanvasNodePosition = (nodeId: NodeId, x: number, y: number) => {
    updateActiveWorkflow((draft) => {
      const node = draft.nodes.find((item) => item.id === nodeId);
      if (node) {
        node.position.x = x;
        node.position.y = y;
      }
    });
  };

  const handleCreateEdge = (from: NodeId, to: NodeId) => {
    if (from === to) return;
    const edgeId = crypto.randomUUID();
    let created = false;
    updateActiveWorkflow((draft) => {
      const duplicate = draft.edges.some((edge) => edge.from === from && edge.to === to);
      if (duplicate) return;
      draft.edges.push({ id: edgeId, from, to });
      created = true;
    });
    if (created) {
      setSelectedNodeId(null);
      setSelectedEdgeId(edgeId);
      setEditingNodeId(null);
      setNodeLabelDraft("");
    }
  };

  const handleReconnectEdge = (edgeId: EdgeId, from: NodeId, to: NodeId) => {
    if (from === to) return;
    let reconnected = false;
    updateActiveWorkflow((draft) => {
      const duplicate = draft.edges.some(
        (edge) => edge.id !== edgeId && edge.from === from && edge.to === to,
      );
      if (duplicate) return;
      const edge = draft.edges.find((item) => item.id === edgeId);
      if (!edge) return;
      edge.from = from;
      edge.to = to;
      reconnected = true;
    });
    if (reconnected) {
      setSelectedNodeId(null);
      setSelectedEdgeId(edgeId);
      setEditingNodeId(null);
      setNodeLabelDraft("");
    }
  };

  // ── Name / label edit handlers ────────────────────────────────────────────
  const handleStartWorkflowNameEdit = (workflowId: string, currentName: string) => {
    setEditingWorkflowId(workflowId);
    setWorkflowNameDraft(currentName);
  };

  const handleCancelWorkflowNameEdit = () => {
    setEditingWorkflowId(null);
    setWorkflowNameDraft("");
  };

  const handleWorkflowNameCommit = () => {
    const workflowId = editingWorkflowId();
    if (!workflowId) return;
    const nextName = workflowNameDraft().trim();
    if (nextName !== "") {
      setWorkflows(
        workflows().map((workflow) =>
          workflow.id === workflowId ? { ...workflow, name: nextName } : workflow,
        ),
      );
    }
    handleCancelWorkflowNameEdit();
  };

  const handleWorkflowNameKeyDown = (event: KeyboardEvent) => {
    if (event.key === "Enter") {
      event.preventDefault();
      handleWorkflowNameCommit();
      return;
    }
    if (event.key === "Escape") {
      event.preventDefault();
      handleCancelWorkflowNameEdit();
    }
  };

  const handleStartNodeLabelEdit = (nodeId: NodeId, currentLabel: string) => {
    setEditingNodeId(nodeId);
    setNodeLabelDraft(currentLabel);
  };

  const handleCancelNodeLabelEdit = () => {
    setEditingNodeId(null);
    setNodeLabelDraft("");
  };

  const handleCommitNodeLabel = () => {
    const nodeId = editingNodeId();
    if (!nodeId) return;
    const currentLabel = currentNode()?.label ?? "";
    const nextLabel = resolveCommittedNodeLabel(currentLabel, nodeLabelDraft());
    updateActiveWorkflow((draft) => {
      const nextNode = draft.nodes.find((item) => item.id === nodeId);
      if (nextNode) nextNode.label = nextLabel;
    });
    handleCancelNodeLabelEdit();
  };

  const handleChatInputKeyDown = (event: KeyboardEvent, nodeId: NodeId) => {
    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      void handleSubmitChat(nodeId);
    }
  };

  // ── Global keyboard handler ───────────────────────────────────────────────
  function handleKeyDown(event: KeyboardEvent) {
    const command = event.metaKey || event.ctrlKey;
    if (command && event.key === "0") {
      event.preventDefault();
      handleZoomReset();
      return;
    }
    if (command && (event.key === "=" || event.key === "+")) {
      event.preventDefault();
      handleZoomIn();
      return;
    }
    if (command && (event.key === "-" || event.key === "_")) {
      event.preventDefault();
      handleZoomOut();
      return;
    }
    if (command && event.key.toLowerCase() === "s") {
      event.preventDefault();
      if (screen() === "agents") {
        void handleSaveAgents();
      } else if (screen() === "settings") {
        void handleSaveSettings();
      } else {
        void persistAll();
      }
      return;
    }
    if (command && event.key === "Enter") {
      event.preventDefault();
      if (continuableRun() && !runState()?.active) {
        void handleContinueRun();
      } else {
        void handleRun();
      }
      return;
    }
    if (command && event.key === ".") {
      event.preventDefault();
      void handleStopRun();
      return;
    }
    if (event.key === "?" && !isTextInputTarget(event.target)) {
      event.preventDefault();
      openShortcutsModal();
      return;
    }
    if (command && event.key.toLowerCase() === "j" && !isTextInputTarget(event.target) && screen() === "editor") {
      event.preventDefault();
      handleToggleRightPanel();
      return;
    }
    if (
      (event.key === "Delete" || event.key === "Backspace") &&
      !isTextInputTarget(event.target) &&
      screen() === "editor"
    ) {
      event.preventDefault();
      const edgeId = selectedEdgeId();
      if (edgeId) {
        handleDeleteEdge(edgeId);
        return;
      }
      handleDeleteSelectedNode();
    }
  }

  // ── Effects ───────────────────────────────────────────────────────────────
  createEffect(() => {
    const providerId = settings().active_provider;
    void desktop.loadProviderApiKey(providerId)
      .then((apiKey) => {
        if (settings().active_provider !== providerId) return;
        const nextKey = apiKey ?? "";
        setProviderKeyInputByProvider((current) => ({ ...current, [providerId]: nextKey }));
        return desktop.resolveProviderReadiness(settings(), nextKey || null);
      })
      .then((nextReadiness) => {
        if (nextReadiness) setReadiness(nextReadiness);
      })
      .catch((error) => setError(normalizeError(error)));
  });

  createEffect(() => {
    applyTheme(resolvedTheme());
  });

  createEffect(() => {
    const node = currentNode();
    setSchemaText(node ? prettyJson(node.agent.output_schema) : "");
  });

  createEffect(() => {
    const agent = selectedAgent();
    setAgentSchemaDraft(agent ? prettyJson(agent.output_schema) : "");
  });

  createEffect(() => {
    const workflowId = editingWorkflowId();
    if (!workflowId) return;
    queueMicrotask(() => {
      if (editingWorkflowId() !== workflowId || !workflowNameInput) return;
      workflowNameInput.focus();
      workflowNameInput.setSelectionRange(0, workflowNameInput.value.length);
    });
  });

  createEffect(() => {
    const agentId = editingAgentId();
    if (!agentId) return;
    queueMicrotask(() => {
      if (editingAgentId() !== agentId || !agentNameInput) return;
      agentNameInput.focus();
      agentNameInput.setSelectionRange(0, agentNameInput.value.length);
    });
  });

  createEffect(() => {
    const tab = bottomTab();
    setDockHeight((current) => clampDockHeight(current, tab));
  });

  // ── Mount ─────────────────────────────────────────────────────────────────
  onMount(async () => {
    let unlisten: (() => void) | null = null;
    let unlistenTerminal: (() => void) | undefined;
    let unlistenMaximized: (() => void) | null = null;

    window.addEventListener("keydown", handleKeyDown);
    window.addEventListener("pointermove", handleDockResizePointerMove);
    window.addEventListener("pointerup", clearDockResizeState);
    window.addEventListener("pointercancel", clearDockResizeState);

    onCleanup(() => {
      window.removeEventListener("keydown", handleKeyDown);
      window.removeEventListener("pointermove", handleDockResizePointerMove);
      window.removeEventListener("pointerup", clearDockResizeState);
      window.removeEventListener("pointercancel", clearDockResizeState);
      document.body.classList.remove("is-resizing-dock");
      if (unlisten) void unlisten();
      if (unlistenTerminal) void unlistenTerminal();
      void handleStopTerminal();
      if (unlistenMaximized) void unlistenMaximized();
    });

    applyUiZoom(uiZoom());

    try {
      const appWindow = getAppWindow();
      const initialMaximized = await appWindow.isMaximized();
      setIsMaximized(initialMaximized);
      unlistenMaximized = await appWindow.onResized(() => {
        void appWindow.isMaximized().then(setIsMaximized);
      });
      unlisten = await bindRunStateEvents(
        {
          handleRunStateUpdate: (nextRunState) => {
            setRunState(nextRunState);
            void flushPendingKickoff(nextRunState);
            if (nextRunState.pendingApprovals.length > 0) {
              const approval = nextRunState.pendingApprovals[0];
              focusChatNode(approval.nodeId);
              setDockOpen(true);
              setBottomTab("chat");
              setDockHeight((current) => clampDockHeight(current, "chat"));
              toast(`${approval.nodeLabel} needs tool approval`, { id: STATUS_TOAST_ID });
            } else {
              const awaitingIds =
                nextRunState.awaitingNodeIds && nextRunState.awaitingNodeIds.length > 0
                  ? nextRunState.awaitingNodeIds
                  : nextRunState.awaitingNodeId
                    ? [nextRunState.awaitingNodeId]
                    : [];
              const focusId = awaitingIds[0];
              if (focusId) {
                const label =
                  activeWorkflow()?.nodes.find((n) => n.id === focusId)?.label ?? "Node";
                focusChatNode(focusId);
                setDockOpen(true);
                setBottomTab("chat");
                setDockHeight((current) => clampDockHeight(current, "chat"));
                const suffix =
                  awaitingIds.length > 1 ? ` (+${awaitingIds.length - 1} more)` : "";
                toast(`${label} is waiting for input${suffix}`, { id: STATUS_TOAST_ID });
              }
            }
            if (nextRunState.lastError) {
              setError(nextRunState.lastError);
            }
          },
        },
        desktop,
      );
      unlistenTerminal = await desktop.listenToTerminalEvent(handleTerminalEvent);
      const data = await desktop.bootstrapApp();
      setAvailableSkills(data.skills ?? []);
      await initializeWorkspace(
        data.workflows,
        data.agents,
        data.projects ?? [],
        data.settings,
        data.runState,
      );
      setContinuableRun(data.runContinuable ?? false);
      setAppReady(true);
    } catch (error) {
      setError(normalizeError(error));
    }

    const media = globalThis.matchMedia?.("(prefers-color-scheme: dark)");
    const handleSystemThemeChange = () => {
      if (themePreference() === "system") {
        applyTheme(resolveTheme("system"));
      }
    };
    media?.addEventListener("change", handleSystemThemeChange);
    onCleanup(() => media?.removeEventListener("change", handleSystemThemeChange));
  });

  // ── Context value ─────────────────────────────────────────────────────────
  const value = {
    // Signals
    workflows,
    projects,
    agents,
    activeWorkflowId,
    selectedNodeId,
    selectedEdgeId,
    screen,
    settings,
    runState,
    readiness,
    bottomTab,
    dockOpen,
    dockHeight,
    chatFocusMode,
    selectedTraceIndex,
    schemaText,
    chatFilterNodeId,
    pickedLiveNodeId,
    chatFocusNode,
    newModelInputByProvider,
    providerKeyInputByProvider,
    uiZoom,
    workflowSettingsOpen,
    selectedProjectId,
    editingWorkflowId,
    workflowNameDraft,
    selectedAgentId,
    editingAgentId,
    agentNameDraft,
    editingNodeId,
    nodeLabelDraft,
    agentSchemaDraft,
    addNodePickerOpen,
    assignWorkflowPickerProjectId,
    isMaximized,
    availableSkills,
    skillById,
    appReady,
    startingRun,
    continuableRun,
    themePreference,
    resolvedTheme,
    shortcutsModalOpen,
    terminalSession,
    terminalStarting,
    terminalError,
    terminalOutput,
    // Setters
    setWorkflowNameDraft,
    setAgentNameDraft,
    setChatFilterNodeId,
    setPickedLiveNodeId,
    chatDraft,
    setChatDraft,
    setNewModelInputByProvider,
    setProviderKeyInputByProvider,
    setNodeLabelDraft,
    setSchemaText,
    setSelectedTraceIndex,
    setSelectedAgentId,
    setScreen,
    // Memos
    activeWorkflow,
    activeProject,
    independentWorkflows: independentWorkflowsMemo,
    executionCwdForActiveWorkflow,
    selectedAgent,
    canvasGraph,
    canvasStatusByNode,
    canvasSubagentsByNode,
    currentNode,
    activeProfileMemo,
    providerIdsMemo,
    activeProviderKeyInput,
    selectedTrace,
    hasRunTraceMemo,
    currentNodeOutput,
    chatLayout,
    chatSubmissionFor,
    canSendChatFor,
    composerBusyFor,
    // Ref setters
    setWorkflowNameInputRef,
    setAgentNameInputRef,
    // Handlers
    handleSwitchWorkflow,
    handleCreateWorkflow,
    handleOpenAssignWorkflowPicker,
    closeAssignWorkflowPicker,
    workflowsAddableToProject: workflowsAddableToProjectMemo,
    handleAssignWorkflowToProject,
    handleOpenAgents,
    handleAddProject,
    handleSelectProject,
    handleToggleProjectExpanded,
    isProjectExpanded,
    workflowsForProject: workflowsForProjectMemo,
    handleCreateAgent,
    handleSaveAgents,
    handleAgentSchemaInput,
    updateSelectedAgent,
    handleStartAgentNameEdit,
    handleCancelAgentNameEdit,
    handleAgentNameCommit,
    handleAgentNameKeyDown,
    handleSaveSettings,
    handleAddKnownModel,
    handleRemoveKnownModel,
    handleApiKeyInput,
    updateSettings,
    handleSelectNode,
    handleSelectEdge,
    handleCanvasNodePosition,
    handleCreateEdge,
    handleReconnectEdge,
    handleDeleteEdge,
    handleDeleteSelectedNode,
    handleOpenAddNodePicker,
    handleAddNode,
    closeAddNodePicker,
    handleValidate,
    handleRun,
    handleContinueRun,
    handleStopRun,
    handleInterruptNode,
    handleRetryNode,
    stoppingRun,
    handleSetThemePreference,
    openShortcutsModal,
    closeShortcutsModal,
    handleClearRunTrace,
    handleSubmitChat,
    handleRefreshSkills,
    searchProjectFileReferences,
    handleToolApproval,
    handleStartNodeLabelEdit,
    handleCancelNodeLabelEdit,
    handleCommitNodeLabel,
    handleStartWorkflowNameEdit,
    handleCancelWorkflowNameEdit,
    handleWorkflowNameCommit,
    handleWorkflowNameKeyDown,
    handleChatInputKeyDown,
    updateCurrentNode,
    updateCurrentNodeToolConfig,
    setToolEnabled,
    applySchemaEditor,
    persistAll,
    handleOpenTerminal,
    handleTerminalInput,
    handleTerminalResize,
    handleStopTerminal,
    handleTerminalEvent,
    handleSelectBottomTab,
    handleToggleChatFocusMode,
    handleDockResizePointerDown,
    handleZoomIn,
    handleZoomOut,
    handleZoomReset,
    handleToggleWorkflowSettings,
    updateActiveWorkflowSettings,
    rightPanelHidden,
    handleToggleRightPanel,
    workflowsSectionExpanded,
    handleToggleWorkflowsSection,
  };

  return <AppContext.Provider value={value}>{props.children}</AppContext.Provider>;
}
