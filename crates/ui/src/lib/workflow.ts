import type {
  AgentNodeConfig,
  AgentStatus,
  AppSettings,
  Edge,
  EdgeId,
  EditBatch,
  FileChangeRecord,
  Node,
  NodeId,
  NodeToolConfig,
  ProviderProfile,
  ReasoningEffortOption,
  SubagentSummary,
  Workflow,
  WorkflowRunState,
} from "./types";
import { PROVIDER_ORDER } from "../constants/providers";

export const NODE_WIDTH = 320;
export const NODE_HEIGHT = 88;

export function providerDisplayOrder(settings: AppSettings): string[] {
  const providerIds = Object.keys(settings.providers);
  const ordered = PROVIDER_ORDER.filter((providerId) => providerId in settings.providers);
  const extras = providerIds
    .filter(
      (providerId) =>
        !PROVIDER_ORDER.includes(providerId as (typeof PROVIDER_ORDER)[number]),
    )
    .sort();
  return [...ordered, ...extras];
}

export function nextNodePlacement(workflow: Workflow): { index: number; x: number; y: number } {
  const index = workflow.nodes.length;
  return {
    index,
    x: 96 + index * 32,
    y: 96 + index * 20,
  };
}
export const SUPPORTED_NODE_TOOLS = [
  {
    name: "read",
    description: "Read local files, directories, and URLs.",
  },
  {
    name: "search",
    description: "Search files with regular expressions.",
  },
  {
    name: "find",
    description: "Find files and directories by glob.",
  },
  {
    name: "ast_grep",
    description: "Search code structurally with ast-grep. Included with this repo tooling.",
  },
  {
    name: "write",
    description: "Create or overwrite a file under the execution folder.",
  },
  {
    name: "edit",
    description: "Replace exact or fuzzy-matched text in a file.",
  },
  {
    name: "apply_patch",
    description: "Apply a Codex patch envelope to files under the execution folder.",
  },
  {
    name: "bash",
    description: "Run shell commands (git, cargo, npm, etc.) in the execution folder.",
  },
] as const;



export function createEmptyToolConfig(): NodeToolConfig {
  return {
    catalog: { tools: SUPPORTED_NODE_TOOLS.map((tool) => ({ name: tool.name })) },
    approvalMode: "write",
    overrides: [],
  };
}
export type WorkflowCanvasGraphNode = {
  id: NodeId;
  label: string;
  position: Node["position"];
};

export type WorkflowCanvasGraphEdge = {
  id: EdgeId;
  from: NodeId;
  to: NodeId;
};

export type WorkflowCanvasGraph = {
  id: Workflow["id"];
  nodes: WorkflowCanvasGraphNode[];
  edges: WorkflowCanvasGraphEdge[];
};

export type WorkflowCanvasStatusByNode = Readonly<Record<NodeId, AgentStatus>>;

export type WorkflowCanvasSubagent = {
  id: string;
  name: string;
  purpose: string;
  status: SubagentSummary["status"];
};

export type WorkflowCanvasSubagentsByNode = Readonly<Record<NodeId, WorkflowCanvasSubagent[]>>;

export function cloneWorkflow(workflow: Workflow): Workflow {
  return {
    id: workflow.id,
    name: workflow.name,
    nodes: workflow.nodes.map(cloneNode),
    edges: workflow.edges.map(cloneEdge),
    settings: {
      shared_context: workflow.settings?.shared_context ?? "",
      schedule: workflow.settings?.schedule ?? null,
      retry_policy: workflow.settings?.retry_policy ?? {
        max_attempts: 0,
        backoff_ms: 1_000,
      },
      provider_id: workflow.settings?.provider_id ?? null,
    },
  };
}

export function cloneSettings(settings: AppSettings): AppSettings {
  return {
    active_provider: settings.active_provider,
    providers: Object.fromEntries(
      Object.entries(settings.providers).map(([providerId, profile]) => [
        providerId,
        cloneProviderProfile(profile),
      ]),
    ),
    skill_search_paths: settings.skill_search_paths
      ? [...settings.skill_search_paths]
      : undefined,
    lsp: settings.lsp ? { ...settings.lsp } : undefined,
  };
}

