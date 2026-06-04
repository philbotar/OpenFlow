export type NodeId = string;
export type WorkflowId = string;
export type EdgeId = string;

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

export interface AgentNodeConfig {
  system_prompt: string;
  task_prompt: string;
  model: string;
  output_schema: unknown;
  auto_start: boolean;
}

export interface AgentDefinition {
  id: string;
  name: string;
  system_prompt: string;
  task_prompt: string;
  model: string;
  output_schema: unknown;
  auto_start: boolean;
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
}

export type AgentStatus =
  | "idle"
  | "queued"
  | "started"
  | "awaiting_input"
  | "completed"
  | "failed";

export type TraceStatus = "queued" | "running" | "paused" | "failed" | "completed";

export interface RunTraceEntry {
  nodeId: NodeId;
  nodeLabel: string;
  status: TraceStatus;
  message: string;
  output: unknown | null;
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
  statusByNode: Record<NodeId, AgentStatus>;
  lastReport: RunReport | null;
  lastError: string | null;
  chatLogs: Record<NodeId, ChatMessage[]>;
  runTrace: RunTraceEntry[];
  outputs: Record<NodeId, unknown>;
}

export type AiProviderKind = "open_ai" | "open_ai_compatible";
export type ProviderTransport = "responses" | "chat_completions";

export interface ProviderProfile {
  display_name: string;
  base_url: string;
  transport: ProviderTransport;
  responses_path: string;
  chat_completions_path: string;
  api_key: string;
  known_models: string[];
  default_model: string | null;
}

export interface AppSettings {
  active_provider: AiProviderKind;
  openai: ProviderProfile;
  openai_compatible: ProviderProfile;
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
  settings: AppSettings;
  runState: WorkflowRunState | null;
}
