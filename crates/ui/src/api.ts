import { invoke, isTauri } from "@tauri-apps/api/core";
import { getVersion } from "@tauri-apps/api/app";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open as openDialog, confirm as confirmDialog } from "@tauri-apps/plugin-dialog";
import { openPath, openUrl } from "@tauri-apps/plugin-opener";
import type { ConfirmDialogOptions, OpenDialogOptions } from "@tauri-apps/plugin-dialog";
import { relaunch } from "@tauri-apps/plugin-process";
import { check } from "@tauri-apps/plugin-updater";
import type {
  AgentDefinition,
  AgentDefinitionSummary,
  AppSettings,
  BootstrapPayload,
  Chat,
  ChatDeleteResult,
  ChatRunPayload,
  CodexLoginStatus,
  DebugLogEntry,
  DebugLogWrite,
  DurableRunContinuationInput,
  Node,
  Project,
  ProjectFileReference,
  CopyWorkflowToProjectResult,
  McpServerConfig,
  McpCatalogPage,
  McpCapabilityCatalog,
  McpConfigImport,
  McpContextSnapshot,
  McpInstallPreview,
  McpInstallResult,
  McpOAuthStatus,
  McpProbeResult,
  SettingsLoadPayload,
  StagedChatAttachment,
  SkillSummary,
  ProviderReadiness,
  RunSummary,
  Workflow,
  WorkflowListItem,
  WorkflowRunState,
  WorkflowValidationSummary,
  WorkflowAuthoringStartResult,
  WorkflowAuthoringRuntimeConfig,
  WorkflowAuthoringTurnResult,
  WorkflowAuthoringDraftEvent,
  WorkflowAuthoringThinkingEvent,
  UserMessageInput,
  AttachmentPreview,
  TerminalEvent,
  TerminalStart,
  ScheduleDraft,
  ScheduleStatus,
  WorkflowSchedule,
} from "./lib/types";

export const RUN_STATE_EVENT = "run-state";
export const TERMINAL_EVENT = "terminal-event";
export const SCHEDULE_EVENT = "schedule-event";
export const WORKFLOW_AUTHORING_THINKING_EVENT = "workflow-authoring-thinking";
export const WORKFLOW_AUTHORING_DRAFT_EVENT = "workflow-authoring-draft";

export type AppUpdateResult =
  | { status: "current" }
  | { status: "updated" }
  | { status: "unavailable" }
  | { status: "error"; message: string };

export type AppUpdateAvailability = {
  available: boolean;
  version: string | null;
};

export function getAppVersion() {
  if (!isTauri()) {
    return Promise.resolve("dev");
  }
  return getVersion();
}

export function openExternalUrl(url: string): Promise<void> {
  if (!isTauri()) {
    window.open(url, "_blank", "noopener,noreferrer");
    return Promise.resolve();
  }
  return openUrl(url);
}

export function openLocalPath(path: string): Promise<void> {
  if (!isTauri()) return Promise.resolve();
  return openPath(path);
}

export async function checkAppUpdateAvailable(): Promise<AppUpdateAvailability> {
  if (!isTauri()) {
    return { available: false, version: null };
  }
  try {
    const update = await check();
    return { available: update !== null, version: update?.version ?? null };
  } catch {
    return { available: false, version: null };
  }
}

export async function installAppUpdate(): Promise<AppUpdateResult> {
  if (!isTauri()) {
    return { status: "unavailable" };
  }
  try {
    const update = await check();
    if (!update) {
      return { status: "current" };
    }
    await update.downloadAndInstall();
    await relaunch();
    return { status: "updated" };
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    return { status: "error", message };
  }
}

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