export function activeProfile(settings: AppSettings): ProviderProfile {
  return settings.providers[settings.active_provider] ?? Object.values(settings.providers)[0];
}

export function reasoningEffortOptions(profile: ProviderProfile): ReasoningEffortOption[] {
  return profile.reasoning_effort_options ?? profile.reasoningEffortOptions ?? [];
}

export function defaultReasoningBudgetTokens(
  profile: ProviderProfile,
): Record<string, number> {
  return (
    profile.default_reasoning_budget_tokens ?? profile.defaultReasoningBudgetTokens ?? {}
  );
}

export function defaultReasoningEffort(profile: ProviderProfile): string | null {
  return profile.default_reasoning_effort ?? profile.defaultReasoningEffort ?? null;
}

export function reasoningBudgetForEffort(
  profile: ProviderProfile,
  effort: string,
): number | undefined {
  const option = reasoningEffortOptions(profile).find((entry) => entry.value === effort);
  if (!option?.uses_budget_tokens) {
    return undefined;
  }
  return defaultReasoningBudgetTokens(profile)[effort];
}

export function agentReasoningEffort(agent: AgentNodeConfig): string | null {
  return agent.reasoning_effort ?? agent.reasoningEffort ?? null;
}

export function agentReasoningBudgetTokens(agent: AgentNodeConfig): number | null {
  const budget = agent.reasoning_budget_tokens ?? agent.reasoningBudgetTokens;
  return budget ?? null;
}

export function withDefaultReasoningFromProfile(
  agent: AgentNodeConfig,
  profile: ProviderProfile,
): AgentNodeConfig {
  const effort = defaultReasoningEffort(profile);
  if (!effort || agentReasoningEffort(agent)) {
    return agent;
  }
  const budget = reasoningBudgetForEffort(profile, effort);
  return {
    ...agent,
    reasoning_effort: effort,
    reasoning_budget_tokens: budget ?? null,
  };
}

export function createIdleRunState(workflow: Workflow): WorkflowRunState {
  const statusByNode = workflow.nodes.reduce<Record<NodeId, AgentStatus>>((acc, node) => {
    acc[node.id] = "idle";
    return acc;
  }, {});
  const chatLogs = workflow.nodes.reduce<Record<NodeId, []>>((acc, node) => {
    acc[node.id] = [];
    return acc;
  }, {});
  return {
    active: false,
    awaitingNodeId: null,
    awaitingNodeIds: [],
    activeManualNodeId: null,
    activeToolCallId: null,
    pendingApprovals: [],
    toolCallsByNode: {},
    toolArtifacts: {},
    execApprovalGranted: false,
    statusByNode,
    subagentsByNode: {},
    lastReport: null,
    lastError: null,
    chatLogs,
    runTrace: [],
    outputs: {},
    changedFiles: [],
    changedFilesByNode: {},
    editBatches: [],
  };
}

export function nodeChangedFiles(
  runState: WorkflowRunState | null,
  nodeId: NodeId | null,
): FileChangeRecord[] {
  if (!runState || !nodeId) {
    return [];
  }
  return runState.changedFilesByNode[nodeId] ?? [];
}

export function nodeEditBatches(
  runState: WorkflowRunState | null,
  nodeId: NodeId | null,
): EditBatch[] {
  if (!runState || !nodeId) {
    return [];
  }
  return runState.editBatches.filter((batch) => batch.nodeId === nodeId);
}

export function selectedNode(
  workflow: Workflow | undefined,
  selectedNodeId: NodeId | null,
): Node | undefined {
  return workflow?.nodes.find((node) => node.id === selectedNodeId);
}

export function replaceWorkflow(
  workflows: Workflow[],
  nextWorkflow: Workflow,
): Workflow[] {
  const next = workflows.map((workflow) =>
    workflow.id === nextWorkflow.id ? nextWorkflow : workflow,
  );
  return next.some((workflow) => workflow.id === nextWorkflow.id)
    ? next
    : [...next, nextWorkflow];
}

