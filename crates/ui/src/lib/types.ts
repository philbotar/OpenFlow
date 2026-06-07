export type NodeId = string;
export type WorkflowId = string;
export type EdgeId = string;

export type BottomTab = "overview" | "chat" | "trace";
export type Screen = "editor" | "settings" | "agents";

export interface Workflow {
  id: WorkflowId;
  name: string;
  nodes: Node[];
  edges: Edge[];
}

export interface Node {
  id: NodeId;
  label: string;
  kind: "Agent";
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
  maxToolRounds: number;
}

export interface AgentNodeConfig {
  system_prompt: string;
  task_prompt: string;
  model: string;
  output_schema: unknown;
  auto_start: boolean;
  tools: NodeToolConfig;
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

export type ChatRole = "System" | "Thinking" | "User" | "Assistant";

export interface ChatMessage {
  role: ChatRole;
  content: string;
  toolCallId?: string;
}

export type AgentStatus =
  | "idle"
  | "queued"
  | "started"
  | "awaiting_input"
  | "awaiting_tool_approval"
  | "running_tool"
  | "completed"
  | "failed";

export type SubagentStatus = "declared" | "active" | "completed" | "failed";

export interface SubagentSummary {
  id: string;
  name: string;
  purpose: string;
  status: SubagentStatus;
}

export type TraceStatus = "queued" | "running" | "paused" | "failed" | "completed";

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
  kind: "Queued" | "Started" | "Completed" | "Failed";
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

export interface WorkflowRunState {
  active: boolean;
  awaitingNodeId: NodeId | null;
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
  key_ref: string;
  editable: boolean;
}

export interface AppSettings {
  active_provider: ProviderId;
  providers: Record<ProviderId, ProviderProfile>;
  skill_search_paths?: string[];
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
  skills: SkillSummary[];
  settings: AppSettings;
  runState: WorkflowRunState | null;
}
