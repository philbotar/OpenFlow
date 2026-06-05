import { onCleanup, onMount, createEffect, createMemo, createSignal, For, Show } from "solid-js";
import ArrowUp from "lucide-solid/icons/arrow-up";
import { getCurrentWindow } from "@tauri-apps/api/window";
import CircleCheck from "lucide-solid/icons/circle-check";
import Bot from "lucide-solid/icons/bot";
import PencilLine from "lucide-solid/icons/pencil-line";
import Play from "lucide-solid/icons/play";
import Plus from "lucide-solid/icons/plus";
import Save from "lucide-solid/icons/save";
import Settings2 from "lucide-solid/icons/settings-2";
import Trash2 from "lucide-solid/icons/trash-2";
import PanelLeftClose from "lucide-solid/icons/panel-left-close";
import {
  bootstrapApp,
  clearRunTrace,
  createAgentDefinition,
  createAgentNode,
  createWorkflow,
  resolveProviderReadiness,
  saveAgents,
  saveSettings,
  saveWorkflows,
  startRun,
  submitToolApproval,
  submitUserInput,
  validateWorkflow,
  listenToRunState,
} from "./api";
import WorkflowCanvasHost from "./canvas/WorkflowCanvasHost";
import { resolveChatSubmission } from "./chatCommands";
import type {
  AgentDefinition,
  AiProviderKind,
  AppSettings,
  ChatRole,
  EdgeId,
  NodeId,
  ProviderTransport,
  RunTraceEntry,
  Workflow,
  WorkflowRunState,
} from "./types";
import {
  activeProfile,
  cloneSettings,
  cloneWorkflow,
  createIdleRunState,
  nodeOutput,
  prettyJson,
  projectWorkflowCanvasGraph,
  projectWorkflowCanvasStatusByNode,
  removeSelectedNode,
  replaceWorkflow,
  selectedNode,
  SUPPORTED_NODE_TOOLS,
  type WorkflowCanvasGraph,
  type WorkflowCanvasStatusByNode,
} from "./workflow";
import {
  clampUiZoom,
  DEFAULT_UI_ZOOM,
  formatUiZoomLabel,
  readStoredUiZoom,
  writeStoredUiZoom,
  zoomInUi,
  zoomOutUi,
} from "./uiZoom";
import { resolveCommittedNodeLabel } from "./nodeLabel";

type BannerKind = "error" | "success" | "info";
type Banner = { kind: BannerKind; text: string } | null;
type BottomTab = "overview" | "chat" | "trace";
type Screen = "editor" | "settings" | "agents";

const EMPTY_SETTINGS: AppSettings = {
  active_provider: "open_ai",
  openai: {
    display_name: "ChatGPT / OpenAI",
    base_url: "https://api.openai.com",
    transport: "responses",
    responses_path: "v1/responses",
    chat_completions_path: "v1/chat/completions",
    api_key: "",
    known_models: ["gpt-4o", "gpt-4o-mini", "gpt-4.5", "o3"],
    default_model: "gpt-4o-mini",
  },
  openai_compatible: {
    display_name: "OpenAI-compatible API",
    base_url: "http://localhost:11434",
    transport: "chat_completions",
    responses_path: "v1/responses",
    chat_completions_path: "v1/chat/completions",
    api_key: "",
    known_models: ["llama3.1", "qwen2.5", "mistral"],
    default_model: "llama3.1",
  },
};

type SidebarIconName = "agents" | "plus" | "edit" | "settings" | "save" | "validate" | "run" | "trash";

const ICON_STROKE_WIDTH = 1.9;
const BANNER_DISMISS_MS = 4000;
const DEFAULT_DOCK_HEIGHT = 188;
const COLLAPSED_DOCK_HEIGHT = 52;
const DOCK_VIEWPORT_MARGIN = 160;

function SidebarIcon(props: { name: SidebarIconName }) {
  switch (props.name) {
    case "agents":
      return <Bot class="sidebar-icon" aria-hidden="true" absoluteStrokeWidth strokeWidth={ICON_STROKE_WIDTH} />;
    case "plus":
      return <Plus class="sidebar-icon" aria-hidden="true" absoluteStrokeWidth strokeWidth={ICON_STROKE_WIDTH} />;
    case "edit":
      return <PencilLine class="sidebar-icon" aria-hidden="true" absoluteStrokeWidth strokeWidth={ICON_STROKE_WIDTH} />;
    case "settings":
      return <Settings2 class="sidebar-icon" aria-hidden="true" absoluteStrokeWidth strokeWidth={ICON_STROKE_WIDTH} />;
    case "save":
      return <Save class="sidebar-icon" aria-hidden="true" absoluteStrokeWidth strokeWidth={ICON_STROKE_WIDTH} />;
    case "validate":
      return <CircleCheck class="sidebar-icon" aria-hidden="true" absoluteStrokeWidth strokeWidth={ICON_STROKE_WIDTH} />;
    case "run":
      return <Play class="sidebar-icon" aria-hidden="true" absoluteStrokeWidth strokeWidth={ICON_STROKE_WIDTH} />;
    case "trash":
      return <Trash2 class="sidebar-icon" aria-hidden="true" absoluteStrokeWidth strokeWidth={ICON_STROKE_WIDTH} />;
  }
}

function AgentConfigForm(props: {
  model: string;
  onModelChange: (value: string) => void;
  autoStart: boolean;
  onAutoStartChange: (value: boolean) => void;
  systemPrompt: string;
  onSystemPromptChange: (value: string) => void;
  taskPrompt: string;
  onTaskPromptChange: (value: string) => void;
  schemaJson: string;
  onSchemaChange: (value: string) => void;
  knownModels: readonly string[];
  defaultModel: string | null;
  listId: string;
  systemPromptRows?: number;
  taskPromptRows?: number;
  schemaRows?: number;
}) {
  const effectiveModel = () => props.model || props.defaultModel || "";
  return (
    <>
      <label>
        <span>Model</span>
        <input
          class="text-input"
          value={effectiveModel()}
          list={props.listId}
          onInput={(event) => props.onModelChange(event.currentTarget.value)}
        />
        <datalist id={props.listId}>
          <For each={props.knownModels}>{(model) => <option value={model} />}</For>
        </datalist>
      </label>
      <label class="checkbox-row">
        <input
          type="checkbox"
          checked={props.autoStart}
          onChange={(event) => props.onAutoStartChange(event.currentTarget.checked)}
        />
        <span>Auto-start without pausing for human input</span>
      </label>
      <label>
        <span>System prompt</span>
        <textarea
          class="text-area"
          rows={props.systemPromptRows ?? 4}
          value={props.systemPrompt}
          onInput={(event) => props.onSystemPromptChange(event.currentTarget.value)}
        />
      </label>
      <label>
        <span>Task prompt</span>
        <textarea
          class="text-area"
          rows={props.taskPromptRows ?? 3}
          value={props.taskPrompt}
          onInput={(event) => props.onTaskPromptChange(event.currentTarget.value)}
        />
      </label>
      <label>
        <span>JSON output schema</span>
        <textarea
          class="text-area code"
          rows={props.schemaRows ?? 8}
          value={props.schemaJson}
          onInput={(event) => props.onSchemaChange(event.currentTarget.value)}
        />
      </label>
    </>
  );
}

