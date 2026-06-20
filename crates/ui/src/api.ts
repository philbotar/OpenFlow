import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import type { OpenDialogOptions } from "@tauri-apps/plugin-dialog";
import type {
  AgentDefinition,
  AgentDefinitionSummary,
  AppSettings,
  BootstrapPayload,
  Node,
  Project,
  ProjectFileReference,
  ProjectFileReferenceContent,
  CopyWorkflowToProjectResult,
  SkillSummary,
  ProviderReadiness,
  RunSummary,
  Workflow,
  WorkflowListItem,
  WorkflowRunState,
  WorkflowValidationSummary,
  WorkflowAuthoringTurnResult,
  TerminalEvent,
  TerminalStart,
  ScheduleStatus,
} from "./lib/types";

export const RUN_STATE_EVENT = "run-state";
export const TERMINAL_EVENT = "terminal-event";
export const SCHEDULE_EVENT = "schedule-event";

export function bootstrapApp() {
  return invoke<BootstrapPayload>("bootstrap_app");
}

export function listProjects() {
  return invoke<Project[]>("list_projects");
}

export function listProjectFileReferences(
  executionCwd: string,
  query: string | null = null,
  limit: number | null = null,
) {
  return invoke<ProjectFileReference[]>("list_project_file_references", {
    executionCwd,
    query,
    limit,
  });
}

export function readProjectFileReferences(
  executionCwd: string,
  paths: string[],
) {
  return invoke<ProjectFileReferenceContent[]>("read_project_file_references", {
    executionCwd,
    paths,
  });
}

export function saveProjects(projects: Project[]) {
  return invoke<void>("save_projects", { projects });
}

export function createProjectFromDirectory(path: string) {
  return invoke<Project>("create_project_from_directory", { path });
}

export function assignWorkflowToProject(projectId: string, workflowId: string) {
  return invoke<Project[]>("assign_workflow_to_project", { projectId, workflowId });
}

export function copyWorkflowToProject(targetProjectId: string, sourceWorkflowId: string) {
  return invoke<CopyWorkflowToProjectResult>("copy_workflow_to_project", {
    targetProjectId,
    sourceWorkflowId,
  });
}

export function unassignWorkflowFromProject(projectId: string, workflowId: string) {
  return invoke<Project[]>("unassign_workflow_from_project", { projectId, workflowId });
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

export function listSkills() {
  return invoke<SkillSummary[]>("list_skills");
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

export function startWorkflowAuthoring(baseWorkflow: Workflow | null = null) {
  return invoke<string>("start_workflow_authoring", { baseWorkflow });
}

export function workflowAuthoringTurn(
  sessionId: string,
  message: string,
  settings: AppSettings,
  transientApiKey: string | null = null,
) {
  return invoke<WorkflowAuthoringTurnResult>("workflow_authoring_turn", {
    sessionId,
    message,
    settings,
    transientApiKey,
  });
}

export function createAgentNode(index: number, x: number, y: number, agentId: string | null = null) {
  return invoke<Node>("create_agent_node", { index, x, y, agentId });
}

export function startRun(
  workflow: Workflow,
  settings: AppSettings,
  executionCwd: string | null = null,
  transientApiKey: string | null = null,
  entrypoint: string | null = null,
) {
  return invoke<WorkflowRunState>("start_run", {
    workflow,
    settings,
    executionCwd,
    transientApiKey,
    entrypoint,
  });
}

export function stopRun() {
  return invoke<WorkflowRunState>("stop_run");
}

export function continueRun(
  workflow: Workflow,
  settings: AppSettings,
  transientApiKey: string | null = null,
) {
  return invoke<WorkflowRunState>("continue_run", {
    workflow,
    settings,
    transientApiKey,
  });
}

export function isRunContinuable() {
  return invoke<boolean>("is_run_continuable");
}

export function listRuns(workflowId: string | null = null) {
  return invoke<RunSummary[]>("list_runs", { workflowId });
}

export function replayRun(runId: string) {
  return invoke<WorkflowRunState>("replay_run", { runId });
}

export function resumeDurableRun(
  runId: string,
  settings: AppSettings,
  transientApiKey: string | null = null,
) {
  return invoke<WorkflowRunState>("resume_durable_run", {
    runId,
    settings,
    transientApiKey,
  });
}

export function interruptNode(nodeId: string) {
  return invoke<WorkflowRunState>("interrupt_node", { nodeId });
}

export function retryNode(nodeId: string) {
  return invoke<WorkflowRunState>("retry_node", { nodeId });
}

export function previewFileEdit(
  approvalId: string,
  toolName: string,
  toolArguments: unknown,
) {
  return invoke<import("./lib/types").FileEditPreview>("preview_file_edit", {
    approvalId,
    toolName,
    arguments: toolArguments,
  });
}

export function gitDiffFile(path: string) {
  return invoke<string>("git_diff_file", { path });
}

export function revertEditBatch(batchId: string) {
  return invoke<WorkflowRunState>("revert_edit_batch", { batchId });
}

export function submitUserInput(nodeId: string, text: string) {
  return invoke<WorkflowRunState>("submit_user_input", { nodeId, text });
}

export function submitToolApproval(
  approvalId: string,
  allow: boolean,
  reason?: string | null,
) {
  return invoke<WorkflowRunState>("submit_tool_approval", {
    approvalId,
    allow,
    reason: reason ?? null,
  });
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

export function startTerminal(
  cwd: string | null = null,
  cols = 80,
  rows = 24,
) {
  return invoke<TerminalStart>("start_terminal", { cwd, cols, rows });
}

export function writeTerminal(sessionId: string, data: string) {
  return invoke<void>("write_terminal", { sessionId, data });
}

export function resizeTerminal(sessionId: string, cols: number, rows: number) {
  return invoke<void>("resize_terminal", { sessionId, cols, rows });
}

export function stopTerminal(sessionId: string) {
  return invoke<void>("stop_terminal", { sessionId });
}

export function listenToTerminalEvent(handler: (event: TerminalEvent) => void) {
  return listen<TerminalEvent>(TERMINAL_EVENT, (event) => handler(event.payload));
}

export function listenToRunState(handler: (runState: WorkflowRunState) => void) {
  return listen<WorkflowRunState>(RUN_STATE_EVENT, (event) => handler(event.payload));
}

export function listScheduleStatuses() {
  return invoke<ScheduleStatus[]>("list_schedule_statuses");
}

export function refreshSchedules() {
  return invoke<ScheduleStatus[]>("refresh_schedules");
}

export function listenToScheduleStatuses(handler: (statuses: ScheduleStatus[]) => void) {
  return listen<ScheduleStatus[]>(SCHEDULE_EVENT, (event) => handler(event.payload));
}

/** Native app window handle (Tauri seam — do not import @tauri-apps in components). */
export function getAppWindow() {
  return getCurrentWindow();
}

/** Native file/folder picker (Tauri seam). */
export function openNativeDialog(options?: OpenDialogOptions) {
  return openDialog(options);
}
