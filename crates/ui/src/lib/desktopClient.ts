import * as desktopApi from "../api";
import type {
	AgentDefinition,
	AgentDefinitionSummary,
	AppSettings,
	BootstrapPayload,
	Node,
	Project,
	ProviderReadiness,
	SkillSummary,
	Workflow,
	WorkflowListItem,
	WorkflowRunState,
	WorkflowValidationSummary,
} from "./types";

export type RunStateListener = (runState: WorkflowRunState) => void;

export interface RunStateEventSink {
	handleRunStateUpdate: (runState: WorkflowRunState) => void;
}

export interface UiDesktopOutboundPort {
	bootstrapApp: () => Promise<BootstrapPayload>;
	listProjects: () => Promise<Project[]>;
	saveProjects: (projects: Project[]) => Promise<void>;
	createProjectFromDirectory: (path: string) => Promise<Project>;
	assignWorkflowToProject: (projectId: string, workflowId: string) => Promise<Project[]>;
	unassignWorkflowFromProject: (projectId: string, workflowId: string) => Promise<Project[]>;
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
	loadSettings: () => Promise<AppSettings>;
	saveSettings: (settings: AppSettings) => Promise<void>;
	loadProviderApiKey: (providerId: string) => Promise<string | null>;
	saveProviderApiKey: (providerId: string, apiKey: string) => Promise<void>;
	deleteProviderApiKey: (providerId: string) => Promise<void>;
	resolveProviderReadiness: (
		settings: AppSettings,
		transientApiKey?: string | null,
	) => Promise<ProviderReadiness>;
	validateWorkflow: (workflow: Workflow) => Promise<WorkflowValidationSummary>;
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
	) => Promise<WorkflowRunState>;
	stopRun: () => Promise<WorkflowRunState>;
	submitUserInput: (nodeId: string, text: string) => Promise<WorkflowRunState>;
	submitToolApproval: (approvalId: string, allow: boolean) => Promise<WorkflowRunState>;
	completeManualNode: () => Promise<WorkflowRunState>;
	getRunState: () => Promise<WorkflowRunState | null>;
	clearRunTrace: () => Promise<WorkflowRunState | null>;
	listenToRunState: (handler: RunStateListener) => Promise<() => void>;
}

export function createUiDesktopOutboundAdapter(): UiDesktopOutboundPort {
	return {
		bootstrapApp: desktopApi.bootstrapApp,
		listProjects: desktopApi.listProjects,
		saveProjects: desktopApi.saveProjects,
		createProjectFromDirectory: desktopApi.createProjectFromDirectory,
		assignWorkflowToProject: desktopApi.assignWorkflowToProject,
		unassignWorkflowFromProject: desktopApi.unassignWorkflowFromProject,
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
		loadProviderApiKey: desktopApi.loadProviderApiKey,
		saveProviderApiKey: desktopApi.saveProviderApiKey,
		deleteProviderApiKey: desktopApi.deleteProviderApiKey,
		resolveProviderReadiness: desktopApi.resolveProviderReadiness,
		validateWorkflow: desktopApi.validateWorkflow,
		createAgentNode: desktopApi.createAgentNode,
		startRun: desktopApi.startRun,
		stopRun: desktopApi.stopRun,
		submitUserInput: desktopApi.submitUserInput,
		submitToolApproval: desktopApi.submitToolApproval,
		completeManualNode: desktopApi.completeManualNode,
		getRunState: desktopApi.getRunState,
		clearRunTrace: desktopApi.clearRunTrace,
		listenToRunState: desktopApi.listenToRunState,
	};
}

export function bindRunStateEvents(
	sink: RunStateEventSink,
	outboundPort: UiDesktopOutboundPort = createUiDesktopOutboundAdapter(),
) {
	return outboundPort.listenToRunState((runState) => sink.handleRunStateUpdate(runState));
}
