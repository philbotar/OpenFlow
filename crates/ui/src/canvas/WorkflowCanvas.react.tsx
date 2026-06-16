/** @jsxImportSource react */
/** @jsxRuntime automatic */
import {
  Background,
  BackgroundVariant,
  Controls,
  MarkerType,
  Panel,
  ReactFlow,
  ReactFlowProvider,
  useNodesInitialized,
  useReactFlow,
  type Connection,
  type Edge as FlowEdge,
  type EdgeChange,
  type Node as FlowNode,
  type NodeChange,
  type OnSelectionChangeParams,
  useEdgesState,
  useNodesState,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import * as React from "react";
import { useCallback, useEffect, useMemo, useRef } from "react";
import type { AgentStatus, EdgeId, NodeId, SubagentSummary } from "../lib/types";
import {
  NODE_HEIGHT,
  NODE_WIDTH,
  statusForNode,
  type WorkflowCanvasGraph,
  type WorkflowCanvasStatusByNode,
  type WorkflowCanvasSubagentsByNode,
} from "../lib/workflow";
import { WorkflowNode } from "./WorkflowNode.react";
type WorkflowCanvasProps = {
  graph: WorkflowCanvasGraph | null;
  selectedNodeId: NodeId | null;
  selectedEdgeId: EdgeId | null;
  statusByNode: WorkflowCanvasStatusByNode | null;
  subagentsByNode: WorkflowCanvasSubagentsByNode | null;
  chatFocusNode?: { nodeId: NodeId; tick: number } | null;
  viewportEnabled?: boolean;
  previewMode?: boolean;
  runActive?: boolean;
  colorMode?: "light" | "dark";
  onSelectNode: (nodeId: NodeId | null) => void;
  onSelectEdge: (edgeId: EdgeId | null) => void;
  onUpdateNodePosition: (nodeId: NodeId, x: number, y: number) => void;
  onCreateEdge: (from: NodeId, to: NodeId) => void;
  onReconnectEdge: (edgeId: EdgeId, from: NodeId, to: NodeId) => void;
  onDeleteEdge: (edgeId: EdgeId) => void;
  onAddNode: () => void;
  onInterruptNode?: (nodeId: NodeId) => void;
  onRetryNode?: (nodeId: NodeId) => void;
};

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

const NODE_TYPES = {
  workflowNode: WorkflowNode,
};

export const FIT_ALL_VIEWPORT_OPTIONS = {
  padding: 0.2,
  maxZoom: 1,
  duration: 200,
} as const;

export const FIT_NODE_VIEWPORT_OPTIONS = {
  padding: 0.35,
  maxZoom: 1.2,
  duration: 200,
} as const;

const NODE_FOCUS_SUPPRESS_MS = 400;

function CanvasViewportController(props: {
  workflowId: string | null;
  selectedNodeId: NodeId | null;
  chatFocusNode?: { nodeId: NodeId; tick: number } | null;
  viewportEnabled?: boolean;
}) {
  const { fitView } = useReactFlow();
  const nodesInitialized = useNodesInitialized();
  const nodesReadyRef = useRef(false);
  const previousWorkflowIdRef = useRef<string | null>(null);
  const previousSelectedNodeIdRef = useRef<NodeId | null>(null);
  const previousChatFocusTickRef = useRef(0);
  const suppressNodeFocusUntilRef = useRef(0);

  if (nodesInitialized) {
    nodesReadyRef.current = true;
  }

  useEffect(() => {
    if (!nodesReadyRef.current || props.viewportEnabled === false) {
      return;
    }

    const workflowId = props.workflowId;
    if (workflowId && workflowId !== previousWorkflowIdRef.current) {
      previousWorkflowIdRef.current = workflowId;
      previousSelectedNodeIdRef.current = props.selectedNodeId;
      previousChatFocusTickRef.current = props.chatFocusNode?.tick ?? 0;
      suppressNodeFocusUntilRef.current = performance.now() + NODE_FOCUS_SUPPRESS_MS;
      void fitView(FIT_ALL_VIEWPORT_OPTIONS);
      return;
    }

    const chatFocus = props.chatFocusNode;
    if (chatFocus && chatFocus.tick !== previousChatFocusTickRef.current) {
      previousChatFocusTickRef.current = chatFocus.tick;
      void fitView({
        ...FIT_NODE_VIEWPORT_OPTIONS,
        nodes: [{ id: chatFocus.nodeId }],
      });
      return;
    }

    const selectedNodeId = props.selectedNodeId;
    if (!selectedNodeId) {
      previousSelectedNodeIdRef.current = null;
      return;
    }

    if (selectedNodeId === previousSelectedNodeIdRef.current) {
      return;
    }

    previousSelectedNodeIdRef.current = selectedNodeId;
    if (performance.now() < suppressNodeFocusUntilRef.current) {
      return;
    }

    void fitView({
      ...FIT_NODE_VIEWPORT_OPTIONS,
      nodes: [{ id: selectedNodeId }],
    });
  }, [
    fitView,
    props.chatFocusNode,
    props.selectedNodeId,
    props.viewportEnabled,
    props.workflowId,
  ]);

  return null;
}

function edgeStrokeForTheme(colorMode: "light" | "dark") {
  return colorMode === "dark" ? "#4b5568" : "#c3cbda";
}

function backgroundDotForTheme(colorMode: "light" | "dark") {
  return colorMode === "dark" ? "rgba(255, 255, 255, 0.08)" : "rgba(24, 24, 27, 0.14)";
}

function defaultEdgeOptions(colorMode: "light" | "dark") {
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

export function withoutNodeRemovals(
  changes: NodeChange<WorkflowCanvasNode>[],
): NodeChange<WorkflowCanvasNode>[] {
  return changes.filter((change) => change.type !== "remove");
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

export function selectionIdsFromChange(
  selection: OnSelectionChangeParams<WorkflowCanvasNode, WorkflowCanvasEdge>,
) {
  return {
    selectedNodeId: selection.nodes[0]?.id ?? null,
    selectedEdgeId: selection.edges[0]?.id ?? null,
  };
}

export function shouldEmitSelectionChange(
  current: { selectedNodeId: NodeId | null; selectedEdgeId: EdgeId | null },
  next: { selectedNodeId: NodeId | null; selectedEdgeId: EdgeId | null },
): boolean {
  return (
    current.selectedNodeId !== next.selectedNodeId ||
    current.selectedEdgeId !== next.selectedEdgeId
  );
}

export function isValidCanvasConnection(connection: { source: string | null; target: string | null }) {
  return connection.source !== null && connection.target !== null && connection.source !== connection.target;
}

export function WorkflowCanvas(props: WorkflowCanvasProps) {
  const previewMode = props.previewMode ?? false;
  const externalNodes = useMemo<WorkflowCanvasNode[]>(
    () =>
      buildFlowNodes(
        props.graph,
        props.selectedNodeId,
        props.statusByNode,
        props.subagentsByNode,
        props.runActive,
        props.onInterruptNode,
        props.onRetryNode,
      ),
    [
      props.graph,
      props.selectedNodeId,
      props.statusByNode,
      props.subagentsByNode,
      props.runActive,
      props.onInterruptNode,
      props.onRetryNode,
    ],
  );

  const colorMode = props.colorMode ?? "light";
  const runActive = props.runActive ?? false;

  const externalEdges = useMemo<WorkflowCanvasEdge[]>(
    () => buildFlowEdges(props.graph, props.selectedEdgeId, runActive, colorMode),
    [props.graph, props.selectedEdgeId, runActive, colorMode],
  );

  const flowEdgeDefaults = useMemo(() => defaultEdgeOptions(colorMode), [colorMode]);

  // Use xyflow hooks for state management
  const [nodes, setNodes, onNodesChange] = useNodesState<WorkflowCanvasNode>(externalNodes);
  const [edges, setEdges, onEdgesChange] = useEdgesState<WorkflowCanvasEdge>(externalEdges);

  // Sync external updates to internal state
  useEffect(() => {
    setNodes((current) => {
      const next = reconcileFlowNodes(current, externalNodes);
      return next === current ? current : next;
    });
  }, [externalNodes, setNodes]);

  useEffect(() => {
    setEdges((current) => {
      const next = reconcileFlowEdges(current, externalEdges);
      return next === current ? current : next;
    });
  }, [externalEdges, setEdges]);

  const handleNodeClick = useCallback(
    (_event: React.MouseEvent, node: WorkflowCanvasNode) => {
      props.onSelectEdge(null);
      props.onSelectNode(node.id);
    },
    [props.onSelectEdge, props.onSelectNode],
  );

  const handleEdgeClick = useCallback(
    (_event: React.MouseEvent, edge: WorkflowCanvasEdge) => {
      props.onSelectNode(null);
      props.onSelectEdge(edge.id);
    },
    [props.onSelectEdge, props.onSelectNode],
  );

  const handleNodesChange = useCallback(
    (changes: NodeChange<WorkflowCanvasNode>[]) => {
      if (previewMode) {
        return;
      }
      const allowedChanges = withoutProgrammaticNodeChanges(changes);
      if (allowedChanges.length === 0) {
        return;
      }

      onNodesChange(allowedChanges);
      forEachNodePositionChange(allowedChanges, props.onUpdateNodePosition);
    },
    [onNodesChange, previewMode, props.onUpdateNodePosition],
  );

  const handleBeforeDelete = useCallback(() => Promise.resolve(false), []);

  const handleEdgesChange = useCallback(
    (changes: EdgeChange<WorkflowCanvasEdge>[]) => {
      if (previewMode) {
        return;
      }
      const allowedChanges = withoutProgrammaticEdgeChanges(changes);
      if (allowedChanges.length > 0) {
        onEdgesChange(allowedChanges);
      }
      forEachRemovedEdge(changes, props.onDeleteEdge);
    },
    [onEdgesChange, previewMode, props.onDeleteEdge],
  );

  const handleConnect = useCallback(
    (connection: Connection) => {
      if (previewMode || !connection.source || !connection.target) {
        return;
      }

      props.onCreateEdge(connection.source, connection.target);
    },
    [previewMode, props.onCreateEdge],
  );

  const handleReconnect = useCallback(
    (edge: WorkflowCanvasEdge, connection: Connection) => {
      if (previewMode || !connection.source || !connection.target) {
        return;
      }

      props.onReconnectEdge(edge.id, connection.source, connection.target);
    },
    [previewMode, props.onReconnectEdge],
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
      <ReactFlowProvider>
        <ReactFlow<WorkflowCanvasNode, WorkflowCanvasEdge>
          nodes={nodes}
          edges={edges}
          nodeTypes={NODE_TYPES}
          colorMode={colorMode}
          proOptions={{ hideAttribution: true }}
          defaultEdgeOptions={flowEdgeDefaults}
          onNodesChange={handleNodesChange}
          onEdgesChange={handleEdgesChange}
          onConnect={handleConnect}
          onReconnect={handleReconnect}
          onPaneClick={handlePaneClick}
          onNodeClick={handleNodeClick}
          onEdgeClick={handleEdgeClick}
          onBeforeDelete={handleBeforeDelete}
          deleteKeyCode={null}
          fitView={false}
          fitViewOptions={FIT_ALL_VIEWPORT_OPTIONS}
          minZoom={0.4}
          maxZoom={1.8}
          panOnScroll
          selectionOnDrag={false}
          nodesDraggable={!previewMode}
          nodesConnectable={!previewMode}
          edgesReconnectable={!previewMode}
          isValidConnection={isValidCanvasConnection}
          snapToGrid={!previewMode}
          snapGrid={[16, 16]}
        >
          <CanvasViewportController
            workflowId={props.graph?.id ?? null}
            selectedNodeId={props.selectedNodeId}
            chatFocusNode={props.chatFocusNode}
            viewportEnabled={props.viewportEnabled ?? true}
          />
          <Background
            gap={22}
            size={1.5}
            color={backgroundDotForTheme(colorMode)}
            variant={BackgroundVariant.Dots}
          />
          {!previewMode ? (
            <Panel position="top-left" className="workflow-flow-panel">
              <button type="button" className="secondary-button small workflow-flow-add-button" onClick={handleAddNode}>
                Add node
              </button>
            </Panel>
          ) : null}
          <Controls showInteractive={false} position="bottom-left" />
        </ReactFlow>
      </ReactFlowProvider>
    </div>
  );
}