function ToolConfigEditor(props: {
  config: {
    catalog: { tools: { name: string }[] };
    approvalMode: "always_ask" | "write" | "yolo" | null;
    maxToolRounds: number;
  };
  onToolEnabledChange: (toolName: string, enabled: boolean) => void;
  onApprovalModeChange: (value: "always_ask" | "write" | "yolo" | null) => void;
  onMaxToolRoundsChange: (value: number) => void;
  defaultOpen?: boolean;
}) {
  const [open, setOpen] = createSignal(props.defaultOpen ?? false);
  const enabledTools = createMemo(
    () => new Set(props.config.catalog.tools.map((tool) => tool.name)),
  );

  return (
    <section class="tool-config-section">
      <div class="tool-config-header">
        <div class="tool-config-header-copy">
          <div class="eyebrow">Tool access</div>
          <p>Safe retrieval tools are enabled by default for this node.</p>
        </div>
        <button
          type="button"
          class="secondary-button tool-config-toggle"
          onClick={() => setOpen((value) => !value)}
          aria-expanded={open()}
        >
          {open() ? "Hide tools" : "Show tools"}
        </button>
      </div>
      <Show when={open()}>
        <div class="tool-config-body">
          <div class="tool-config-list" role="group" aria-label="Enabled node tools">
            <For each={SUPPORTED_NODE_TOOLS}>
              {(tool) => (
                <label class="tool-config-option">
                  <span class="tool-config-option-copy">
                    <span class="tool-config-option-title">{tool.name}</span>
                    <span class="tool-config-option-description">{tool.description}</span>
                  </span>
                  <input
                    type="checkbox"
                    checked={enabledTools().has(tool.name)}
                    onChange={(event) =>
                      props.onToolEnabledChange(tool.name, event.currentTarget.checked)
                    }
                  />
                </label>
              )}
            </For>
          </div>
          <div class="field-grid tool-config-grid">
            <label>
              <span>Approval mode</span>
              <select
                class="text-input"
                value={props.config.approvalMode ?? "write"}
                onChange={(event) =>
                  props.onApprovalModeChange(
                    event.currentTarget.value as "always_ask" | "write" | "yolo",
                  )
                }
              >
                <option value="always_ask">Always ask</option>
                <option value="write">Read tools auto-approve</option>
                <option value="yolo">Read and write auto-approve</option>
              </select>
            </label>
            <label>
              <span>Max tool rounds</span>
              <input
                class="text-input"
                type="number"
                min="1"
                max="32"
                value={props.config.maxToolRounds}
                onInput={(event) =>
                  props.onMaxToolRoundsChange(
                    Number.parseInt(event.currentTarget.value, 10) || 1,
                  )
                }
              />
            </label>
          </div>
        </div>
      </Show>
    </section>
  );
}

