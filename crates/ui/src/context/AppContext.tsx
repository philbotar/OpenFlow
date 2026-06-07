import { createContext, useContext } from "solid-js";
import type { Accessor, Setter } from "solid-js";
import type {
  AgentDefinition,
  AiProviderKind,
  AppSettings,
  BottomTab,
  ChatMessage,
  EdgeId,
  NodeId,
  PendingToolApproval,
  ProviderProfile,
  ProviderReadiness,
  RunTraceEntry,
  Screen,
  SkillSummary,
  Workflow,
  WorkflowRunState,
} from "../lib/types";
import type { ChatSubmissionResolution } from "../lib/chatCommands";
import type {
  WorkflowCanvasGraph,
  WorkflowCanvasStatusByNode,
  WorkflowCanvasSubagentsByNode,
} from "../lib/workflow";

export interface AppContextValue {
  // ── Signal accessors ──────────────────────────────────────────────────────
  workflows: Accessor<Workflow[]>;
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
  chatInput: Accessor<string>;
  newModelInputByProvider: Accessor<Record<AiProviderKind, string>>;
  providerKeyInputByProvider: Accessor<Record<AiProviderKind, string>>;
  uiZoom: Accessor<number>;
  editingWorkflowId: Accessor<string | null>;
  workflowNameDraft: Accessor<string>;
  selectedAgentId: Accessor<string | null>;
  editingAgentId: Accessor<string | null>;
  agentNameDraft: Accessor<string>;
  editingNodeId: Accessor<NodeId | null>;
  nodeLabelDraft: Accessor<string>;
  agentSchemaDraft: Accessor<string>;
  addNodePickerOpen: Accessor<boolean>;
  isMaximized: Accessor<boolean>;
  availableSkills: Accessor<SkillSummary[]>;
  skillById: Accessor<Map<string, SkillSummary>>;

  // ── Signal setters (form inputs + simple UI state) ────────────────────────
  setWorkflowNameDraft: Setter<string>;
  setChatInput: Setter<string>;
  setNewModelInputByProvider: Setter<Record<AiProviderKind, string>>;
  setProviderKeyInputByProvider: Setter<Record<AiProviderKind, string>>;
  setNodeLabelDraft: Setter<string>;
  setSchemaText: Setter<string>;
  setSelectedTraceIndex: Setter<number | null>;
  setSelectedAgentId: Setter<string | null>;
  setScreen: Setter<Screen>;

  // ── Derived memos ─────────────────────────────────────────────────────────
  activeWorkflow: Accessor<Workflow | undefined>;
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
  chatMessages: Accessor<ChatMessage[]>;
  selectedPendingApproval: Accessor<PendingToolApproval | null>;
  chatEnabledMemo: Accessor<boolean>;
  chatComposerBusyMemo: Accessor<boolean>;
  chatSubmission: Accessor<ChatSubmissionResolution>;
  canSendChatMemo: Accessor<boolean>;

  // ── Ref setters ───────────────────────────────────────────────────────────
  setWorkflowNameInputRef: (el: HTMLInputElement | undefined) => void;

  // ── Workflow handlers ─────────────────────────────────────────────────────
  handleSwitchWorkflow: (workflowId: string) => void;
  handleCreateWorkflow: () => Promise<void>;
  handleOpenAgents: () => void;

  // ── Agent handlers ────────────────────────────────────────────────────────
  handleCreateAgent: () => Promise<void>;
  handleSaveAgents: () => Promise<void>;
  handleAgentSchemaInput: (text: string) => void;
  updateSelectedAgent: (mutator: (draft: AgentDefinition) => void) => void;

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
  handleClearRunTrace: () => Promise<void>;
  handleSubmitChat: () => Promise<void>;
  handleRefreshSkills: () => Promise<void>;
  handleToolApproval: (allow: boolean) => Promise<void>;

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
  handleChatInputKeyDown: (event: KeyboardEvent) => void;

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
