import { createContext, useContext } from "solid-js";
import type { Accessor, Setter } from "solid-js";
import type {
  AgentDefinition,
  AiProviderKind,
  AppSettings,
  BottomTab,
  EdgeId,
  NodeId,
  ProviderProfile,
  ProviderReadiness,
  RunTraceEntry,
  Screen,
  Project,
  SkillSummary,
  Workflow,
  WorkflowRunState,
} from "../lib/types";
import type { ResolvedTheme, ThemePreference } from "../lib/theme";
import type { ChatSubmissionResolution } from "../lib/chatCommands";
import type {
  ChatLayoutProjection,
  WorkflowCanvasGraph,
  WorkflowCanvasStatusByNode,
  WorkflowCanvasSubagentsByNode,
} from "../lib/workflow";

export interface AppContextValue {
  // ── Signal accessors ──────────────────────────────────────────────────────
  workflows: Accessor<Workflow[]>;
  projects: Accessor<Project[]>;
  agents: Accessor<AgentDefinition[]>;
  activeWorkflowId: Accessor<string | null>;
  selectedNodeId: Accessor<NodeId | null>;
  selectedEdgeId: Accessor<EdgeId | null>;
  screen: Accessor<Screen>;
  settings: Accessor<AppSettings>;
  runState: Accessor<WorkflowRunState | null>;
  readiness: Accessor<ProviderReadiness | null>;
  bottomTab: Accessor<BottomTab>;
  dockOpen: Accessor<boolean>;
  dockHeight: Accessor<number>;
  selectedTraceIndex: Accessor<number | null>;
  schemaText: Accessor<string>;
  newModelInputByProvider: Accessor<Record<AiProviderKind, string>>;
  providerKeyInputByProvider: Accessor<Record<AiProviderKind, string>>;
  uiZoom: Accessor<number>;
  workflowSettingsOpen: Accessor<boolean>;
  selectedProjectId: Accessor<string | null>;
  editingWorkflowId: Accessor<string | null>;
  workflowNameDraft: Accessor<string>;
  selectedAgentId: Accessor<string | null>;
  editingAgentId: Accessor<string | null>;
  agentNameDraft: Accessor<string>;
  editingNodeId: Accessor<NodeId | null>;
  nodeLabelDraft: Accessor<string>;
  agentSchemaDraft: Accessor<string>;
  addNodePickerOpen: Accessor<boolean>;
  assignWorkflowPickerProjectId: Accessor<string | null>;
  isMaximized: Accessor<boolean>;
  availableSkills: Accessor<SkillSummary[]>;
  skillById: Accessor<Map<string, SkillSummary>>;
  appReady: Accessor<boolean>;
  startingRun: Accessor<boolean>;
  themePreference: Accessor<ThemePreference>;
  resolvedTheme: Accessor<ResolvedTheme>;
  shortcutsModalOpen: Accessor<boolean>;
  chatFilterNodeId: Accessor<NodeId | null>;
  chatFocusNode: Accessor<{ nodeId: NodeId; tick: number } | null>;
  pickedLiveNodeId: Accessor<NodeId | null>;

  // ── Signal setters (form inputs + simple UI state) ────────────────────────
  setWorkflowNameDraft: Setter<string>;
  setAgentNameDraft: Setter<string>;
  setChatFilterNodeId: Setter<NodeId | null>;
  setPickedLiveNodeId: Setter<NodeId | null>;
  setChatDraft: (nodeId: NodeId, text: string) => void;
  setNewModelInputByProvider: Setter<Record<AiProviderKind, string>>;
  setProviderKeyInputByProvider: Setter<Record<AiProviderKind, string>>;
  setNodeLabelDraft: Setter<string>;
  setSchemaText: Setter<string>;
  setSelectedTraceIndex: Setter<number | null>;
  setSelectedAgentId: Setter<string | null>;
  setScreen: Setter<Screen>;

  // ── Derived memos ─────────────────────────────────────────────────────────
  activeWorkflow: Accessor<Workflow | undefined>;
  activeProject: Accessor<Project | undefined>;
  independentWorkflows: Accessor<Workflow[]>;
  executionCwdForActiveWorkflow: Accessor<string | null>;
  selectedAgent: Accessor<AgentDefinition | null>;
  canvasGraph: Accessor<WorkflowCanvasGraph | null>;
  canvasStatusByNode: Accessor<WorkflowCanvasStatusByNode | null>;
  canvasSubagentsByNode: Accessor<WorkflowCanvasSubagentsByNode | null>;
  currentNode: Accessor<Workflow["nodes"][number] | undefined>;
  activeProfileMemo: Accessor<ProviderProfile>;
  providerIdsMemo: Accessor<string[]>;
  activeProviderKeyInput: Accessor<string>;
  selectedTrace: Accessor<RunTraceEntry | null>;
  hasRunTraceMemo: Accessor<boolean>;
  currentNodeOutput: Accessor<unknown>;
  chatLayout: Accessor<ChatLayoutProjection>;
  chatDraft: (nodeId: NodeId) => string;
  chatSubmissionFor: (nodeId: NodeId) => ChatSubmissionResolution;
  canSendChatFor: (nodeId: NodeId) => boolean;
  composerBusyFor: (nodeId: NodeId) => boolean;

  // ── Ref setters ───────────────────────────────────────────────────────────
  setWorkflowNameInputRef: (el: HTMLInputElement | undefined) => void;
  setAgentNameInputRef: (el: HTMLInputElement | undefined) => void;