export function removeSelectedNode(
  workflow: Workflow,
  selectedNodeId: NodeId | null,
): Workflow {
  if (!selectedNodeId) {
    return workflow;
  }
  const next = cloneWorkflow(workflow);
  next.nodes = next.nodes.filter((node) => node.id !== selectedNodeId);
  next.edges = next.edges.filter(
    (edge) => edge.from !== selectedNodeId && edge.to !== selectedNodeId,
  );
  return next;
}

export function projectWorkflowCanvasGraph(
  workflow: Workflow | null | undefined,
  previous: WorkflowCanvasGraph | null = null,
): WorkflowCanvasGraph | null {
  if (!workflow) {
    return null;
  }

  if (
    previous &&
    previous.id === workflow.id &&
    sameWorkflowCanvasNodes(previous.nodes, workflow.nodes) &&
    sameWorkflowCanvasEdges(previous.edges, workflow.edges)
  ) {
    return previous;
  }

  return {
    id: workflow.id,
    nodes: workflow.nodes.map((node) => ({
      id: node.id,
      label: node.label,
      position: { x: node.position.x, y: node.position.y },
    })),
    edges: workflow.edges.map((edge) => ({
      id: edge.id,
      from: edge.from,
      to: edge.to,
    })),
  };
}

export function projectWorkflowCanvasStatusByNode(
  runState: WorkflowRunState | null,
  previous: WorkflowCanvasStatusByNode | null = null,
): WorkflowCanvasStatusByNode | null {
  if (!runState) {
    return null;
  }

  if (previous && sameWorkflowCanvasStatusByNode(previous, runState.statusByNode)) {
    return previous;
  }

  return { ...runState.statusByNode };
}

export function projectWorkflowCanvasSubagentsByNode(
  runState: WorkflowRunState | null,
  previous: WorkflowCanvasSubagentsByNode | null = null,
): WorkflowCanvasSubagentsByNode | null {
  if (!runState) {
    return null;
  }

  if (previous && sameWorkflowCanvasSubagentsByNode(previous, runState.subagentsByNode)) {
    return previous;
  }

  const result: Record<NodeId, WorkflowCanvasSubagent[]> = {};
  for (const [nodeId, subs] of Object.entries(runState.subagentsByNode)) {
    result[nodeId] = subs.map((s) => ({
      id: s.id,
      name: s.name,
      purpose: s.purpose,
      status: s.status,
    }));
  }
  return result;
}

export function statusForNode(
  statusByNode: WorkflowCanvasStatusByNode | null,
  nodeId: NodeId,
): AgentStatus {
  return statusByNode?.[nodeId] ?? "idle";
}

export function nodeOutput(
  runState: WorkflowRunState | null,
  nodeId: NodeId | null,
): unknown | null {
  if (!nodeId) {
    return null;
  }
  return runState?.outputs[nodeId] ?? null;
}

export function prettyJson(value: unknown): string {
  return JSON.stringify(value, null, 2);
}

export function isNodeAwaitingInput(
  runState: WorkflowRunState | null,
  nodeId: NodeId | null,
): boolean {
  if (!runState || !nodeId) {
    return false;
  }
  if (runState.awaitingNodeIds?.includes(nodeId)) {
    return true;
  }
  return runState.awaitingNodeId === nodeId;
}

export function pendingApprovalForNode(
  runState: WorkflowRunState | null,
  nodeId: NodeId | null,
) {
  if (!runState?.pendingApprovals || !nodeId) {
    return undefined;
  }
  return runState.pendingApprovals.find((approval) => approval.nodeId === nodeId);
}

export function canSendChat(
  runState: WorkflowRunState | null,
  selectedNodeId: NodeId | null,
  readinessReady: boolean,
  text: string,
): boolean {
  return (
    runState?.active === true &&
    isNodeAwaitingInput(runState, selectedNodeId) &&
    !pendingApprovalForNode(runState, selectedNodeId) &&
    readinessReady &&
    text.trim() !== ""
  );
}

export function isChatComposerBusy(
  runState: WorkflowRunState | null,
  selectedNodeId: NodeId | null,
): boolean {
  if (!runState || !selectedNodeId) {
    return false;
  }

  const status = runState.statusByNode[selectedNodeId];
  return status === "started" || status === "running_tool";
}

