export type NodeId = string;
export type WorkflowId = string;
export type EdgeId = string;

export type BottomTab = "overview" | "chat" | "trace";
export type Screen = "editor" | "settings" | "agents";

export interface RetryPolicy {
  max_attempts: number;
  backoff_ms: number;
}

export interface WorkflowSchedule {
  cron: string;
  enabled: boolean;
  timezone: string;
}

export interface WorkflowSettings {
  shared_context: string;
  schedule?: WorkflowSchedule | null;
  retry_policy?: RetryPolicy;
  provider_id?: string | null;
}

export interface ProjectMetadata {
  description: string;
}

export interface Project {
  id: string;
  path: string;
  name: string;
  metadata: ProjectMetadata;
  workflow_ids: string[];
  default_execution_cwd: string;
}

export interface Workflow {
  id: WorkflowId;
  name: string;
  nodes: Node[];
  edges: Edge[];
  settings: WorkflowSettings;
}

export interface Node {
  id: NodeId;
  label: string;
  kind: "agent" | "Agent";
  position: NodePosition;
  agent: AgentNodeConfig;
}

export interface NodePosition {
  x: number;
  y: number;
}

export type ToolTier = "read" | "write" | "exec";
export type ToolConcurrency = "shared" | "exclusive";
export type ToolPolicy = "allow" | "prompt" | "deny";
export type ApprovalMode = "always_ask" | "write" | "yolo";

export interface ToolRef {
  name: string;
  tier?: ToolTier | null;
}

export interface ToolCatalogSelection {
  tools: ToolRef[];
}

export interface ToolPolicyOverride {
  toolName: string;
  policy: ToolPolicy;
  timeoutSecs: number | null;
}

export interface NodeToolConfig {
  catalog: ToolCatalogSelection;
  approvalMode: ApprovalMode | null;
  overrides: ToolPolicyOverride[];
}

export interface AgentNodeConfig {
  system_prompt: string;
  task_prompt: string;
  model: string;
  output_schema: unknown;
  auto_start: boolean;
  tools: NodeToolConfig;
  callable_agents: string[];
  allow_all_callable_agents: boolean;
  reasoning_effort?: string | null;
  reasoning_budget_tokens?: number | null;
  /** Wire alias from persisted workflow JSON. */
  reasoningEffort?: string | null;
  /** Wire alias from persisted workflow JSON. */
  reasoningBudgetTokens?: number | null;
}

export interface ReasoningEffortOption {
  value: string;
  label: string;
  uses_budget_tokens: boolean;
}

export interface AgentDefinition {
  id: string;
  name: string;
  system_prompt: string;
  task_prompt: string;
  model: string;
  output_schema: unknown;
  auto_start: boolean;
  tools: NodeToolConfig;
}

export interface Edge {
  id: EdgeId;
  from: NodeId;
  to: NodeId;
}

export type ChatRole =
  | "system"
  | "thinking"
  | "user"
  | "assistant"
  | "System"
  | "Thinking"
  | "User"
  | "Assistant";

export type ChatMessageKind = "node_completed";

export interface ChatMessage {
  role: ChatRole;
  content: string;
  id?: string;
  streaming?: boolean;
  toolCallId?: string;
  messageKind?: ChatMessageKind;
}

export type AgentStatus =
  | "idle"
  | "queued"
  | "started"
  | "awaiting_input"
  | "awaiting_tool_approval"
  | "running_tool"
  | "completed"
  | "failed"
  | "interrupted"
  | "stopped";

export type SubagentStatus = "declared" | "active" | "completed" | "failed";

export interface SubagentSummary {
  id: string;
  name: string;
  purpose: string;
  status: SubagentStatus;
}

export type TraceStatus =
  | "queued"
  | "running"
  | "paused"
  | "failed"
  | "stopped"
  | "completed";

export interface RunTraceEntry {
  nodeId: NodeId;
  nodeLabel: string;
  status: TraceStatus;
  message: string;
  output: unknown | null;
}

export type ToolCallStatus =
  | "proposed"
  | "awaiting_approval"
  | "running"
  | "completed"
  | "blocked"
  | "failed"
  | "aborted";

export interface ToolCall {
  id: string;
  name: string;
  arguments: unknown;
  intent: string | null;
}

export interface PendingToolApproval {
  approvalId: string;
  nodeId: NodeId;
  nodeLabel: string;
  toolCall: ToolCall;
  tier: ToolTier;
}

