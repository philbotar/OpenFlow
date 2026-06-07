import {
  createEffect,
  createMemo,
  createSignal,
  onCleanup,
  onMount,
} from "solid-js";
import type { ParentProps } from "solid-js";
import { toast } from "solid-sonner";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { bindRunStateEvents, createUiDesktopOutboundAdapter } from "../adapters";
import { resolveChatSubmission } from "../lib/chatCommands";
import type {
  AgentDefinition,
  AiProviderKind,
  AppSettings,
  BottomTab,
  EdgeId,
  NodeId,
  SkillSummary,
  Workflow,
  WorkflowRunState,
} from "../lib/types";
import {
  activeProfile,
  cloneSettings,
  cloneWorkflow,
  createIdleRunState,
  nextNodePlacement,
  isChatComposerBusy,
  nodeOutput,
  prettyJson,
  projectWorkflowCanvasGraph,
  projectWorkflowCanvasStatusByNode,
  projectWorkflowCanvasSubagentsByNode,
  providerDisplayOrder,
  removeSelectedNode,
  replaceWorkflow,
  selectedNode,
  type WorkflowCanvasGraph,
  type WorkflowCanvasStatusByNode,
  type WorkflowCanvasSubagentsByNode,
} from "../lib/workflow";
import {
  clampUiZoom,
  DEFAULT_UI_ZOOM,
  readStoredUiZoom,
  writeStoredUiZoom,
  zoomInUi,
  zoomOutUi,
} from "../lib/uiZoom";
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
import { AppContext } from "./AppContext";