export function deleteWorkflow(workflowId: string) {
  return invoke<Project[]>("delete_workflow", { workflowId });
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

export function createChat() {
  return invoke<Chat>("create_chat");
}

export function listChats() {
  return invoke<Chat[]>("list_chats");
}

export function deleteChat(chatId: string) {
  return invoke<ChatDeleteResult>("delete_chat", { chatId });
}

export function updateChatConfig(
  chatId: string,
  config: import("./lib/types").ChatConfig,
) {
  return invoke<Chat>("update_chat_config", { chatId, config });
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

export function createAgentDefinitionWithAi(
  description: string,
  settings: AppSettings,
  transientApiKey: string | null = null,
) {
  return invoke<AgentDefinition>("create_agent_definition_with_ai", {
    description,
    settings,
    transientApiKey,
  });
}

export function saveAgents(agents: AgentDefinition[]) {
  return invoke<void>("save_agents", { agents });
}

export function loadSettings(projectPath?: string | null) {
  return invoke<SettingsLoadPayload>("load_settings", { projectPath: projectPath ?? null });
}

export function saveSettings(settings: AppSettings) {
  return invoke<void>("save_settings", { settings });
}

export function debugLogPath() {
  return invoke<string>("debug_log_path");
}

export function appendDebugLog(settings: AppSettings, entry: DebugLogEntry) {
  return invoke<DebugLogWrite>("append_debug_log", { settings, entry });
}

export function importMcpConfig(content: string) {
  return invoke<McpConfigImport>("import_mcp_config", { content });
}

export function applyMcpConfig(content: string) {
  return invoke<McpConfigImport>("apply_mcp_config", { content });
}

export function exportMcpConfig() {
  return invoke<string>("export_mcp_config");
}

export function probeMcpServer(config: McpServerConfig, sourcePath?: string) {
  return invoke<McpProbeResult>("probe_mcp_server", { config, sourcePath });
}

export function listMcpCapabilities(serverId: string) {
  return invoke<McpCapabilityCatalog>("list_mcp_capabilities", { serverId });
}

export function previewMcpResource(serverId: string, uri: string, maxBytes: number) {
  return invoke<McpContextSnapshot>("preview_mcp_resource", { serverId, uri, maxBytes });
}

export function previewMcpPrompt(
  serverId: string,
  name: string,
  arguments_: Record<string, string>,
  maxBytes: number,
) {
  return invoke<McpContextSnapshot>("preview_mcp_prompt", {
    serverId,
    name,
    arguments: arguments_,
    maxBytes,
  });
}

export function startMcpOAuth(serverId: string, scopes: string[]) {
  return invoke<McpOAuthStatus>("start_mcp_oauth", { serverId, scopes });
}

export function mcpOAuthStatus(serverId: string) {
  return invoke<McpOAuthStatus>("mcp_oauth_status", { serverId });
}

export function refreshMcpOAuth(serverId: string) {
  return invoke<McpOAuthStatus>("refresh_mcp_oauth", { serverId });
}

export function disconnectMcpOAuth(serverId: string) {
  return invoke<McpOAuthStatus>("disconnect_mcp_oauth", { serverId });
}

export function saveMcpSecret(serverId: string, slot: string, value: string) {
  return invoke<string>("save_mcp_secret", { serverId, slot, value });
}

export function deleteMcpSecret(secretRef: string) {
  return invoke<void>("delete_mcp_secret", { secretRef });
}

export function searchMcpRegistry(search?: string, cursor?: string) {
  return invoke<McpCatalogPage>("search_mcp_registry", {
    search: search || null,
    cursor: cursor || null,
  });
}

export function listMcpRegistryVersions(serverName: string) {
  return invoke<McpCatalogPage>("list_mcp_registry_versions", { serverName });
}

export function previewMcpRegistryInstall(
  serverName: string,
  version: string,
  packageIndex: number,
) {
  return invoke<McpInstallPreview>("preview_mcp_registry_install", {
    serverName,
    version,
    packageIndex,
  });
}

export function previewMcpRegistryRemote(
  serverName: string,
  version: string,
  remoteIndex: number,
) {
  return invoke<McpInstallPreview>("preview_mcp_registry_remote", {
    serverName,
    version,
    remoteIndex,
  });
}

export function installMcpPackage(operationId: string, server: McpServerConfig) {
  return invoke<McpInstallResult>("install_mcp_package", { operationId, server });
}

export function cancelMcpInstall(operationId: string) {
  return invoke<boolean>("cancel_mcp_install", { operationId });
}

export function rollbackMcpInstall(serverId: string) {
  return invoke<McpServerConfig>("rollback_mcp_install", { serverId });
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

export function startCodexLogin() {
  return invoke<CodexLoginStatus>("start_codex_login");
}

export function codexLoginStatus() {
  return invoke<CodexLoginStatus>("codex_login_status");
}

export function cancelCodexLogin() {
  return invoke<CodexLoginStatus>("cancel_codex_login");
}

export function disconnectCodex() {
  return invoke<CodexLoginStatus>("disconnect_codex");
}

export function loadSearchApiKey(provider: string) {
  return invoke<string | null>("load_search_api_key", { provider });
}

export function saveSearchApiKey(provider: string, apiKey: string) {
  return invoke<void>("save_search_api_key", { provider, apiKey });
}

export function deleteSearchApiKey(provider: string) {
  return invoke<void>("delete_search_api_key", { provider });
}


export function resolveProviderReadiness(settings: AppSettings, transientApiKey: string | null = null) {
  return invoke<ProviderReadiness>("resolve_provider_readiness", {
    settings,
    transientApiKey,
  });
}

export function refreshBedrockModels(settings: AppSettings) {
  return invoke<string[]>("refresh_bedrock_models", { settings });
}

export function refreshProviderModels(
  settings: AppSettings,
  transientApiKey: string | null = null,
) {
  return invoke<string[]>("refresh_provider_models", {
    settings,
    transientApiKey,
  });
}

export function verifyBedrockCredentials(settings: AppSettings) {
  return invoke<string>("verify_bedrock_credentials", { settings });
}

export function validateWorkflow(workflow: Workflow) {
  return invoke<WorkflowValidationSummary>("validate_workflow", { workflow });
}

export function startWorkflowAuthoring(
  baseWorkflow: Workflow | null = null,
  targetProjectId: string | null = null,
) {
  return invoke<WorkflowAuthoringStartResult>("start_workflow_authoring", {
    baseWorkflow,
    targetProjectId,
  });
}

export function endWorkflowAuthoring(sessionId: string) {
  return invoke<boolean>("end_workflow_authoring", { sessionId });
}

export function workflowAuthoringTurn(
  sessionId: string,
  message: string,
  settings: AppSettings,
  transientApiKey: string | null = null,
  runtimeConfig: WorkflowAuthoringRuntimeConfig = {
    model: null,
    reasoningEffort: null,
    reasoningBudgetTokens: null,
    fastMode: false,
  },
) {
  return invoke<WorkflowAuthoringTurnResult>("workflow_authoring_turn", {
    sessionId,
    message,
    settings,
    runtimeConfig,
    transientApiKey,
  });
}

export function createAgentNode(index: number, x: number, y: number, agentId: string | null = null) {
  return invoke<Node>("create_agent_node", { index, x, y, agentId });
}

export function startRun(
  workflow: Workflow,
  settings: AppSettings,
  projectId: string | null = null,
  transientApiKey: string | null = null,
  message: UserMessageInput | null = null,
  invokedSkillIds: readonly string[] = [],
) {
  return invoke<WorkflowRunState>("start_run", {
    workflow,
    settings,
    projectId,
    transientApiKey,
    message,
    ...(invokedSkillIds.length > 0 ? { invokedSkillIds: [...invokedSkillIds] } : {}),
  });
}

export function startChat(
  chatId: string,
  settings: AppSettings,
  transientApiKey: string | null,
  message: UserMessageInput,
  invokedSkillIds: readonly string[] = [],
) {
  return invoke<ChatRunPayload>("start_chat", {
    chatId,
    settings,
    transientApiKey,
    message,
    ...(invokedSkillIds.length > 0 ? { invokedSkillIds: [...invokedSkillIds] } : {}),
  });
}

export function stopRun(runId: string) {
  return invoke<WorkflowRunState>("stop_run", { runId });
}

export function continueRun(
  runId: string,
  workflow: Workflow,
  settings: AppSettings,
  transientApiKey: string | null = null,
) {
  return invoke<WorkflowRunState>("continue_run", {
    runId,
    workflow,
    settings,
    transientApiKey,
  });
}

export function isRunContinuable(runId: string) {
  return invoke<boolean>("is_run_continuable", { runId });
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
  continuation?: DurableRunContinuationInput,
) {
  return invoke<WorkflowRunState>("resume_durable_run", {
    runId,
    settings,
    transientApiKey,
    ...(continuation ? { continuation } : {}),
  });
}

export function interruptNode(runId: string, nodeId: string) {
  return invoke<WorkflowRunState>("interrupt_node", { runId, nodeId });
}

export function retryNode(runId: string, nodeId: string) {
  return invoke<WorkflowRunState>("retry_node", { runId, nodeId });
}

export function updateNodeRuntimeConfig(
  runId: string,
  nodeId: string,
  update: import("./lib/types").NodeRuntimeConfigUpdate,
) {
  return invoke<WorkflowRunState>("update_node_runtime_config", { runId, nodeId, update });
}

export function previewFileEdit(
  runId: string,
  approvalId: string,
  toolName: string,
  toolArguments: unknown,
) {
  return invoke<import("./lib/types").FileEditPreview>("preview_file_edit", {
    runId,
    approvalId,
    toolName,
    arguments: toolArguments,
  });
}

export function gitDiffFile(runId: string, path: string) {
  return invoke<string>("git_diff_file", { runId, path });
}

export function loadFileChangeDiff(runId: string, diffArtifactId: string) {
  return invoke<string>("load_file_change_diff", { runId, diffArtifactId });
}

export function gitDiffRepo(cwd: string) {
  return invoke<string>("git_diff_repo", { cwd });
}

export function gitIsRepo(cwd: string) {
  return invoke<boolean>("git_is_repo", { cwd });
}

export function gitCurrentBranch(cwd: string) {
  return invoke<string>("git_current_branch", { cwd });
}

export function revertEditBatch(runId: string, batchId: string) {
  return invoke<WorkflowRunState>("revert_edit_batch", { runId, batchId });
}

export function submitUserInput(
  runId: string,
  nodeId: string,
  message: UserMessageInput,
  invokedSkillIds: readonly string[] = [],
) {
  return invoke<WorkflowRunState>("submit_user_input", {
    runId,
    nodeId,
    message,
    ...(invokedSkillIds.length > 0 ? { invokedSkillIds: [...invokedSkillIds] } : {}),
  });
}

export function submitToolApproval(
  runId: string,
  approvalId: string,
  allow: boolean,
  reason?: string | null,
) {
  return invoke<WorkflowRunState>("submit_tool_approval", {
    runId,
    approvalId,
    allow,
    reason: reason ?? null,
  });
}

export function resolveMcpClientRequest(
  runId: string,
  requestId: string,
  decision: import("./lib/types").McpClientRequestDecision,
) {
  return invoke<WorkflowRunState>("resolve_mcp_client_request", {
    runId,
    requestId,
    decision,
  });
}

export function getRunState(runId: string) {
  return invoke<WorkflowRunState | null>("get_run_state", { runId });
}

export function clearRunTrace(runId: string) {
  return invoke<WorkflowRunState | null>("clear_run_trace", { runId });
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

export function scheduleFromPreset(draft: ScheduleDraft) {
  return invoke<WorkflowSchedule>("build_schedule_from_draft", { draft });
}

export function scheduleDraftFromSchedule(schedule: WorkflowSchedule) {
  return invoke<ScheduleDraft>("schedule_draft_from_schedule", { schedule });
}

export function describeWorkflowSchedule(schedule: WorkflowSchedule) {
  return invoke<string>("describe_workflow_schedule", { schedule });
}

export function listenToScheduleStatuses(handler: (statuses: ScheduleStatus[]) => void) {
  return listen<ScheduleStatus[]>(SCHEDULE_EVENT, (event) => handler(event.payload));
}

export function listenToWorkflowAuthoringThinking(
  handler: (event: WorkflowAuthoringThinkingEvent) => void,
) {
  return listen<WorkflowAuthoringThinkingEvent>(WORKFLOW_AUTHORING_THINKING_EVENT, (event) =>
    handler(event.payload),
  );
}

export function listenToWorkflowAuthoringDraft(
  handler: (event: WorkflowAuthoringDraftEvent) => void,
) {
  return listen<WorkflowAuthoringDraftEvent>(WORKFLOW_AUTHORING_DRAFT_EVENT, (event) =>
    handler(event.payload),
  );
}

/** Native app window handle (Tauri seam — do not import @tauri-apps in components). */
export function getAppWindow() {
  return getCurrentWindow();
}

/** Native file/folder picker (Tauri seam). */
export function openNativeDialog(options?: OpenDialogOptions) {
  return openDialog(options);
}

const CHAT_ATTACHMENT_FILTERS: OpenDialogOptions["filters"] = [
  {
    name: "Images and documents",
    extensions: [
      "jpg",
      "jpeg",
      "png",
      "gif",
      "webp",
      "pdf",
      "txt",
      "md",
      "markdown",
      "csv",
      "json",
      "html",
      "htm",
      "css",
      "js",
      "mjs",
      "cjs",
      "py",
    ],
  },
];

/** Native chat attachment picker. Returns source paths in selection order. */
export async function pickChatAttachmentSources(): Promise<string[]> {
  const selected = await openDialog({
    multiple: true,
    directory: false,
    filters: CHAT_ATTACHMENT_FILTERS,
  });
  if (!selected) {
    return [];
  }
  return Array.isArray(selected) ? selected : [selected];
}

export function stageChatAttachment(
  fileName: string,
  mediaType: string,
  dataBase64: string,
) {
  return invoke<StagedChatAttachment>("stage_chat_attachment", {
    fileName,
    mediaType,
    dataBase64,
  });
}

export function removeStagedChatAttachment(token: string) {
  return invoke<void>("remove_staged_chat_attachment", { token });
}

export function loadChatAttachmentPreview(runId: string, attachmentId: string) {
  return invoke<AttachmentPreview>("load_chat_attachment_preview", {
    runId,
    attachmentId,
  });
}

/** Native confirm dialog (Tauri seam). */
export function confirmNativeDialog(message: string, options?: string | ConfirmDialogOptions) {
  return confirmDialog(message, options);
}