export interface ToolCallSummary {
  toolCallId: string;
  toolName: string;
  status: ToolCallStatus;
  arguments: unknown;
  lastOutput: string | null;
  isError: boolean;
}

export interface ToolArtifactSummary {
  artifactId: string;
  toolName: string;
  path: string;
  sizeBytes: number;
}

export interface RunEvent {
  node_id: NodeId;
  kind:
    | "queued"
    | "started"
    | "retrying"
    | "completed"
    | "failed"
    | "Queued"
    | "Started"
    | "Retrying"
    | "Completed"
    | "Failed";
  message: string;
  output: unknown | null;
}

export interface NodeRunOutput {
  node_id: NodeId;
  output: unknown;
}

export interface RunReport {
  workflow_id: WorkflowId;
  events: RunEvent[];
  outputs: NodeRunOutput[];
}

export type FileChangeOp = "create" | "update" | "delete" | "rename";

export interface FileEditPreviewEntry {
  path: string;
  op: FileChangeOp;
  diff: string;
  renameTo?: string | null;
}

export interface FileEditPreview {
  entries: FileEditPreviewEntry[];
  error?: string;
}

export interface FileChangeRecord {
  path: string;
  op: FileChangeOp;
  renameTo?: string | null;
  diffSummary?: string | null;
  batchId?: string | null;
  timestampMs: number;
}

export interface FileSnapshot {
  path: string;
  existed: boolean;
  content?: string | null;
}

export interface EditBatch {
  batchId: string;
  nodeId: string;
  toolCallId: string;
  toolName: string;
  timestampMs: number;
  snapshots: FileSnapshot[];
}

export interface WorkflowRunState {
  active: boolean;
  awaitingNodeId: NodeId | null;
  awaitingNodeIds?: NodeId[];
  activeManualNodeId: NodeId | null;
  activeToolCallId: string | null;
  pendingApprovals: PendingToolApproval[];
  toolCallsByNode: Record<NodeId, ToolCallSummary[]>;
  toolArtifacts: Record<string, ToolArtifactSummary>;
  execApprovalGranted: boolean;
  statusByNode: Record<NodeId, AgentStatus>;
  subagentsByNode: Record<NodeId, SubagentSummary[]>;
  lastReport: RunReport | null;
  lastError: string | null;
  chatLogs: Record<NodeId, ChatMessage[]>;
  runTrace: RunTraceEntry[];
  outputs: Record<NodeId, unknown>;
  changedFiles: FileChangeRecord[];
  changedFilesByNode: Record<NodeId, FileChangeRecord[]>;
  editBatches: EditBatch[];
}

export type ProviderId = string;
export type AiProviderKind = ProviderId;
export type ProviderTransport = "responses" | "chat_completions";

export interface ProviderProfile {
  display_name: string;
  base_url: string;
  transport: ProviderTransport;
  responses_path: string;
  chat_completions_path: string;
  known_models: string[];
  default_model: string | null;
  editable: boolean;
  reasoning_effort_options?: ReasoningEffortOption[];
  default_reasoning_budget_tokens?: Record<string, number>;
  default_reasoning_effort?: string | null;
  /** Wire aliases from persisted settings JSON. */
  reasoningEffortOptions?: ReasoningEffortOption[];
  defaultReasoningBudgetTokens?: Record<string, number>;
  defaultReasoningEffort?: string | null;
}

export interface LspSettings {
  enabled: boolean;
  format_on_write: boolean;
  diagnostics_on_write: boolean;
}

export interface AppSettings {
  active_provider: ProviderId;
  providers: Record<ProviderId, ProviderProfile>;
  skill_search_paths?: string[];
  lsp?: LspSettings;
}

export interface SkillSummary {
  id: string;
  name: string;
  description: string;
  path?: string;
}

export interface WorkflowListItem {
  id: string;
  name: string;
}

export interface AgentDefinitionSummary {
  id: string;
  name: string;
  model: string;
}

export interface ProviderReadiness {
  ready: boolean;
  provider: string;
  message: string;
  envVar: string;
}

export interface WorkflowValidationSummary {
  layerCount: number;
  layers: string[][];
}

export interface BootstrapPayload {
  workflows: Workflow[];
  agents: AgentDefinition[];
  projects?: Project[];
  skills: SkillSummary[];
  settings: AppSettings;
  runState: WorkflowRunState | null;
}
