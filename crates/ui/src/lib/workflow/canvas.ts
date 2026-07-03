import type {
  AgentStatus,
  EdgeId,
  Node,
  NodeId,
  SubagentSummary,
  Workflow,
  WorkflowRunState,
} from "../types";

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