  // ── Workflow handlers ─────────────────────────────────────────────────────
  handleSwitchWorkflow: (workflowId: string) => void;
  handleCreateWorkflow: (projectId?: string) => Promise<void>;
  handleOpenAssignWorkflowPicker: (projectId: string) => void;
  closeAssignWorkflowPicker: () => void;
  workflowsAddableToProject: (projectId: string) => Workflow[];
  handleAssignWorkflowToProject: (projectId: string, workflowId: string) => Promise<void>;
  handleOpenAgents: () => void;
  handleAddProject: () => Promise<void>;
  handleSelectProject: (projectId: string) => void;
  handleToggleProjectExpanded: (projectId: string) => void;
  isProjectExpanded: (projectId: string) => boolean;
  workflowsForProject: (project: Project) => Workflow[];

  // ── Agent handlers ────────────────────────────────────────────────────────
  handleCreateAgent: () => Promise<void>;
  handleSaveAgents: () => Promise<void>;
  handleAgentSchemaInput: (text: string) => void;
  updateSelectedAgent: (mutator: (draft: AgentDefinition) => void) => void;
  handleStartAgentNameEdit: (agentId: string, currentName: string) => void;
  handleCancelAgentNameEdit: () => void;
  handleAgentNameCommit: () => void;
  handleAgentNameKeyDown: (event: KeyboardEvent) => void;

  // ── Settings handlers ─────────────────────────────────────────────────────
  handleSaveSettings: () => Promise<void>;
  handleAddKnownModel: () => void;
  handleRemoveKnownModel: (model: string) => void;
  handleApiKeyInput: (key: string) => void;
  updateSettings: (mutator: (draft: AppSettings) => void) => Promise<void>;

  // ── Canvas / graph handlers ───────────────────────────────────────────────
  handleSelectNode: (nodeId: NodeId | null) => void;
  handleSelectEdge: (edgeId: EdgeId | null) => void;
  handleCanvasNodePosition: (nodeId: NodeId, x: number, y: number) => void;
  handleCreateEdge: (from: NodeId, to: NodeId) => void;
  handleReconnectEdge: (edgeId: EdgeId, from: NodeId, to: NodeId) => void;
  handleDeleteEdge: (edgeId: EdgeId) => void;
  handleDeleteSelectedNode: () => void;
  handleOpenAddNodePicker: () => void;
  handleAddNode: (agentId: string | null) => Promise<void>;
  closeAddNodePicker: () => void;

  // ── Run handlers ──────────────────────────────────────────────────────────
  handleValidate: () => Promise<void>;
  handleRun: () => Promise<void>;
  handleStopRun: () => Promise<void>;
  handleInterruptNode: (nodeId: NodeId) => Promise<void>;
  handleRetryNode: (nodeId: NodeId) => Promise<void>;
  stoppingRun: Accessor<boolean>;
  handleSetThemePreference: (preference: ThemePreference) => void;
  openShortcutsModal: () => void;
  closeShortcutsModal: () => void;
  handleClearRunTrace: () => Promise<void>;
  handleSubmitChat: (nodeId: NodeId) => Promise<void>;
  handleRefreshSkills: () => Promise<void>;
  handleToolApproval: (approvalId: string, allow: boolean) => Promise<void>;

  // ── Node label edit handlers ──────────────────────────────────────────────
  handleStartNodeLabelEdit: (nodeId: NodeId, currentLabel: string) => void;
  handleCancelNodeLabelEdit: () => void;
  handleCommitNodeLabel: () => void;

  // ── Workflow name edit handlers ───────────────────────────────────────────
  handleStartWorkflowNameEdit: (workflowId: string, currentName: string) => void;
  handleCancelWorkflowNameEdit: () => void;
  handleWorkflowNameCommit: () => void;
  handleWorkflowNameKeyDown: (event: KeyboardEvent) => void;

  // ── Input / keyboard handlers ─────────────────────────────────────────────
  handleChatInputKeyDown: (event: KeyboardEvent, nodeId: NodeId) => void;

  // ── Workflow settings handlers ────────────────────────────────────────────
  handleToggleWorkflowSettings: () => void;
  updateActiveWorkflowSettings: (mutator: (settings: Workflow["settings"]) => void) => void;

  // ── Node mutation helpers ─────────────────────────────────────────────────
  updateCurrentNode: (mutator: (node: Workflow["nodes"][number]) => void) => void;
  updateCurrentNodeToolConfig: (
    mutator: (tools: Workflow["nodes"][number]["agent"]["tools"]) => void,
  ) => void;
  setToolEnabled: (
    tools: { catalog: { tools: { name: string }[] } },
    toolName: string,
    enabled: boolean,
  ) => void;
  applySchemaEditor: () => boolean;
  persistAll: (successText?: string) => Promise<boolean>;

  // ── Dock handlers ─────────────────────────────────────────────────────────
  handleSelectBottomTab: (tab: BottomTab) => void;
  handleDockResizePointerDown: (event: PointerEvent) => void;

  // ── Zoom handlers ─────────────────────────────────────────────────────────
  handleZoomIn: () => void;
  handleZoomOut: () => void;
  handleZoomReset: () => void;
}

export const AppContext = createContext<AppContextValue | undefined>(undefined);

export function useAppContext(): AppContextValue {
  const ctx = useContext(AppContext);
  if (!ctx) {
    throw new Error("useAppContext must be used inside <AppProvider>");
  }
  return ctx;
}