function cloneNode(node: Node): Node {
  return {
    id: node.id,
    label: node.label,
    kind: node.kind,
    position: { x: node.position.x, y: node.position.y },
    agent: {
      system_prompt: node.agent.system_prompt,
      task_prompt: node.agent.task_prompt,
      model: node.agent.model,
      output_schema: structuredClone(node.agent.output_schema),
      auto_start: node.agent.auto_start,
      tools: structuredClone(node.agent.tools),
      callable_agents: [...(node.agent.callable_agents ?? [])],
      allow_all_callable_agents: node.agent.allow_all_callable_agents ?? false,
      reasoning_effort: agentReasoningEffort(node.agent),
      reasoning_budget_tokens:
        node.agent.reasoning_budget_tokens ?? node.agent.reasoningBudgetTokens ?? null,
    },
  };
}

function cloneEdge(edge: Edge): Edge {
  return {
    id: edge.id,
    from: edge.from,
    to: edge.to,
  };
}

function cloneProviderProfile(profile: ProviderProfile): ProviderProfile {
  const reasoningOptions = reasoningEffortOptions(profile);
  const budgetTokens = defaultReasoningBudgetTokens(profile);
  return {
    display_name: profile.display_name,
    base_url: profile.base_url,
    transport: profile.transport,
    responses_path: profile.responses_path,
    chat_completions_path: profile.chat_completions_path,
    known_models: [...profile.known_models],
    default_model: profile.default_model,
    editable: profile.editable,
    reasoning_effort_options: reasoningOptions.map((option) => ({ ...option })),
    default_reasoning_budget_tokens: { ...budgetTokens },
    default_reasoning_effort: defaultReasoningEffort(profile),
  };
}

function sameWorkflowCanvasNodes(
  previousNodes: WorkflowCanvasGraphNode[],
  nextNodes: Workflow["nodes"],
): boolean {
  if (previousNodes.length !== nextNodes.length) {
    return false;
  }

  for (let index = 0; index < nextNodes.length; index += 1) {
    const previous = previousNodes[index];
    const next = nextNodes[index];
    if (
      previous.id !== next.id ||
      previous.label !== next.label ||
      previous.position.x !== next.position.x ||
      previous.position.y !== next.position.y
    ) {
      return false;
    }
  }

  return true;
}

function sameWorkflowCanvasEdges(
  previousEdges: WorkflowCanvasGraphEdge[],
  nextEdges: Workflow["edges"],
): boolean {
  if (previousEdges.length !== nextEdges.length) {
    return false;
  }

  for (let index = 0; index < nextEdges.length; index += 1) {
    const previous = previousEdges[index];
    const next = nextEdges[index];
    if (
      previous.id !== next.id ||
      previous.from !== next.from ||
      previous.to !== next.to
    ) {
      return false;
    }
  }

  return true;
}

function sameWorkflowCanvasStatusByNode(
  previous: WorkflowCanvasStatusByNode,
  next: Record<NodeId, AgentStatus>,
): boolean {
  const previousKeys = Object.keys(previous);
  const nextKeys = Object.keys(next);

  if (previousKeys.length !== nextKeys.length) {
    return false;
  }

  for (const nodeId of nextKeys) {
    if (previous[nodeId] !== next[nodeId]) {
      return false;
    }
  }

  return true;
}

function sameWorkflowCanvasSubagentsByNode(
  previous: WorkflowCanvasSubagentsByNode,
  next: Record<NodeId, SubagentSummary[]>,
): boolean {
  const previousKeys = Object.keys(previous);
  const nextKeys = Object.keys(next);

  if (previousKeys.length !== nextKeys.length) {
    return false;
  }

  for (const nodeId of nextKeys) {
    const prevSubs = previous[nodeId];
    const nextSubs = next[nodeId];
    if (!prevSubs || prevSubs.length !== nextSubs.length) {
      return false;
    }
    for (let i = 0; i < nextSubs.length; i += 1) {
      if (
        prevSubs[i].id !== nextSubs[i].id ||
        prevSubs[i].name !== nextSubs[i].name ||
        prevSubs[i].status !== nextSubs[i].status
      ) {
        return false;
      }
    }
  }

  return true;
}