export function AppProvider(props: ParentProps) {
  const desktop = createUiDesktopOutboundAdapter();

  // ── Signals ───────────────────────────────────────────────────────────────
  const [workflows, setWorkflows] = createSignal<Workflow[]>([]);
  const [agents, setAgents] = createSignal<AgentDefinition[]>([]);
  const [activeWorkflowId, setActiveWorkflowId] = createSignal<string | null>(null);
  const [selectedNodeId, setSelectedNodeId] = createSignal<NodeId | null>(null);
  const [selectedEdgeId, setSelectedEdgeId] = createSignal<EdgeId | null>(null);
  const [screen, setScreen] = createSignal<"editor" | "settings" | "agents">("editor");
  const [settings, setSettings] = createSignal<AppSettings>(cloneSettings(EMPTY_SETTINGS));
  const [runState, setRunState] = createSignal<WorkflowRunState | null>(null);
  const [readiness, setReadiness] = createSignal<{
    ready: boolean;
    provider: string;
    message: string;
    envVar: string;
  } | null>(null);
  const [bottomTab, setBottomTab] = createSignal<BottomTab>("overview");
  const [dockOpen, setDockOpen] = createSignal(true);
  const [dockHeight, setDockHeight] = createSignal(DEFAULT_DOCK_HEIGHT);
  const [selectedTraceIndex, setSelectedTraceIndex] = createSignal<number | null>(null);
  const [schemaText, setSchemaText] = createSignal("");
  const [chatInput, setChatInput] = createSignal("");
  const [newModelInputByProvider, setNewModelInputByProvider] = createSignal<
    Record<AiProviderKind, string>
  >({} as Record<AiProviderKind, string>);
  const [providerKeyInputByProvider, setProviderKeyInputByProvider] = createSignal<
    Record<AiProviderKind, string>
  >({} as Record<AiProviderKind, string>);
  const [uiZoom, setUiZoom] = createSignal(readStoredUiZoom(globalThis.localStorage));
  const [editingWorkflowId, setEditingWorkflowId] = createSignal<string | null>(null);
  const [workflowNameDraft, setWorkflowNameDraft] = createSignal("");
  const [selectedAgentId, setSelectedAgentId] = createSignal<string | null>(null);
  const [editingAgentId, setEditingAgentId] = createSignal<string | null>(null);
  const [agentNameDraft, setAgentNameDraft] = createSignal("");
  const [editingNodeId, setEditingNodeId] = createSignal<NodeId | null>(null);
  const [nodeLabelDraft, setNodeLabelDraft] = createSignal("");
  const [agentSchemaDraft, setAgentSchemaDraft] = createSignal("");
  const [addNodePickerOpen, setAddNodePickerOpen] = createSignal(false);
  const [isMaximized, setIsMaximized] = createSignal(false);
  const [availableSkills, setAvailableSkills] = createSignal<SkillSummary[]>([]);

  // ── Mutable refs (not signals) ────────────────────────────────────────────
  let workflowNameInput: HTMLInputElement | undefined;
  let agentNameInput: HTMLInputElement | undefined;
  let dockResizeState: { startY: number; startHeight: number } | null = null;

  const setWorkflowNameInputRef = (el: HTMLInputElement | undefined) => {
    workflowNameInput = el;
  };

  // ── Memos ─────────────────────────────────────────────────────────────────
  const activeWorkflow = createMemo(() =>
    workflows().find((workflow) => workflow.id === activeWorkflowId()),
  );
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
  const chatMessages = createMemo(() => {
    const nodeId = selectedNodeId();
    if (!nodeId) return [];
    return runState()?.chatLogs[nodeId] ?? [];
  });
  const selectedPendingApproval = createMemo(() => {
    const nodeId = selectedNodeId();
    const approvals = runState()?.pendingApprovals ?? [];
    if (!nodeId) return approvals[0] ?? null;
    return approvals.find((approval) => approval.nodeId === nodeId) ?? approvals[0] ?? null;
  });
  const chatEnabledMemo = createMemo(
    () =>
      runState()?.active === true &&
      runState()?.awaitingNodeId === selectedNodeId() &&
      (readiness()?.ready ?? false),
  );
  const chatComposerBusyMemo = createMemo(() =>
    isChatComposerBusy(runState(), selectedNodeId()),
  );
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
  const chatSubmission = createMemo(() =>
    resolveChatSubmission(chatInput(), skillIdsMemo()),
  );
  const canSendChatMemo = createMemo(
    () =>
      !selectedPendingApproval() &&
      chatEnabledMemo() &&
      chatSubmission().submittedText !== "",
  );

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
    console.log(`Selecting bottom tab: ${tab}`);
    setBottomTab(tab);
    setDockOpen(true);
    setDockHeight((current) => clampDockHeight(current, tab));
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
    initialSettings: AppSettings,
    initialRunState: WorkflowRunState | null,
  ) => {
    let nextWorkflows = initialWorkflows;
    if (nextWorkflows.length === 0) {
      nextWorkflows = [await desktop.createWorkflow("Workflow 1")];
    }
    const firstWorkflow = nextWorkflows[0];
    setWorkflows(nextWorkflows);
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

  const handleCreateWorkflow = async () => {
    try {
      const workflow = await desktop.createWorkflow(`Workflow ${workflows().length + 1}`);
      setWorkflows([...workflows(), workflow]);
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
      const defaultModel = activeProfileMemo().default_model;
      const nextNode =
        defaultModel && !node.agent.model
          ? { ...node, agent: { ...node.agent, model: defaultModel } }
          : node;
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

  const handleRun = async () => {
    const workflow = activeWorkflow();
    if (!workflow || !applySchemaEditor()) return;
    try {
      const nextRunState = await desktop.startRun(
        activeWorkflow()!,
        settings(),
        activeProviderKeyInput() || null,
      );
      setRunState(nextRunState);
      setSelectedTraceIndex(null);
      setBottomTab("chat");
      clearStatusToast();
    } catch (error) {
      setError(normalizeError(error));
    }
  };

  const handleClearRunTrace = async () => {
    try {
      const nextRunState = await desktop.clearRunTrace();
      if (nextRunState) setRunState(nextRunState);
      setSelectedTraceIndex(null);
    } catch (error) {
      setError(normalizeError(error));
    }
  };

  const handleSubmitChat = async () => {
    const nodeId = selectedNodeId();
    if (!nodeId || !canSendChatMemo()) return;
    try {
      const nextRunState = await desktop.submitUserInput(
        nodeId,
        chatSubmission().submittedText,
      );
      setRunState(nextRunState);
      setChatInput("");
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

  const handleToolApproval = async (allow: boolean) => {
    const approval = selectedPendingApproval();
    if (!approval) return;
    try {
      const nextRunState = await desktop.submitToolApproval(approval.approvalId, allow);
      setRunState(nextRunState);
    } catch (error) {
      setError(normalizeError(error));
    }
  };

  const handleSelectNode = (nodeId: NodeId | null) => {
    setSelectedEdgeId(null);
    setSelectedNodeId(nodeId);
    setEditingNodeId(null);
    setNodeLabelDraft("");
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

  const handleChatInputKeyDown = (event: KeyboardEvent) => {
    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      void handleSubmitChat();
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
      void handleRun();
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
    const tab = bottomTab();
    setDockHeight((current) => clampDockHeight(current, tab));
  });

  // ── Mount ─────────────────────────────────────────────────────────────────
  onMount(async () => {
    let unlisten: (() => void) | null = null;
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
      if (unlistenMaximized) void unlistenMaximized();
    });

    applyUiZoom(uiZoom());

    try {
      const appWindow = getCurrentWindow();
      const initialMaximized = await appWindow.isMaximized();
      setIsMaximized(initialMaximized);
      unlistenMaximized = await appWindow.onResized(() => {
        void appWindow.isMaximized().then(setIsMaximized);
      });
      unlisten = await bindRunStateEvents(
        {
          handleRunStateUpdate: (nextRunState) => {
            setRunState(nextRunState);
            if (nextRunState.pendingApprovals.length > 0) {
              setSelectedEdgeId(null);
              setSelectedNodeId(nextRunState.pendingApprovals[0].nodeId);
              setEditingNodeId(null);
              setNodeLabelDraft("");
              setDockOpen(true);
              setBottomTab("chat");
              setDockHeight((current) => clampDockHeight(current, "chat"));
              toast(
                `${nextRunState.pendingApprovals[0].nodeLabel} needs tool approval`,
                { id: STATUS_TOAST_ID },
              );
            } else if (nextRunState.awaitingNodeId) {
              const label =
                activeWorkflow()?.nodes.find((n) => n.id === nextRunState.awaitingNodeId)
                  ?.label ?? "Node";
              setSelectedEdgeId(null);
              setSelectedNodeId(nextRunState.awaitingNodeId);
              setEditingNodeId(null);
              setNodeLabelDraft("");
              setDockOpen(true);
              setBottomTab("chat");
              setDockHeight((current) => clampDockHeight(current, "chat"));
              toast(`${label} is waiting for input`, { id: STATUS_TOAST_ID });
            }
            if (nextRunState.lastError) {
              setError(nextRunState.lastError);
            }
          },
        },
        desktop,
      );
      const data = await desktop.bootstrapApp();
      setAvailableSkills(data.skills ?? []);
      await initializeWorkspace(data.workflows, data.agents, data.settings, data.runState);
    } catch (error) {
      setError(normalizeError(error));
    }
  });

  // ── Context value ─────────────────────────────────────────────────────────
  const value = {
    // Signals
    workflows,
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
    selectedTraceIndex,
    schemaText,
    chatInput,
    newModelInputByProvider,
    providerKeyInputByProvider,
    uiZoom,
    editingWorkflowId,
    workflowNameDraft,
    selectedAgentId,
    editingAgentId,
    agentNameDraft,
    editingNodeId,
    nodeLabelDraft,
    agentSchemaDraft,
    addNodePickerOpen,
    isMaximized,
    availableSkills,
    skillById,
    // Setters
    setWorkflowNameDraft,
    setChatInput,
    setNewModelInputByProvider,
    setProviderKeyInputByProvider,
    setNodeLabelDraft,
    setSchemaText,
    setSelectedTraceIndex,
    setSelectedAgentId,
    setScreen,
    // Memos
    activeWorkflow,
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
    chatMessages,
    selectedPendingApproval,
    chatEnabledMemo,
    chatComposerBusyMemo,
    chatSubmission,
    canSendChatMemo,
    // Ref setters
    setWorkflowNameInputRef,
    // Handlers
    handleSwitchWorkflow,
    handleCreateWorkflow,
    handleOpenAgents,
    handleCreateAgent,
    handleSaveAgents,
    handleAgentSchemaInput,
    updateSelectedAgent,
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
    handleClearRunTrace,
    handleSubmitChat,
    handleRefreshSkills,
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
    handleSelectBottomTab,
    handleDockResizePointerDown,
    handleZoomIn,
    handleZoomOut,
    handleZoomReset,
  };

  return <AppContext.Provider value={value}>{props.children}</AppContext.Provider>;
}