function App() {
  const [workflows, setWorkflows] = createSignal<Workflow[]>([]);
  const [agents, setAgents] = createSignal<AgentDefinition[]>([]);
  const [activeWorkflowId, setActiveWorkflowId] = createSignal<string | null>(null);
  const [selectedNodeId, setSelectedNodeId] = createSignal<NodeId | null>(null);
  const [selectedEdgeId, setSelectedEdgeId] = createSignal<EdgeId | null>(null);
  const [screen, setScreen] = createSignal<Screen>("editor");
  const [settings, setSettings] = createSignal<AppSettings>(cloneSettings(EMPTY_SETTINGS));
  const [runState, setRunState] = createSignal<WorkflowRunState | null>(null);
  const [readiness, setReadiness] = createSignal<{ ready: boolean; provider: string; message: string; envVar: string } | null>(null);
  const [banner, setBanner] = createSignal<Banner>(null);
  const [bottomTab, setBottomTab] = createSignal<BottomTab>("overview");
  const [dockOpen, setDockOpen] = createSignal(true);
  const [dockHeight, setDockHeight] = createSignal(DEFAULT_DOCK_HEIGHT);
  const [selectedTraceIndex, setSelectedTraceIndex] = createSignal<number | null>(null);
  const [schemaText, setSchemaText] = createSignal("");
  const [chatInput, setChatInput] = createSignal("");
  const [newModelInputByProvider, setNewModelInputByProvider] = createSignal<Record<AiProviderKind, string>>({
    open_ai: "",
    open_ai_compatible: "",
  });
  const [uiZoom, setUiZoom] = createSignal(readStoredUiZoom(globalThis.localStorage));
  const [editingWorkflowId, setEditingWorkflowId] = createSignal<string | null>(null);
  const [workflowNameDraft, setWorkflowNameDraft] = createSignal("");
  const [selectedAgentId, setSelectedAgentId] = createSignal<string | null>(null);
  const [editingAgentId, setEditingAgentId] = createSignal<string | null>(null);
  const [agentNameDraft, setAgentNameDraft] = createSignal("");
  const [editingNodeId, setEditingNodeId] = createSignal<NodeId | null>(null);
  const [nodeLabelDraft, setNodeLabelDraft] = createSignal("");
  const [agentSchemaDraft, setAgentSchemaDraft] = createSignal("");
  const [isMaximized, setIsMaximized] = createSignal(false);
  let workflowNameInput: HTMLInputElement | undefined;
  let agentNameInput: HTMLInputElement | undefined;
  let chatHistoryRef: HTMLDivElement | undefined;
  let dockResizeState: { startY: number; startHeight: number } | null = null;
  const activeWorkflow = createMemo(() =>
    workflows().find((workflow) => workflow.id === activeWorkflowId()),
  );
  const selectedAgent = createMemo(() =>
    agents().find((agent) => agent.id === selectedAgentId()) ?? null,
  );
  const canvasGraph = createMemo<WorkflowCanvasGraph | null>(
    (previous) => projectWorkflowCanvasGraph(activeWorkflow(), previous),
    null,
  );
  const canvasStatusByNode = createMemo<WorkflowCanvasStatusByNode | null>(
    (previous) => projectWorkflowCanvasStatusByNode(runState(), previous),
    null,
  );
  const currentNode = createMemo(() =>
    selectedNode(activeWorkflow(), selectedNodeId()),
  );
  const activeProfileMemo = createMemo(() => activeProfile(settings()));
  const selectedTrace = createMemo<RunTraceEntry | null>(() => {
    const index = selectedTraceIndex();
    if (index === null) {
      return null;
    }
    return runState()?.runTrace[index] ?? null;
  });
  const hasRunTraceMemo = createMemo(() =>
    (runState()?.runTrace.length ?? 0) > 0,
  );
  const currentNodeOutput = createMemo(() =>
    nodeOutput(runState(), selectedNodeId()),
  );
  const chatMessages = createMemo(() => {
    const nodeId = selectedNodeId();
    if (!nodeId) {
      return [];
    }
    return runState()?.chatLogs[nodeId] ?? [];
  });
  createEffect(() => {
    chatMessages();
    if (chatHistoryRef?.isConnected) {
      chatHistoryRef.scrollTop = chatHistoryRef.scrollHeight;
    }
  });
  const selectedPendingApproval = createMemo(() => {
    const nodeId = selectedNodeId();
    const approvals = runState()?.pendingApprovals ?? [];
    if (!nodeId) {
      return approvals[0] ?? null;
    }
    return approvals.find((approval) => approval.nodeId === nodeId) ?? approvals[0] ?? null;
  });
  const chatEnabledMemo = createMemo(() =>
    runState()?.active === true &&
    runState()?.awaitingNodeId === selectedNodeId() &&
    (readiness()?.ready ?? false),
  );
  const chatSubmission = createMemo(() => resolveChatSubmission(chatInput()));
  const canSendChatMemo = createMemo(() =>
    !selectedPendingApproval() &&
    chatEnabledMemo() &&
    chatSubmission().submittedText !== "",
  );

  const showBanner = (kind: BannerKind, text: string) => setBanner({ kind, text });
  const setError = (text: string) => showBanner("error", text);
  const setSuccess = (text: string) => showBanner("success", text);
  const applyUiZoom = (nextZoom: number) => {
    const normalized = clampUiZoom(nextZoom);
    setUiZoom(normalized);
    writeStoredUiZoom(globalThis.localStorage, normalized);
    document.documentElement.style.setProperty("--ui-zoom", String(normalized));
  };

  const handleSelectBottomTab = (tab: BottomTab) => {
    setBottomTab(tab);
    setDockOpen(true);
    setDockHeight((current) => clampDockHeight(current, tab));
  };

  const handleDockResizePointerDown = (event: PointerEvent) => {
    if (event.button !== 0) {
      return;
    }
    event.preventDefault();
    dockResizeState = {
      startY: event.clientY,
      startHeight: dockOpen() ? dockHeight() : COLLAPSED_DOCK_HEIGHT,
    };
    document.body.classList.add("is-resizing-dock");
  };

  const handleDockResizePointerMove = (event: PointerEvent) => {
    if (!dockResizeState) {
      return;
    }
    const nextHeight = dockResizeState.startHeight + (dockResizeState.startY - event.clientY);
    if (shouldCollapseDock(nextHeight, bottomTab())) {
      setDockOpen(false);
      return;
    }
    setDockOpen(true);
    setDockHeight(clampDockHeight(nextHeight, bottomTab()));
  };

  const clearDockResizeState = () => {
    if (!dockResizeState) {
      return;
    }
    dockResizeState = null;
    document.body.classList.remove("is-resizing-dock");
  };

  const handleZoomIn = () => {
    applyUiZoom(zoomInUi(uiZoom()));
  };

  const handleZoomOut = () => {
    applyUiZoom(zoomOutUi(uiZoom()));
  };

  const handleZoomReset = () => {
    applyUiZoom(DEFAULT_UI_ZOOM);
  };

  const refreshReadiness = async (
    nextSettings = settings(),
  ) => {
    try {
      setReadiness(await resolveProviderReadiness(nextSettings));
    } catch (error) {
      setError(normalizeError(error));
    }
  };

  const initializeWorkspace = async (
    initialWorkflows: Workflow[],
    initialAgents: AgentDefinition[],
    initialSettings: AppSettings,
    initialRunState: WorkflowRunState | null,
  ) => {
    let nextWorkflows = initialWorkflows;
    if (nextWorkflows.length === 0) {
      nextWorkflows = [await createWorkflow("Workflow 1")];
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
    setBanner(null);
    await refreshReadiness(initialSettings);
  };

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
      if (unlisten) {
        void unlisten();
      }
      if (unlistenMaximized) {
        void unlistenMaximized();
      }
    });
    applyUiZoom(uiZoom());
    try {
      const appWindow = getCurrentWindow();
      const initialMaximized = await appWindow.isMaximized();
      setIsMaximized(initialMaximized);
      unlistenMaximized = await appWindow.onResized(() => {
        void appWindow.isMaximized().then(setIsMaximized);
      });
      unlisten = await listenToRunState((nextRunState) => {
        setRunState(nextRunState);
        if (nextRunState.pendingApprovals.length > 0) {
          setSelectedEdgeId(null);
          setSelectedNodeId(nextRunState.pendingApprovals[0].nodeId);
          setEditingNodeId(null);
          setNodeLabelDraft("");
          setDockOpen(true);
          setBottomTab("chat");
          setDockHeight((current) => clampDockHeight(current, "chat"));
        } else if (nextRunState.awaitingNodeId) {
          setSelectedEdgeId(null);
          setSelectedNodeId(nextRunState.awaitingNodeId);
          setEditingNodeId(null);
          setNodeLabelDraft("");
          setDockOpen(true);
          setBottomTab("chat");
          setDockHeight((current) => clampDockHeight(current, "chat"));
        }
        if (nextRunState.lastError) {
          setError(nextRunState.lastError);
        }
      });
      const data = await bootstrapApp();
      await initializeWorkspace(data.workflows, data.agents, data.settings, data.runState);
    } catch (error) {
      setError(normalizeError(error));
    }
  });

  createEffect(() => {
    const currentBanner = banner();
    if (!currentBanner) {
      return;
    }

    const timeoutId = window.setTimeout(() => {
      setBanner((active) => (active === currentBanner ? null : active));
    }, BANNER_DISMISS_MS);

    onCleanup(() => window.clearTimeout(timeoutId));
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
    if (!workflowId) {
      return;
    }
    queueMicrotask(() => {
      if (editingWorkflowId() !== workflowId || !workflowNameInput) {
        return;
      }
      workflowNameInput.focus();
      workflowNameInput.setSelectionRange(0, workflowNameInput.value.length);
    });
  });

  createEffect(() => {
    const tab = bottomTab();
    setDockHeight((current) => clampDockHeight(current, tab));
  });

  const updateSettings = async (mutator: (draft: AppSettings) => void) => {
    const next = cloneSettings(settings());
    mutator(next);
    setSettings(next);
    await refreshReadiness(next);
  };

  const updateActiveWorkflow = (mutator: (draft: Workflow) => void) => {
    const workflow = activeWorkflow();
    if (!workflow) {
      return;
    }
    const next = cloneWorkflow(workflow);
    mutator(next);
    setWorkflows(replaceWorkflow(workflows(), next));
  };

  const updateCurrentNode = (mutator: (node: Workflow["nodes"][number]) => void) => {
    const nodeId = selectedNodeId();
    if (!nodeId) {
      return;
    }
    updateActiveWorkflow((draft) => {
      const nextNode = draft.nodes.find((item) => item.id === nodeId);
      if (nextNode) {
        mutator(nextNode);
      }
    });
  };

  const updateCurrentNodeToolConfig = (
    mutator: (tools: Workflow["nodes"][number]["agent"]["tools"]) => void,
  ) => {
    updateCurrentNode((node) => {
      mutator(node.agent.tools);
    });
  };

  const setToolEnabled = (
    tools: { catalog: { tools: { name: string }[] } },
    toolName: string,
    enabled: boolean,
  ) => {
    const nextTools = tools.catalog.tools.filter((tool) => tool.name !== toolName);
    tools.catalog.tools = enabled
      ? [...nextTools, { name: toolName }].sort((left, right) => left.name.localeCompare(right.name))
      : nextTools;
  };

  const applySchemaEditor = () => {
    const nodeId = selectedNodeId();
    const workflow = activeWorkflow();
    if (!nodeId || !workflow) {
      return true;
    }
    try {
      const parsed = JSON.parse(schemaText());
      updateActiveWorkflow((draft) => {
        const node = draft.nodes.find((item) => item.id === nodeId);
        if (node) {
          node.agent.output_schema = parsed;
        }
      });
      setBanner(null);
      return true;
    } catch (error) {
      setError(`output schema JSON invalid: ${normalizeError(error)}`);
      return false;
    }
  };

  const persistAll = async (successText = "Saved") => {
    if (!applySchemaEditor()) {
      return false;
    }
    try {
      await saveWorkflows(workflows());
      await saveSettings(settings());
      setSuccess(successText);
      return true;
    } catch (error) {
      setError(normalizeError(error));
      return false;
    }
  };

  const handleSwitchWorkflow = (workflowId: string) => {
    if (!applySchemaEditor()) {
      return;
    }
    const workflow = workflows().find((item) => item.id === workflowId);
    if (!workflow) {
      return;
    }
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
      const workflow = await createWorkflow(`Workflow ${workflows().length + 1}`);
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
    setScreen("agents");
    if (!selectedAgentId() && agents().length > 0) {
      setSelectedAgentId(agents()[0].id);
    }
  };

  const handleCreateAgent = async () => {
    try {
      const agent = await createAgentDefinition(`Agent ${agents().length + 1}`);
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

  const updateSelectedAgent = (mutator: (draft: AgentDefinition) => void) => {
    const current = selectedAgent();
    if (!current) {
      return;
    }
    const next = {
      ...current,
      output_schema: structuredClone(current.output_schema),
    };
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
      setBanner(null);
    } catch {
      // preserve draft until save
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
      await saveAgents(agents());
      setSuccess("Saved agents");
    } catch (error) {
      setError(normalizeError(error));
    }
  };
  const handleSaveSettings = async () => {
    try {
      await saveSettings(settings());
      setSuccess("Settings saved successfully.");
    } catch (error) {
      setError(normalizeError(error));
    }
  };

  const handleAddNode = async () => {
    const workflow = activeWorkflow();
    if (!workflow) {
      return;
    }
    try {
      const node = await createAgentNode(
        workflow.nodes.length,
        96 + workflow.nodes.length * 32,
        96 + workflow.nodes.length * 20,
      );
      const defaultModel = activeProfileMemo().default_model;
      const nextNode = defaultModel ? { ...node, agent: { ...node.agent, model: defaultModel } } : node;
      updateActiveWorkflow((draft) => {
        draft.nodes.push(nextNode);
      });
      setSelectedNodeId(node.id);
      setSelectedEdgeId(null);
      setEditingNodeId(null);
      setNodeLabelDraft("");
      setSuccess("Added node");
    } catch (error) {
      setError(normalizeError(error));
    }
  };

  const handleDeleteSelectedNode = () => {
    const workflow = activeWorkflow();
    const nodeId = selectedNodeId();
    if (!workflow || !nodeId) {
      return;
    }
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
    if (selectedEdgeId() === edgeId) {
      setSelectedEdgeId(null);
    }
  };

  const handleValidate = async () => {
    const workflow = activeWorkflow();
    if (!workflow || !applySchemaEditor()) {
      return;
    }
    try {
      const summary = await validateWorkflow(activeWorkflow()!);
      setSuccess(`Valid DAG · ${summary.layerCount} layer${summary.layerCount === 1 ? "" : "s"}`);
    } catch (error) {
      setError(normalizeError(error));
    }
  };

  const handleRun = async () => {
    const workflow = activeWorkflow();
    if (!workflow || !applySchemaEditor()) {
      return;
    }
    try {
      const nextRunState = await startRun(activeWorkflow()!, settings());
      setRunState(nextRunState);
      setSelectedTraceIndex(null);
      setBottomTab("chat");
      setBanner(null);
    } catch (error) {
      setError(normalizeError(error));
    }
  };

  const handleClearRunTrace = async () => {
    try {
      const nextRunState = await clearRunTrace();
      if (nextRunState) {
        setRunState(nextRunState);
      }
      setSelectedTraceIndex(null);
    } catch (error) {
      setError(normalizeError(error));
    }
  };

  const handleSubmitChat = async () => {
    const nodeId = selectedNodeId();
    if (!nodeId || !canSendChatMemo()) {
      return;
    }
    try {
      const nextRunState = await submitUserInput(nodeId, chatSubmission().submittedText);
      setRunState(nextRunState);
      setChatInput("");
    } catch (error) {
      setError(normalizeError(error));
    }
  };

  const handleToolApproval = async (allow: boolean) => {
    const approval = selectedPendingApproval();
    if (!approval) {
      return;
    }
    try {
      const nextRunState = await submitToolApproval(approval.approvalId, allow);
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
    if (edgeId) {
      setSelectedNodeId(null);
    }
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
    if (from === to) {
      return;
    }

    const edgeId = crypto.randomUUID();
    let created = false;

    updateActiveWorkflow((draft) => {
      const duplicate = draft.edges.some((edge) => edge.from === from && edge.to === to);
      if (duplicate) {
        return;
      }

      draft.edges.push({
        id: edgeId,
        from,
        to,
      });
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
    if (from === to) {
      return;
    }

    let reconnected = false;

    updateActiveWorkflow((draft) => {
      const duplicate = draft.edges.some(
        (edge) => edge.id !== edgeId && edge.from === from && edge.to === to,
      );
      if (duplicate) {
        return;
      }

      const edge = draft.edges.find((item) => item.id === edgeId);
      if (!edge) {
        return;
      }

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
    if (!workflowId) {
      return;
    }
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

  const handleChatInputKeyDown = (event: KeyboardEvent) => {
    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      void handleSubmitChat();
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
    if (!nodeId) {
      return;
    }

    const currentLabel = currentNode()?.label ?? "";
    const nextLabel = resolveCommittedNodeLabel(currentLabel, nodeLabelDraft());
    updateActiveWorkflow((draft) => {
      const nextNode = draft.nodes.find((item) => item.id === nodeId);
      if (nextNode) {
        nextNode.label = nextLabel;
      }
    });
    handleCancelNodeLabelEdit();
  };

  const handleAddKnownModel = () => {
    const provider = settings().active_provider;
    const nextName = newModelInputByProvider()[provider].trim();
    if (nextName === "") {
      return;
    }
    void updateSettings((draft) => {
      const profile = activeProfile(draft);
      if (!profile.known_models.includes(nextName)) {
        profile.known_models = [...profile.known_models, nextName];
      }
    });
    setNewModelInputByProvider({
      ...newModelInputByProvider(),
      [provider]: "",
    });
  };

  const handleRemoveKnownModel = (model: string) => {
    void updateSettings((draft) => {
      const profile = activeProfile(draft);
      profile.known_models = profile.known_models.filter((item) => item !== model);
    });
  };

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
    if ((event.key === "Delete" || event.key === "Backspace") && !isTextInputTarget(event.target) && screen() === "editor") {
      event.preventDefault();
      const edgeId = selectedEdgeId();
      if (edgeId) {
        handleDeleteEdge(edgeId);
        return;
      }
      handleDeleteSelectedNode();
    }
  }

  return (
    <div class="app-shell">
      <aside class="sidebar" classList={{ "sidebar-macos": isMacOS(), "sidebar-maximized": isMaximized() }}>
        <Show when={isMacOS()}>
          <div class="sidebar-window-controls-spacer" aria-hidden="true" data-tauri-drag-region />
        </Show>
        <div class="sidebar-brand" data-tauri-drag-region>
          <div class="brand-mark" aria-hidden="true" />
          <div class="brand-copy">
            <span class="brand-title">OpenFlow</span>
          </div>
        </div>
        <div class="sidebar-list">
          <button class="sidebar-nav-button" onClick={handleOpenAgents}>
            <SidebarIcon name="agents" />
            <span>Agents</span>
          </button>
          <button class="sidebar-nav-button" onClick={() => void handleCreateWorkflow()}>
            <SidebarIcon name="plus" />
            <span>New workflow</span>
          </button>
          <For each={workflows()}>
            {(workflow) => {
              const active = () => workflow.id === activeWorkflowId() && screen() === "editor";
              const editing = () => workflow.id === editingWorkflowId();
              return (
                <div class="workflow-row" classList={{ active: active(), editing: editing() }}>
                  <Show
                    when={!editing()}
                    fallback={
                      <div class="workflow-row-main">
                        <input
                          ref={(element) => {
                            workflowNameInput = element;
                          }}
                          value={workflowNameDraft()}
                          onInput={(event) => setWorkflowNameDraft(event.currentTarget.value)}
                          onBlur={handleWorkflowNameCommit}
                          onKeyDown={handleWorkflowNameKeyDown}
                          class="workflow-row-input"
                          aria-label={`Workflow name for ${workflow.name}`}
                        />
                      </div>
                    }
                  >
                    <button
                      type="button"
                      class="workflow-row-main"
                      onClick={() => handleSwitchWorkflow(workflow.id)}
                    >
                      <div class="workflow-row-details">
                        <span class="workflow-row-title">{workflow.name}</span>
                      </div>
                    </button>
                  </Show>
                  <button
                    type="button"
                    class="sidebar-icon-button workflow-row-action hover-show"
                    onClick={() => handleStartWorkflowNameEdit(workflow.id, workflow.name)}
                    title="Rename workflow"
                    aria-label={`Rename ${workflow.name}`}
                  >
                    <SidebarIcon name="edit" />
                  </button>
                </div>
              );
            }}
          </For>
        </div>
        <div class="sidebar-footer">
          <div class="settings-nav-menu">
            <button
              class="sidebar-nav-button"
              onClick={() => setScreen(screen() === "settings" ? "editor" : "settings")}
            >
              <SidebarIcon name="settings" />
              <span>{screen() === "settings" ? "Back to editor" : "Settings"}</span>
            </button>
            <div class="settings-nav-popup" aria-hidden="true">Zoom {formatUiZoomLabel(uiZoom())}</div>
          </div>
        </div>
      </aside>

      <main class="main-shell">
        <header class="topbar" classList={{ "topbar-macos": isMacOS(), "topbar-maximized": isMaximized() }} data-tauri-drag-region>
          <div class="topbar-leading">
            <div class="topbar-copy" data-tauri-drag-region>
              <h2>{screen() === "agents" ? "Agents" : activeWorkflow()?.name ?? "Loading…"}</h2>
            </div>
          </div>
          <div class="topbar-actions" data-tauri-drag-region>
            <div class="readiness-chip" classList={{ ready: readiness()?.ready }}>
              <span class="status-dot" />
              <span>{readiness()?.message ?? "Checking provider"}</span>
            </div>
            <Show when={screen() === "editor"}>
              <div class="toolbar-group topbar-button-group">
                <button
                  class="topbar-icon-button"
                  onClick={() => void persistAll()}
                  title="Save"
                  aria-label="Save workflow"
                  data-tauri-drag-region="false"
                >
                  <SidebarIcon name="save" />
                </button>
                <button
                  class="topbar-icon-button"
                  onClick={() => void handleValidate()}
                  title="Validate"
                  aria-label="Validate workflow"
                  data-tauri-drag-region="false"
                >
                  <SidebarIcon name="validate" />
                </button>
                <button
                  class="topbar-icon-button topbar-icon-button-primary"
                  onClick={() => void handleRun()}
                  title="Run"
                  aria-label="Run workflow"
                  data-tauri-drag-region="false"
                >
                  <SidebarIcon name="run" />
                </button>
              </div>
            </Show>
          </div>
        </header>

        <Show
          when={screen() === "editor"}
          fallback={
            <Show
              when={screen() === "settings"}
              fallback={
                <section class="agents-screen">
                  <div class="agents-layout">
                    <aside class="agents-sidebar-panel">
                      <div class="agents-sidebar-header">
                        <div>
                          <h3>Agents</h3>
                        </div>
                        <button class="sidebar-icon-button" aria-label="New agent" onClick={() => void handleCreateAgent()}>
                          <SidebarIcon name="plus" />
                        </button>
                      </div>
                      <div class="agent-definition-list">
                        <Show when={agents().length > 0} fallback={<div class="empty-panel agents-empty-panel">No saved agents yet.</div>}>
                          <For each={agents()}>
                            {(agent) => (
                              <button
                                class="agent-list-row"
                                classList={{ active: agent.id === selectedAgentId() }}
                                onClick={() => setSelectedAgentId(agent.id)}
                              >

                                <span class="agent-list-row-title">{agent.name || "Untitled agent"}</span>
                              </button>
                            )}
                          </For>
                        </Show>
                      </div>
                    </aside>
                    <section class="agents-detail-panel">
                      <Show
                        when={selectedAgent()}
                        fallback={<div class="empty-panel agents-detail-empty">Select an agent to edit its prompts, schema, and model.</div>}
                      >
                        {(agent) => (
                          <div class="settings-section">
                            <label>
                              <span>Name</span>
                              <input
                                class="text-input"
                                value={agent().name}
                                onInput={(event) =>
                                  updateSelectedAgent((draft) => {
                                    draft.name = event.currentTarget.value;
                                  })
                                }
                              />
                            </label>

                            <AgentConfigForm
                              model={agent().model}
                              onModelChange={(value) =>
                                updateSelectedAgent((draft) => {
                                  draft.model = value;
                                })
                              }
                              autoStart={agent().auto_start}
                              onAutoStartChange={(value) =>
                                updateSelectedAgent((draft) => {
                                  draft.auto_start = value;
                                })
                              }
                              systemPrompt={agent().system_prompt}
                              onSystemPromptChange={(value) =>
                                updateSelectedAgent((draft) => {
                                  draft.system_prompt = value;
                                })
                              }
                              taskPrompt={agent().task_prompt}
                              onTaskPromptChange={(value) =>
                                updateSelectedAgent((draft) => {
                                  draft.task_prompt = value;
                                })
                              }
                              schemaJson={agentSchemaDraft()}
                              onSchemaChange={(value) => handleAgentSchemaInput(value)}
                              knownModels={activeProfileMemo().known_models}
                              defaultModel={activeProfileMemo().default_model}
                              listId="agent-model-list"
                            />
                            <ToolConfigEditor
                              config={agent().tools}
                              onToolEnabledChange={(toolName, enabled) =>
                                updateSelectedAgent((draft) => {
                                  setToolEnabled(draft.tools, toolName, enabled);
                                })
                              }
                              onApprovalModeChange={(value) =>
                                updateSelectedAgent((draft) => {
                                  draft.tools.approvalMode = value;
                                })
                              }
                              onMaxToolRoundsChange={(value) =>
                                updateSelectedAgent((draft) => {
                                  draft.tools.maxToolRounds = Math.min(32, Math.max(1, value));
                                })
                              }
                            />
                            <div class="button-row end">
                              <button class="primary-button" onClick={() => void handleSaveAgents()}>Save</button>
                            </div>
                          </div>
                        )}
                      </Show>
                    </section>
                  </div>
                </section>
              }
            >
              <section class="settings-screen">
                <div class="settings-panel">
                  <div class="settings-section">
                    <div>
                      <div class="eyebrow">Authentication</div>
                      <h3>Provider API key</h3>
                      <p>Saved in this app’s settings file for the selected provider. Environment variables still act as fallback.</p>
                    </div>
                    <input
                      type="password"
                      value={activeProfileMemo().api_key}
                      onInput={(event) =>
                        void updateSettings((draft) => {
                          activeProfile(draft).api_key = event.currentTarget.value;
                        })
                      }
                      placeholder={readiness()?.envVar ?? "OPENAI_API_KEY"}
                      class="text-input"
                    />
                  </div>

                  <div class="settings-section">
                    <div>
                      <div class="eyebrow">Provider</div>
                      <h3>Execution transport</h3>
                    </div>
                    <div class="segmented-control">
                      <button
                        classList={{ active: settings().active_provider === "open_ai" }}
                        onClick={() => void updateSettings((draft) => {
                          draft.active_provider = "open_ai";
                        })}
                      >
                        OpenAI
                      </button>
                      <button
                        classList={{ active: settings().active_provider === "open_ai_compatible" }}
                        onClick={() => void updateSettings((draft) => {
                          draft.active_provider = "open_ai_compatible";
                        })}
                      >
                        Compatible
                      </button>
                    </div>
                    <div class="field-grid">
                      <label>
                        <span>Base URL</span>
                        <input
                          class="text-input"
                          value={activeProfileMemo().base_url}
                          onInput={(event) =>
                            void updateSettings((draft) => {
                              activeProfile(draft).base_url = event.currentTarget.value;
                            })
                          }
                        />
                      </label>
                      <label>
                        <span>Transport</span>
                        <select
                          class="text-input"
                          value={activeProfileMemo().transport}
                          onChange={(event) =>
                            void updateSettings((draft) => {
                              activeProfile(draft).transport = event.currentTarget.value as ProviderTransport;
                            })
                          }
                        >
                          <option value="responses">Responses API</option>
                          <option value="chat_completions">Chat Completions API</option>
                        </select>
                      </label>
                      <label>
                        <span>Responses path</span>
                        <input
                          class="text-input"
                          value={activeProfileMemo().responses_path}
                          onInput={(event) =>
                            void updateSettings((draft) => {
                              activeProfile(draft).responses_path = event.currentTarget.value;
                            })
                          }
                        />
                      </label>
                      <label>
                        <span>Chat completions path</span>
                        <input
                          class="text-input"
                          value={activeProfileMemo().chat_completions_path}
                          onInput={(event) =>
                            void updateSettings((draft) => {
                              activeProfile(draft).chat_completions_path = event.currentTarget.value;
                            })
                          }
                        />
                      </label>
                    </div>
                  </div>

                  <div class="settings-section">
                    <div>
                      <div class="eyebrow">Models</div>
                      <h3>Known models for the active provider</h3>
                    </div>
                    <div class="chip-list">
                      <For each={activeProfileMemo().known_models}>
                        {(model) => (
                          <button class="model-chip" onClick={() => handleRemoveKnownModel(model)}>
                            {model}
                            <span>×</span>
                          </button>
                        )}
                      </For>
                    </div>
                    <div class="inline-form">
                      <input
                        class="text-input"
                        placeholder="Add model"
                        value={newModelInputByProvider()[settings().active_provider]}
                        onInput={(event) =>
                          setNewModelInputByProvider({
                            ...newModelInputByProvider(),
                            [settings().active_provider]: event.currentTarget.value,
                          })
                        }
                      />
                      <button class="secondary-button" onClick={handleAddKnownModel}>
                        Add model
                      </button>
                    </div>
                    <label>
                      <span>Default model</span>
                      <input
                        class="text-input"
                        list="known-models-settings"
                        value={activeProfileMemo().default_model ?? ""}
                        onInput={(event) =>
                          void updateSettings((draft) => {
                            activeProfile(draft).default_model = event.currentTarget.value || null;
                          })
                        }
                      />
                      <datalist id="known-models-settings">
                        <For each={activeProfileMemo().known_models}>{(model) => <option value={model} />}</For>
                      </datalist>
                    </label>
                    <button class="primary-button" onClick={() => void handleSaveSettings()}>Save settings</button>
                  </div>
                </div>
              </section>
            </Show>
          }
        >
          <div
            class="editor-screen"
            style={{ "--dock-height": `${dockOpen() ? dockHeight() : COLLAPSED_DOCK_HEIGHT}px` }}
          >
            <Show when={banner()}>
              {(currentBanner) => (
                <div class="banner" classList={{ error: currentBanner().kind === "error", success: currentBanner().kind === "success" }}>
                  {currentBanner().text}
                </div>
              )}
            </Show>

            <div class="workspace-grid">
              <section class="canvas-panel">
                <WorkflowCanvasHost
                  graph={canvasGraph()}
                  selectedNodeId={selectedNodeId()}
                  selectedEdgeId={selectedEdgeId()}
                  statusByNode={canvasStatusByNode()}
                  onSelectNode={handleSelectNode}
                  onSelectEdge={handleSelectEdge}
                  onUpdateNodePosition={handleCanvasNodePosition}
                  onCreateEdge={handleCreateEdge}
                  onReconnectEdge={handleReconnectEdge}
                  onDeleteEdge={handleDeleteEdge}
                  onAddNode={() => void handleAddNode()}
                />
              </section>

              <aside class="inspector-panel">
                <Show
                  when={currentNode()}
                  fallback={<div class="empty-panel">Select a node to edit its prompts, schema, and model.</div>}
                >
                  {(node) => (
                    <>
                      <div class="panel-header">
                        <div class="panel-header-copy">
                          <div class="eyebrow">Inspector</div>
                          <div class="panel-header-title-row">
                            <Show
                              when={editingNodeId() === node().id}
                              fallback={<h3>{node().label}</h3>}
                            >
                              <input
                                class="text-input inspector-title-input"
                                value={nodeLabelDraft()}
                                onInput={(event) => setNodeLabelDraft(event.currentTarget.value)}
                                onBlur={handleCommitNodeLabel}
                                onKeyDown={(event) => {
                                  if (event.key === "Enter") {
                                    handleCommitNodeLabel();
                                    return;
                                  }
                                  if (event.key === "Escape") {
                                    handleCancelNodeLabelEdit();
                                  }
                                }}
                                aria-label="Node label"
                                autofocus
                              />
                            </Show>
                            <div class="panel-header-actions">
                              <button
                                class="inspector-action-button"
                                onClick={() => handleStartNodeLabelEdit(node().id, node().label)}
                                title="Rename node"
                                aria-label={`Rename ${node().label}`}
                              >
                                <SidebarIcon name="edit" />
                              </button>
                              <button
                                class="inspector-delete-button"
                                onClick={handleDeleteSelectedNode}
                                title="Delete node"
                                aria-label={`Delete ${node().label}`}
                              >
                                <SidebarIcon name="trash" />
                              </button>
                            </div>
                          </div>
                        </div>
                      </div>

                      <AgentConfigForm
                        model={node().agent.model}
                        onModelChange={(value) =>
                          updateCurrentNode((nextNode) => {
                            nextNode.agent.model = value;
                          })
                        }
                        autoStart={node().agent.auto_start}
                        onAutoStartChange={(value) =>
                          updateCurrentNode((nextNode) => {
                            nextNode.agent.auto_start = value;
                          })
                        }
                        systemPrompt={node().agent.system_prompt}
                        onSystemPromptChange={(value) =>
                          updateCurrentNode((nextNode) => {
                            nextNode.agent.system_prompt = value;
                          })
                        }
                        taskPrompt={node().agent.task_prompt}
                        onTaskPromptChange={(value) =>
                          updateCurrentNode((nextNode) => {
                            nextNode.agent.task_prompt = value;
                          })
                        }
                        schemaJson={schemaText()}
                        onSchemaChange={(value) => setSchemaText(value)}
                        knownModels={activeProfileMemo().known_models}
                        defaultModel={activeProfileMemo().default_model}
                        listId="node-model-list"
                        systemPromptRows={8}
                        taskPromptRows={5}
                        schemaRows={14}
                      />

                      <ToolConfigEditor
                        config={node().agent.tools}
                        onToolEnabledChange={(toolName, enabled) =>
                          updateCurrentNodeToolConfig((tools) => {
                            setToolEnabled(tools, toolName, enabled);
                          })
                        }
                        onApprovalModeChange={(value) =>
                          updateCurrentNodeToolConfig((tools) => {
                            tools.approvalMode = value;
                          })
                        }
                        onMaxToolRoundsChange={(value) =>
                          updateCurrentNodeToolConfig((tools) => {
                            tools.maxToolRounds = Math.min(32, Math.max(1, value));
                          })
                        }
                      />

                      <div class="button-row">
                        <button class="secondary-button" onClick={applySchemaEditor}>
                          Apply schema
                        </button>
                      </div>
                    </>
                  )}
                </Show>
              </aside>
            </div>
            <section class="dock-panel" classList={{ collapsed: !dockOpen() }}>
              <div
                class="dock-resize-zone"
                onPointerDown={handleDockResizePointerDown}
                role="separator"
                aria-orientation="horizontal"
                aria-label="Resize bottom panel"
              />
              <div class="dock-tabs">
                <div class="dock-tab-switcher">
                  <button classList={{ active: bottomTab() === "overview" }} onClick={() => handleSelectBottomTab("overview")}>
                    Overview
                  </button>
                  <button classList={{ active: bottomTab() === "chat" }} onClick={() => handleSelectBottomTab("chat")}>
                    Chat
                  </button>
                  <button classList={{ active: bottomTab() === "trace" }} onClick={() => handleSelectBottomTab("trace")}>
                    Run trace
                  </button>
                </div>
                <Show when={bottomTab() === "trace" && hasRunTraceMemo() && dockOpen()}>
                  <div class="dock-tab-actions">
                    <button class="secondary-button small ghost dock-trace-action" onClick={() => void handleClearRunTrace()}>
                      Clear trace
                    </button>
                  </div>
                </Show>
              </div>
              <Show when={dockOpen()}>
                <Show
                  when={bottomTab() === "overview"}
                  fallback={
                    <Show
                      when={bottomTab() === "chat"}
                      fallback={
                        <div class="trace-layout">
                          <div class="trace-list">
                            <For each={runState()?.runTrace ?? []}>
                              {(entry, index) => (
                                <button
                                  class="trace-row"
                                  classList={{ active: selectedTraceIndex() === index() }}
                                  onClick={() => setSelectedTraceIndex(index())}
                                >
                                  <span class={`trace-pill ${entry.status}`}>{entry.status.replace("_", " ")}</span>
                                  <div>
                                    <strong>{entry.nodeLabel}</strong>
                                    <div>{entry.message}</div>
                                  </div>
                                </button>
                              )}
                            </For>
                          </div>
                          <div class="trace-detail">
                            <Show when={selectedTrace()} fallback={<div class="empty-panel">Select a trace entry.</div>}>
                              {(entry) => (
                                <>
                                  <div class="eyebrow">Trace detail</div>
                                  <h3>{entry().nodeLabel}</h3>
                                  <p>{entry().message}</p>
                                  <pre>{entry().output ? prettyJson(entry().output) : "No output recorded."}</pre>
                                </>
                              )}
                            </Show>
                          </div>
                        </div>
                      }
                    >
                      <div class="chat-layout">
                        <div class="chat-history" ref={(el) => { chatHistoryRef = el; }}>
                          <Show when={chatMessages().length > 0} fallback={<div class="empty-panel">Run a workflow or select a paused node to continue.</div>}>
                            <For each={chatMessages()}>
                              {(message) => (
                                <div class={`chat-row role-${message.role.toLowerCase()}`}>
                                  <div class="chat-role" classList={{ "is-system": message.role === "System" }}>
                                    {chatRoleLabel(message.role, currentNode()?.label)}
                                  </div>
                                  <pre>{message.content}</pre>
                                </div>
                              )}
                            </For>
                          </Show>
                        </div>
                        <Show when={selectedPendingApproval()}>
                          {(approval) => (
                            <div class="inspector-card">
                              <div class="eyebrow">Approval required</div>
                              <h3>{approval().toolCall.name}</h3>
                              <p>{approval().nodeLabel}</p>
                              <pre>{prettyJson(approval().toolCall.arguments)}</pre>
                              <div class="inspector-actions">
                                <button class="secondary-button" onClick={() => void handleToolApproval(false)}>
                                  Deny
                                </button>
                                <button class="primary-button" onClick={() => void handleToolApproval(true)}>
                                  Approve
                                </button>
                              </div>
                            </div>
                          )}
                        </Show>
                        <div class="chat-composer">
                          <div class="chat-composer-pill">
                            <textarea
                              class="text-area composer-input"
                              rows={1}
                              value={chatInput()}
                              onInput={(event) => setChatInput(event.currentTarget.value)}
                              onKeyDown={handleChatInputKeyDown}
                              placeholder={
                                selectedPendingApproval()
                                  ? "Resolve the pending tool approval above."
                                  : "Continue paused node. Prefix /brainstorming for a skill."
                              }
                              disabled={!chatEnabledMemo() || !!selectedPendingApproval()}
                            />
                            <Show when={chatSubmission().invokedSkills.length > 0}>
                              <span
                                class="composer-skill-pill"
                                title={`Sending with skills: ${chatSubmission().invokedSkills.map((skill) => `/${skill}`).join(", ")}`}
                              >
                                {chatSubmission().invokedSkills.map((skill) => `/${skill}`).join(", ")}
                              </span>
                            </Show>
                            <button
                              class="primary-button composer-send-button"
                              onClick={() => void handleSubmitChat()}
                              disabled={!canSendChatMemo()}
                              title="Send to paused node"
                              aria-label="Send to paused node"
                            >
                              <ArrowUp class="composer-send-icon" aria-hidden="true" absoluteStrokeWidth strokeWidth={2.3} />
                            </button>
                          </div>
                        </div>
                      </div>
                    </Show>
                  }
                >
                  <div class="overview-layout">
                    <div class="overview-feed">
                      <Show when={(runState()?.runTrace?.length ?? 0) > 0} fallback={<div class="empty-panel">No workflow runs yet.</div>}>
                        <For each={runState()?.runTrace ?? []}>
                          {(entry) => (
                            <div class="overview-entry">
                              <div class="overview-node-label">{entry.nodeLabel}</div>
                              <div class="overview-status">{entry.status.replace("_", " ")}</div>
                              <div class="overview-message">{entry.message}</div>
                              <Show when={entry.output}>
                                <pre class="overview-output">{prettyJson(entry.output)}</pre>
                              </Show>
                            </div>
                          )}
                        </For>
                      </Show>
                    </div>
                  </div>
                </Show>
              </Show>
            </section>
          </div>
        </Show>
      </main>
    </div>
  );
}

function normalizeError(error: unknown) {
  if (error instanceof Error) {
    return error.message;
  }
  if (typeof error === "string") {
    return error;
  }
  return JSON.stringify(error);
}

function viewportHeight() {
  return typeof globalThis.innerHeight === "number" ? globalThis.innerHeight : 900;
}
function minimumDockHeight(tab: BottomTab) {
  return tab === "chat" ? 116 : tab === "overview" ? 168 : 168;
}

function clampDockHeight(height: number, tab: BottomTab, nextViewportHeight = viewportHeight()) {
  const min = minimumDockHeight(tab);
  const max = Math.max(min, nextViewportHeight - DOCK_VIEWPORT_MARGIN);
  return Math.min(Math.max(Math.round(height), min), max);
}

function shouldCollapseDock(height: number, tab: BottomTab) {
  return height <= Math.max(COLLAPSED_DOCK_HEIGHT + 16, minimumDockHeight(tab) - 32);
}

function chatRoleLabel(role: ChatRole, nodeLabel: string | null | undefined) {
  switch (role) {
    case "System":
      return "System";
    case "Thinking":
    case "Assistant":
      return nodeLabel?.trim() || "Node";
    case "User":
      return "You";
  }
}

function isTextInputTarget(target: EventTarget | null) {
  if (!(target instanceof HTMLElement)) {
    return false;
  }
  return ["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName) || target.isContentEditable;
}

function isMacOS() {
  return typeof navigator === "object" && /Mac/i.test(navigator.userAgent);
}

export default App;
