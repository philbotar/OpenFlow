/** @jsxImportSource react */
/** @jsxRuntime automatic */
import {
  Background,
  BackgroundVariant,
  Controls,
  MarkerType,
  Panel,
  ReactFlow,
  type Connection,
  type Edge as FlowEdge,
  type EdgeChange,
  type Node as FlowNode,
  type NodeChange,
  type OnSelectionChangeParams,
  type NodeProps,
  useEdgesState,
  useNodesState,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import * as React from "react";
import { useCallback, useEffect, useMemo } from "react";
import type { AgentStatus, EdgeId, NodeId } from "../types";
import {
  NODE_HEIGHT,
  NODE_WIDTH,
  statusForNode,
  type WorkflowCanvasGraph,
  type WorkflowCanvasStatusByNode,
} from "../workflow";
import { WorkflowNode } from "./WorkflowNode.react";
type WorkflowCanvasProps = {
  graph: WorkflowCanvasGraph | null;
  selectedNodeId: NodeId | null;
  selectedEdgeId: EdgeId | null;
  statusByNode: WorkflowCanvasStatusByNode | null;
  onSelectNode: (nodeId: NodeId | null) => void;
  onSelectEdge: (edgeId: EdgeId | null) => void;
  onUpdateNodePosition: (nodeId: NodeId, x: number, y: number) => void;
  onCreateEdge: (from: NodeId, to: NodeId) => void;
  onReconnectEdge: (edgeId: EdgeId, from: NodeId, to: NodeId) => void;
  onDeleteEdge: (edgeId: EdgeId) => void;
  onAddNode: () => void;
};

export type WorkflowCanvasNodeData = {
  label: string;
  status: AgentStatus;
};

export type WorkflowCanvasNode = FlowNode<WorkflowCanvasNodeData, "workflowNode">;
export type WorkflowCanvasEdge = FlowEdge<Record<string, never>, "default">;

const NODE_TYPES = {
  workflowNode: WorkflowNode,
};

const DEFAULT_EDGE_OPTIONS = {
  markerEnd: {
    type: MarkerType.ArrowClosed,
    color: "#c3cbda",
  },
  reconnectable: true,
  style: {
    stroke: "#c3cbda",
    strokeWidth: 2,
  },
};

export function buildFlowNodes(
  graph: WorkflowCanvasGraph | null,
  selectedNodeId: NodeId | null,
  statusByNode: WorkflowCanvasStatusByNode | null,
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
    },
    draggable: true,
    selectable: true,
    width: NODE_WIDTH,
    height: NODE_HEIGHT,
  }));
}

