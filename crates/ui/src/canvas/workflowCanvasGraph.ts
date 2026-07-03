import { MarkerType, type Edge as FlowEdge, type EdgeChange, type Node as FlowNode, type NodeChange } from "@xyflow/react";
import type { AgentStatus, EdgeId, NodeId, SubagentSummary } from "../lib/types";
import {
  NODE_HEIGHT,
  NODE_WIDTH,
  statusForNode,
  type WorkflowCanvasGraph,
  type WorkflowCanvasStatusByNode,
  type WorkflowCanvasSubagentsByNode,
} from "../lib/workflow";

export type WorkflowCanvasNodeData = {
  label: string;
  status: AgentStatus;
  subagents: SubagentSummary[];
  runActive?: boolean;
  onInterrupt?: (nodeId: string) => void;
  onRetry?: (nodeId: string) => void;
};

export type WorkflowCanvasNode = FlowNode<WorkflowCanvasNodeData, "workflowNode">;
export type WorkflowCanvasEdge = FlowEdge<Record<string, never>, "default">;

export function graphStructureSignature(graph: WorkflowCanvasGraph | null): string {
  if (!graph) {
    return "graph:none";
  }
  const nodeIds = graph.nodes.map((node) => node.id).sort().join(",");
  const edgeKeys = graph.edges
    .map((edge) => `${edge.id}:${edge.from}->${edge.to}`)
    .sort()
    .join(",");
  return `nodes:${nodeIds}|edges:${edgeKeys}`;
}

function edgeStrokeForTheme(colorMode: "light" | "dark") {
  return colorMode === "dark" ? "#4b5568" : "#c3cbda";
}

export function backgroundDotForTheme(colorMode: "light" | "dark") {
  return colorMode === "dark" ? "rgba(255, 255, 255, 0.08)" : "rgba(24, 24, 27, 0.14)";
}

export function defaultEdgeOptions(colorMode: "light" | "dark") {
  const stroke = edgeStrokeForTheme(colorMode);
  return {
    markerEnd: {
      type: MarkerType.ArrowClosed,
      color: stroke,
    },
    reconnectable: true,
    style: {
      stroke,
      strokeWidth: 2,
    },
  };
}

export function buildFlowNodes(
  graph: WorkflowCanvasGraph | null,
  selectedNodeId: NodeId | null,
  statusByNode: WorkflowCanvasStatusByNode | null,
  subagentsByNode: WorkflowCanvasSubagentsByNode | null,
  runActive = false,
  onInterruptNode?: (nodeId: NodeId) => void,
  onRetryNode?: (nodeId: NodeId) => void,
): WorkflowCanvasNode[] {
  if (!graph) {
    return [];
  }

  return graph.nodes.map((node) => ({
    id: node.id,
    type: "workflowNode",
    position: node.position,
    selected: selectedNodeId === node.id,
    data: {
      label: node.label,
      status: statusForNode(statusByNode, node.id),
      subagents: subagentsByNode?.[node.id] ?? [],
      runActive,
      onInterrupt: onInterruptNode ? (nodeId: string) => onInterruptNode(nodeId) : undefined,
      onRetry: onRetryNode ? (nodeId: string) => onRetryNode(nodeId) : undefined,
    },
    draggable: true,
    selectable: true,
    deletable: false,
    width: NODE_WIDTH,
    height: NODE_HEIGHT,
  }));
}

export function buildFlowEdges(
  graph: WorkflowCanvasGraph | null,
  selectedEdgeId: EdgeId | null,
  runActive = false,
  colorMode: "light" | "dark" = "light",
): WorkflowCanvasEdge[] {
  if (!graph) {
    return [];
  }

  const edgeOptions = defaultEdgeOptions(colorMode);

  return graph.edges.map((edge) => ({
    id: edge.id,
    source: edge.from,
    target: edge.to,
    selected: selectedEdgeId === edge.id,
    reconnectable: true,
    deletable: true,
    animated: runActive,
    markerEnd: edgeOptions.markerEnd,
    style: edgeOptions.style,
  }));
}

export function reconcileFlowNodes(
  currentNodes: WorkflowCanvasNode[],
  incomingNodes: WorkflowCanvasNode[],
): WorkflowCanvasNode[] {
  if (incomingNodes.length === 0 || currentNodes.length === 0) {
    return incomingNodes;
  }

  const currentById = new Map(currentNodes.map((node) => [node.id, node]));
  let changed = incomingNodes.length !== currentNodes.length;
  const result: WorkflowCanvasNode[] = [];

  for (const incoming of incomingNodes) {
    const current = currentById.get(incoming.id);
    if (!current) {
      result.push(incoming);
      changed = true;
      continue;
    }

    const position = current.dragging ? current.position : incoming.position;
    const data =
      current.data.label === incoming.data.label &&
      current.data.status === incoming.data.status
        ? current.data
        : incoming.data;

    if (
      current.selected === incoming.selected &&
      current.position.x === position.x &&
      current.position.y === position.y &&
      current.data === data &&
      current.width === incoming.width &&
      current.height === incoming.height
    ) {
      result.push(current);
      continue;
    }

    changed = true;
    result.push({
      ...current,
      ...incoming,
      position,
      data,
    });
  }

  return changed ? result : currentNodes;
}

export function reconcileFlowEdges(
  currentEdges: WorkflowCanvasEdge[],
  incomingEdges: WorkflowCanvasEdge[],
): WorkflowCanvasEdge[] {
  if (incomingEdges.length === 0 || currentEdges.length === 0) {
    return incomingEdges;
  }

  const currentById = new Map(currentEdges.map((edge) => [edge.id, edge]));
  let changed = incomingEdges.length !== currentEdges.length;
  const result: WorkflowCanvasEdge[] = [];

  for (const incoming of incomingEdges) {
    const current = currentById.get(incoming.id);
    if (!current) {
      result.push(incoming);
      changed = true;
      continue;
    }

    if (
      current.selected === incoming.selected &&
      current.source === incoming.source &&
      current.target === incoming.target &&
      current.animated === incoming.animated
    ) {
      result.push(current);
      continue;
    }

    changed = true;
    result.push({
      ...current,
      ...incoming,
    });
  }

  return changed ? result : currentEdges;
}

export function withoutProgrammaticNodeChanges(
  changes: NodeChange<WorkflowCanvasNode>[],
): NodeChange<WorkflowCanvasNode>[] {
  return changes.filter((change) => change.type !== "remove" && change.type !== "select");
}

export function withoutProgrammaticEdgeChanges(
  changes: EdgeChange<WorkflowCanvasEdge>[],
): EdgeChange<WorkflowCanvasEdge>[] {
  return changes.filter((change) => change.type !== "select");
}

export function forEachNodePositionChange(
  changes: NodeChange<WorkflowCanvasNode>[],
  onPositionChange: (nodeId: NodeId, x: number, y: number) => void,
) {
  for (const change of changes) {
    if (change.type !== "position" || !change.position || change.dragging) {
      continue;
    }

    onPositionChange(change.id, change.position.x, change.position.y);
  }
}

export function forEachRemovedEdge(
  changes: EdgeChange<WorkflowCanvasEdge>[],
  onDeleteEdge: (edgeId: EdgeId) => void,
) {
  for (const change of changes) {
    if (change.type === "remove") {
      onDeleteEdge(change.id);
    }
  }
}

export function isValidCanvasConnection(connection: { source: string | null; target: string | null }) {
  return connection.source !== null && connection.target !== null && connection.source !== connection.target;
}
