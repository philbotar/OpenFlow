import * as desktopApi from "./api";
import type {
	AgentDefinition,
	AgentDefinitionSummary,
	AppSettings,
	BootstrapPayload,
	DebugLogEntry,
	DebugLogWrite,
	Node,
	Project,
	ProjectFileReference,
	ProjectFileReferenceContent,
	CopyWorkflowToProjectResult,
	ProviderReadiness,
	SkillSummary,
	McpServerConfig,
	SettingsLoadPayload,
	Workflow,
	WorkflowListItem,
	WorkflowRunState,
	WorkflowValidationSummary,
	WorkflowAuthoringTurnResult,
	WorkflowAuthoringMessage,
	WorkflowAuthoringValidation,
	FileEditPreview,
	TerminalEvent,
	TerminalStart,
	ScheduleStatus,
	RunSummary,
} from "./lib/types";
import type { AppUpdateAvailability, AppUpdateResult } from "./api";

export type RunStateListener = (runState: WorkflowRunState) => void;

export interface UiDesktopOutboundPort {
	bootstrapApp: () => Promise<BootstrapPayload>;
	listProjects: () => Promise<Project[]>;
	listProjectFileReferences: (
		executionCwd: string,
		query?: string | null,
		limit?: number | null,
	) => Promise<ProjectFileReference[]>;
	readProjectFileReferences: (
		executionCwd: string,
		paths: string[],
	) => Promise<ProjectFileReferenceContent[]>;
	saveProjects: (projects: Project[]) => Promise<void>;
	createProjectFromDirectory: (path: string) => Promise<Project>;
	assignWorkflowToProject: (projectId: string, workflowId: string) => Promise<Project[]>;
	copyWorkflowToProject: (
		targetProjectId: string,
		sourceWorkflowId: string,
	) => Promise<CopyWorkflowToProjectResult>;
	unassignWorkflowFromProject: (projectId: string, workflowId: string) => Promise<Project[]>;
	deleteWorkflow: (workflowId: string) => Promise<Project[]>;
	listWorkflows: () => Promise<WorkflowListItem[]>;
	loadAllWorkflows: () => Promise<Workflow[]>;
	loadWorkflow: (workflowId: string) => Promise<Workflow>;
	createWorkflow: (name: string) => Promise<Workflow>;
	saveWorkflow: (workflow: Workflow) => Promise<Workflow>;
	saveWorkflows: (workflows: Workflow[]) => Promise<void>;
	renameWorkflow: (workflowId: string, name: string) => Promise<WorkflowListItem>;
	listAgents: () => Promise<AgentDefinitionSummary[]>;
	listSkills: () => Promise<SkillSummary[]>;
	loadAgents: () => Promise<AgentDefinition[]>;
	createAgentDefinition: (name: string) => Promise<AgentDefinition>;
	saveAgents: (agents: AgentDefinition[]) => Promise<void>;
	loadSettings: (projectPath?: string | null) => Promise<SettingsLoadPayload>;
	saveSettings: (settings: AppSettings) => Promise<void>;
	debugLogPath: () => Promise<string>;
	appendDebugLog: (settings: AppSettings, entry: DebugLogEntry) => Promise<DebugLogWrite>;
	loadProviderApiKey: (providerId: string) => Promise<string | null>;
	saveProviderApiKey: (providerId: string, apiKey: string) => Promise<void>;
	deleteProviderApiKey: (providerId: string) => Promise<void>;
	resolveProviderReadiness: (
		settings: AppSettings,
		transientApiKey?: string | null,
	) => Promise<ProviderReadiness>;
	refreshBedrockModels: (settings: AppSettings) => Promise<string[]>;
	validateWorkflow: (workflow: Workflow) => Promise<WorkflowValidationSummary>;
	startWorkflowAuthoring: (baseWorkflow?: Workflow | null) => Promise<string>;
	workflowAuthoringTurn: (
		sessionId: string,
		message: string,
		settings: AppSettings,
		transientApiKey?: string | null,
	) => Promise<WorkflowAuthoringTurnResult>;
	createAgentNode: (
		index: number,
		x: number,
		y: number,
		agentId?: string | null,
	) => Promise<Node>;
	startRun: (
		workflow: Workflow,
		settings: AppSettings,
		executionCwd?: string | null,
		transientApiKey?: string | null,
		entrypoint?: string | null,
	) => Promise<WorkflowRunState>;
	stopRun: () => Promise<WorkflowRunState>;
	continueRun: (
		workflow: Workflow,
		settings: AppSettings,
		transientApiKey?: string | null,
	) => Promise<WorkflowRunState>;
	isRunContinuable: () => Promise<boolean>;
	listRuns: (workflowId?: string | null) => Promise<RunSummary[]>;
	replayRun: (runId: string) => Promise<WorkflowRunState>;
	resumeDurableRun: (
		runId: string,
		settings: AppSettings,
		transientApiKey?: string | null,
	) => Promise<WorkflowRunState>;
	interruptNode: (nodeId: string) => Promise<WorkflowRunState>;
	retryNode: (nodeId: string) => Promise<WorkflowRunState>;
	previewFileEdit: (
		approvalId: string,
		toolName: string,
		toolArguments: unknown,
	) => Promise<FileEditPreview>;
	gitDiffFile: (path: string) => Promise<string>;
	gitDiffRepo: (cwd: string) => Promise<string>;
	gitIsRepo: (cwd: string) => Promise<boolean>;
	gitCurrentBranch: (cwd: string) => Promise<string>;
	revertEditBatch: (batchId: string) => Promise<WorkflowRunState>;
	submitUserInput: (nodeId: string, text: string) => Promise<WorkflowRunState>;
	submitToolApproval: (
		approvalId: string,
		allow: boolean,
		reason?: string | null,
	) => Promise<WorkflowRunState>;
	completeManualNode: () => Promise<WorkflowRunState>;
	getRunState: () => Promise<WorkflowRunState | null>;
	clearRunTrace: () => Promise<WorkflowRunState | null>;
	startTerminal: (
		cwd?: string | null,
		cols?: number,
		rows?: number,
	) => Promise<TerminalStart>;
	writeTerminal: (sessionId: string, data: string) => Promise<void>;
	resizeTerminal: (sessionId: string, cols: number, rows: number) => Promise<void>;
	stopTerminal: (sessionId: string) => Promise<void>;
	listenToTerminalEvent: (handler: (event: TerminalEvent) => void) => Promise<() => void>;
	listenToRunState: (handler: RunStateListener) => Promise<() => void>;
	listScheduleStatuses: () => Promise<ScheduleStatus[]>;
	refreshSchedules: () => Promise<ScheduleStatus[]>;
	listenToScheduleStatuses: (handler: (statuses: ScheduleStatus[]) => void) => Promise<() => void>;
	probeMcpServer: (config: McpServerConfig) => Promise<string[]>;
	getAppVersion: () => Promise<string>;
	checkAppUpdateAvailable: () => Promise<AppUpdateAvailability>;
	installAppUpdate: () => Promise<AppUpdateResult>;
}

