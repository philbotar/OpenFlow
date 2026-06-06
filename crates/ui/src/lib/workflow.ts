import type {
  AgentStatus,
  AppSettings,
  Edge,
  EdgeId,
  Node,
  NodeId,
  NodeToolConfig,
  ProviderProfile,
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
] as const;



export function createEmptyToolConfig(): NodeToolConfig {
  return {
    catalog: { tools: SUPPORTED_NODE_TOOLS.map((tool) => ({ name: tool.name })) },
    approvalMode: "write",
    overrides: [],
    maxToolRounds: 8,
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

export function cloneWorkflow(workflow: Workflow): Workflow {
  return {
    id: workflow.id,
    name: workflow.name,
    nodes: workflow.nodes.map(cloneNode),
    edges: workflow.edges.map(cloneEdge),
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
  };
}

export function activeProfile(settings: AppSettings): ProviderProfile {
  return settings.providers[settings.active_provider] ?? Object.values(settings.providers)[0];
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
    activeManualNodeId: null,
    activeToolCallId: null,
    pendingApprovals: [],
    toolCallsByNode: {},
    toolArtifacts: {},
    execApprovalGranted: false,
    statusByNode,
    lastReport: null,
    lastError: null,
    chatLogs,
    runTrace: [],
    outputs: {},
  };
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

export function canSendChat(
  runState: WorkflowRunState | null,
  selectedNodeId: NodeId | null,
  readinessReady: boolean,
  text: string,
): boolean {
  return (
    runState?.active === true &&
    runState.awaitingNodeId === selectedNodeId &&
    readinessReady &&
    text.trim() !== ""
  );
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
  return {
    display_name: profile.display_name,
    base_url: profile.base_url,
    transport: profile.transport,
    responses_path: profile.responses_path,
    chat_completions_path: profile.chat_completions_path,
    known_models: [...profile.known_models],
    default_model: profile.default_model,
    key_ref: profile.key_ref,
    editable: profile.editable,
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