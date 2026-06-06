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
} from "../lib/types";

export type RunStateListener = (runState: WorkflowRunState) => void;

export interface UiDesktopOutboundPort {
	bootstrapApp: () => Promise<BootstrapPayload>;
	listWorkflows: () => Promise<WorkflowListItem[]>;
	loadAllWorkflows: () => Promise<Workflow[]>;
	loadWorkflow: (workflowId: string) => Promise<Workflow>;
	createWorkflow: (name: string) => Promise<Workflow>;
	saveWorkflow: (workflow: Workflow) => Promise<Workflow>;
	saveWorkflows: (workflows: Workflow[]) => Promise<void>;
	renameWorkflow: (workflowId: string, name: string) => Promise<WorkflowListItem>;
	listAgents: () => Promise<AgentDefinitionSummary[]>;
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
		transientApiKey?: string | null,
	) => Promise<WorkflowRunState>;
	submitUserInput: (nodeId: string, text: string) => Promise<WorkflowRunState>;
	submitToolApproval: (approvalId: string, allow: boolean) => Promise<WorkflowRunState>;
	completeManualNode: () => Promise<WorkflowRunState>;
	getRunState: () => Promise<WorkflowRunState | null>;
	clearRunTrace: () => Promise<WorkflowRunState | null>;
	listenToRunState: (handler: RunStateListener) => Promise<() => void>;
}