export function createUiDesktopOutboundAdapter(): UiDesktopOutboundPort {
	return {
		bootstrapApp: desktopApi.bootstrapApp,
		listProjects: desktopApi.listProjects,
		listProjectFileReferences: desktopApi.listProjectFileReferences,
		readProjectFileReferences: desktopApi.readProjectFileReferences,
		saveProjects: desktopApi.saveProjects,
		createProjectFromDirectory: desktopApi.createProjectFromDirectory,
		assignWorkflowToProject: desktopApi.assignWorkflowToProject,
		copyWorkflowToProject: desktopApi.copyWorkflowToProject,
		unassignWorkflowFromProject: desktopApi.unassignWorkflowFromProject,
		deleteWorkflow: desktopApi.deleteWorkflow,
		listWorkflows: desktopApi.listWorkflows,
		loadAllWorkflows: desktopApi.loadAllWorkflows,
		loadWorkflow: desktopApi.loadWorkflow,
		createWorkflow: desktopApi.createWorkflow,
		saveWorkflow: desktopApi.saveWorkflow,
		saveWorkflows: desktopApi.saveWorkflows,
		renameWorkflow: desktopApi.renameWorkflow,
		listAgents: desktopApi.listAgents,
		listSkills: desktopApi.listSkills,
		loadAgents: desktopApi.loadAgents,
		createAgentDefinition: desktopApi.createAgentDefinition,
		saveAgents: desktopApi.saveAgents,
		loadSettings: desktopApi.loadSettings,
		saveSettings: desktopApi.saveSettings,
		debugLogPath: desktopApi.debugLogPath,
		appendDebugLog: desktopApi.appendDebugLog,
		loadProviderApiKey: desktopApi.loadProviderApiKey,
		saveProviderApiKey: desktopApi.saveProviderApiKey,
		deleteProviderApiKey: desktopApi.deleteProviderApiKey,
		resolveProviderReadiness: desktopApi.resolveProviderReadiness,
		refreshBedrockModels: desktopApi.refreshBedrockModels,
		validateWorkflow: desktopApi.validateWorkflow,
		startWorkflowAuthoring: desktopApi.startWorkflowAuthoring,
		workflowAuthoringTurn: desktopApi.workflowAuthoringTurn,
		createAgentNode: desktopApi.createAgentNode,
		startRun: desktopApi.startRun,
		stopRun: desktopApi.stopRun,
		continueRun: desktopApi.continueRun,
		isRunContinuable: desktopApi.isRunContinuable,
		listRuns: desktopApi.listRuns,
		replayRun: desktopApi.replayRun,
		resumeDurableRun: desktopApi.resumeDurableRun,
		interruptNode: desktopApi.interruptNode,
		retryNode: desktopApi.retryNode,
		previewFileEdit: desktopApi.previewFileEdit,
		gitDiffFile: desktopApi.gitDiffFile,
		gitDiffRepo: desktopApi.gitDiffRepo,
		gitIsRepo: desktopApi.gitIsRepo,
		gitCurrentBranch: desktopApi.gitCurrentBranch,
		revertEditBatch: desktopApi.revertEditBatch,
		submitUserInput: desktopApi.submitUserInput,
		submitToolApproval: desktopApi.submitToolApproval,
		completeManualNode: desktopApi.completeManualNode,
		getRunState: desktopApi.getRunState,
		clearRunTrace: desktopApi.clearRunTrace,
		startTerminal: desktopApi.startTerminal,
		writeTerminal: desktopApi.writeTerminal,
		resizeTerminal: desktopApi.resizeTerminal,
		stopTerminal: desktopApi.stopTerminal,
		listenToTerminalEvent: desktopApi.listenToTerminalEvent,
		listenToRunState: desktopApi.listenToRunState,
		listScheduleStatuses: desktopApi.listScheduleStatuses,
		refreshSchedules: desktopApi.refreshSchedules,
		listenToScheduleStatuses: desktopApi.listenToScheduleStatuses,
		probeMcpServer: desktopApi.probeMcpServer,
		getAppVersion: desktopApi.getAppVersion,
		checkAppUpdateAvailable: desktopApi.checkAppUpdateAvailable,
		installAppUpdate: desktopApi.installAppUpdate,
	};
}
