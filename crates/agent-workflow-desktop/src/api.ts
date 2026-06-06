import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type {
  AgentDefinition,
  AgentDefinitionSummary,
  AppSettings,
  BootstrapPayload,
  Node,
  ProviderReadiness,
  Workflow,
  WorkflowListItem,
  WorkflowRunState,
  WorkflowValidationSummary,
} from "./types";

export const RUN_STATE_EVENT = "run-state";

export function bootstrapApp() {
  return invoke<BootstrapPayload>("bootstrap_app");
}

export function listWorkflows() {
  return invoke<WorkflowListItem[]>("list_workflows");
}

export function loadAllWorkflows() {
  return invoke<Workflow[]>("load_all_workflows");
}

export function loadWorkflow(workflowId: string) {
  return invoke<Workflow>("load_workflow", { workflowId });
}

export function createWorkflow(name: string) {
  return invoke<Workflow>("create_workflow", { name });
}

export function saveWorkflow(workflow: Workflow) {
  return invoke<Workflow>("save_workflow", { workflow });
}

export function saveWorkflows(workflows: Workflow[]) {
  return invoke<void>("save_workflows", { workflows });
}

export function renameWorkflow(workflowId: string, name: string) {
  return invoke<WorkflowListItem>("rename_workflow", { workflowId, name });
}

export function listAgents() {
  return invoke<AgentDefinitionSummary[]>("list_agents");
}

export function loadAgents() {
  return invoke<AgentDefinition[]>("load_agents");
}

export function createAgentDefinition(name: string) {
  return invoke<AgentDefinition>("create_agent_definition", { name });
}

export function saveAgents(agents: AgentDefinition[]) {
  return invoke<void>("save_agents", { agents });
}

export function loadSettings() {
  return invoke<AppSettings>("load_settings");
}

export function saveSettings(settings: AppSettings) {
  return invoke<void>("save_settings", { settings });
}

export function loadProviderApiKey(providerId: string) {
  return invoke<string | null>("load_provider_api_key", { providerId });
}

export function saveProviderApiKey(providerId: string, apiKey: string) {
  return invoke<void>("save_provider_api_key", { providerId, apiKey });
}

export function deleteProviderApiKey(providerId: string) {
  return invoke<void>("delete_provider_api_key", { providerId });
}


export function resolveProviderReadiness(settings: AppSettings, transientApiKey: string | null = null) {
  return invoke<ProviderReadiness>("resolve_provider_readiness", {
    settings,
    transientApiKey,
  });
}

export function validateWorkflow(workflow: Workflow) {
  return invoke<WorkflowValidationSummary>("validate_workflow", { workflow });
}

export function createAgentNode(index: number, x: number, y: number, agentId: string | null = null) {
  return invoke<Node>("create_agent_node", { index, x, y, agentId });
}

export function startRun(
  workflow: Workflow,
  settings: AppSettings,
  transientApiKey: string | null = null,
) {
  return invoke<WorkflowRunState>("start_run", {
    workflow,
    settings,
    transientApiKey,
  });
}

export function submitUserInput(nodeId: string, text: string) {
  return invoke<WorkflowRunState>("submit_user_input", { nodeId, text });
}

export function submitToolApproval(approvalId: string, allow: boolean) {
  return invoke<WorkflowRunState>("submit_tool_approval", { approvalId, allow });
}

export function completeManualNode() {
  return invoke<WorkflowRunState>("complete_manual_node");
}

export function getRunState() {
  return invoke<WorkflowRunState | null>("get_run_state");
}

export function clearRunTrace() {
  return invoke<WorkflowRunState | null>("clear_run_trace");
}

export function listenToRunState(handler: (runState: WorkflowRunState) => void) {
  return listen<WorkflowRunState>(RUN_STATE_EVENT, (event) => handler(event.payload));
}