export function buildFlowEdges(
  graph: WorkflowCanvasGraph | null,
  selectedEdgeId: EdgeId | null,
): WorkflowCanvasEdge[] {
  if (!graph) {
    return [];
  }

  return graph.edges.map((edge) => ({
    id: edge.id,
    source: edge.from,
    target: edge.to,
    selected: selectedEdgeId === edge.id,
    reconnectable: true,
    deletable: true,
    markerEnd: DEFAULT_EDGE_OPTIONS.markerEnd,
    style: DEFAULT_EDGE_OPTIONS.style,
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

  return incomingNodes.map((incoming) => {
    const current = currentById.get(incoming.id);
    if (!current) {
      return incoming;
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
      return current;
    }

    return {
      ...current,
      ...incoming,
      position,
      data,
    };
  });
}

export function reconcileFlowEdges(
  currentEdges: WorkflowCanvasEdge[],
  incomingEdges: WorkflowCanvasEdge[],
): WorkflowCanvasEdge[] {
  if (incomingEdges.length === 0 || currentEdges.length === 0) {
    return incomingEdges;
  }

  const currentById = new Map(currentEdges.map((edge) => [edge.id, edge]));

  return incomingEdges.map((incoming) => {
    const current = currentById.get(incoming.id);
    if (!current) {
      return incoming;
    }

    if (
      current.selected === incoming.selected &&
      current.source === incoming.source &&
      current.target === incoming.target
    ) {
      return current;
    }

    return {
      ...current,
      ...incoming,
    };
  });
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

export function selectionIdsFromChange(
  selection: OnSelectionChangeParams<WorkflowCanvasNode, WorkflowCanvasEdge>,
) {
  return {
    selectedNodeId: selection.nodes[0]?.id ?? null,
    selectedEdgeId: selection.edges[0]?.id ?? null,
  };
}

export function isValidCanvasConnection(connection: { source: string | null; target: string | null }) {
  return connection.source !== null && connection.target !== null && connection.source !== connection.target;
}

export function WorkflowCanvas(props: WorkflowCanvasProps) {
  const externalNodes = useMemo<WorkflowCanvasNode[]>(
    () => buildFlowNodes(props.graph, props.selectedNodeId, props.statusByNode),
    [props.graph, props.selectedNodeId, props.statusByNode],
  );

  const externalEdges = useMemo<WorkflowCanvasEdge[]>(
    () => buildFlowEdges(props.graph, props.selectedEdgeId),
    [props.graph, props.selectedEdgeId],
  );

  // Use xyflow hooks for state management
  const [nodes, setNodes, onNodesChange] = useNodesState<WorkflowCanvasNode>(externalNodes);
  const [edges, setEdges, onEdgesChange] = useEdgesState<WorkflowCanvasEdge>(externalEdges);

  // Sync external updates to internal state
  useEffect(() => {
    setNodes((current) => reconcileFlowNodes(current, externalNodes));
  }, [externalNodes, setNodes]);

  useEffect(() => {
    setEdges((current) => reconcileFlowEdges(current, externalEdges));
  }, [externalEdges, setEdges]);

  const handleSelectionChange = useCallback(
    (selection: OnSelectionChangeParams<WorkflowCanvasNode, WorkflowCanvasEdge>) => {
      const { selectedNodeId, selectedEdgeId } = selectionIdsFromChange(selection);
      props.onSelectEdge(selectedEdgeId);
      props.onSelectNode(selectedNodeId);
    },
    [props.onSelectEdge, props.onSelectNode],
  );

  const handleNodesChange = useCallback(
    (changes: NodeChange<WorkflowCanvasNode>[]) => {
      onNodesChange(changes);
      forEachNodePositionChange(changes, props.onUpdateNodePosition);
    },
    [onNodesChange, props.onUpdateNodePosition],
  );

  const handleEdgesChange = useCallback(
    (changes: EdgeChange<WorkflowCanvasEdge>[]) => {
      onEdgesChange(changes);
      forEachRemovedEdge(changes, props.onDeleteEdge);
    },
    [onEdgesChange, props.onDeleteEdge],
  );

  const handleConnect = useCallback(
    (connection: Connection) => {
      if (!connection.source || !connection.target) {
        return;
      }

      props.onCreateEdge(connection.source, connection.target);
    },
    [props.onCreateEdge],
  );

  const handleReconnect = useCallback(
    (edge: WorkflowCanvasEdge, connection: Connection) => {
      if (!connection.source || !connection.target) {
        return;
      }

      props.onReconnectEdge(edge.id, connection.source, connection.target);
    },
    [props.onReconnectEdge],
  );

  const handlePaneClick = useCallback(() => {
    props.onSelectEdge(null);
    props.onSelectNode(null);
  }, [props.onSelectEdge, props.onSelectNode]);

  const handleAddNode = useCallback(() => {
    props.onAddNode();
  }, [props.onAddNode]);

  return (
    <div className="workflow-flow-shell">
      <ReactFlow<WorkflowCanvasNode, WorkflowCanvasEdge>
        nodes={nodes}
        edges={edges}
        nodeTypes={NODE_TYPES}
        defaultEdgeOptions={DEFAULT_EDGE_OPTIONS}
        onNodesChange={handleNodesChange}
        onEdgesChange={handleEdgesChange}
        onConnect={handleConnect}
        onReconnect={handleReconnect}
        onPaneClick={handlePaneClick}
        onSelectionChange={handleSelectionChange}
        deleteKeyCode={null}
        fitView={false}
        minZoom={0.4}
        maxZoom={1.8}
        panOnScroll
        selectionOnDrag={false}
        edgesReconnectable
        isValidConnection={isValidCanvasConnection}
        snapToGrid={true}
        snapGrid={[16, 16]}
      >
        <Background gap={22} size={1.5} color="rgba(24, 24, 27, 0.14)" variant={BackgroundVariant.Dots} />
        <Panel position="top-left" className="workflow-flow-panel">
          <button type="button" className="secondary-button small workflow-flow-add-button" onClick={handleAddNode}>
            Add node
          </button>
        </Panel>
        <Controls showInteractive={false} position="bottom-left" />

      </ReactFlow>
    </div>
  );
}