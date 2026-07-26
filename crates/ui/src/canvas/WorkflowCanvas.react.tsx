/** @jsxImportSource react */
/** @jsxRuntime automatic */
import {
  Background,
  BackgroundVariant,
  Controls,
  Panel,
  ReactFlow,
  ReactFlowProvider,
  type Connection,
  type EdgeChange,
  type NodeChange,
  useEdgesState,
  useNodesState,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import * as React from "react";
import { useCallback, useEffect, useMemo } from "react";
import type { EdgeId, NodeId } from "../lib/types";
import type { WorkflowCanvasGraph, WorkflowCanvasStatusByNode, WorkflowCanvasSubagentsByNode } from "../lib/workflow";
import { WorkflowNode } from "./WorkflowNode.react";
import {
  backgroundDotForTheme,
  buildFlowEdges,
  buildFlowNodes,
  defaultEdgeOptions,
  forEachNodePositionChange,
  forEachRemovedEdge,
  graphStructureSignature,
  isValidCanvasConnection,
  reconcileFlowEdges,
  reconcileFlowNodes,
  withoutProgrammaticEdgeChanges,
  withoutProgrammaticNodeChanges,
  type WorkflowCanvasEdge,
  type WorkflowCanvasNode,
} from "./workflowCanvasGraph";
import { CanvasViewportController, FIT_ALL_VIEWPORT_OPTIONS } from "./workflowCanvasViewport";

export type { WorkflowCanvasEdge, WorkflowCanvasNode, WorkflowCanvasNodeData } from "./workflowCanvasGraph";
export {
  buildFlowEdges,
  buildFlowNodes,
  forEachNodePositionChange,
  forEachRemovedEdge,
  graphStructureSignature,
  isValidCanvasConnection,
  reconcileFlowEdges,
  reconcileFlowNodes,
  withoutProgrammaticNodeChanges,
} from "./workflowCanvasGraph";

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
  onAutoLayout: () => void;
  onCreateEdge: (from: NodeId, to: NodeId) => void;
  onReconnectEdge: (edgeId: EdgeId, from: NodeId, to: NodeId) => void;
  onDeleteEdge: (edgeId: EdgeId) => void;
  onDeleteNode: (nodeId: NodeId) => void;
  onAddNode: () => void;
  onInterruptNode?: (nodeId: NodeId) => void;
  onRetryNode?: (nodeId: NodeId) => void;
};

const NODE_TYPES = {
  workflowNode: WorkflowNode,
};

function AddNodeIcon() {
  return (
    <svg
      className="workflow-flow-action-icon"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      focusable="false"
    >
      <path d="M5 12h14" />
      <path d="M12 5v14" />
    </svg>
  );
}

function AutoLayoutIcon() {
  return (
    <svg
      className="workflow-flow-action-icon"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      focusable="false"
    >
      <rect width="7" height="7" x="3" y="3" rx="1" />
      <rect width="7" height="7" x="14" y="3" rx="1" />
      <rect width="7" height="7" x="14" y="14" rx="1" />
      <rect width="7" height="7" x="3" y="14" rx="1" />
    </svg>
  );
}

export function WorkflowCanvas(props: WorkflowCanvasProps) {
  const previewMode = props.previewMode ?? false;
  const runActive = props.runActive ?? false;
  const editingLocked = previewMode || runActive;
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
  const externalEdges = useMemo<WorkflowCanvasEdge[]>(
    () => buildFlowEdges(props.graph, props.selectedEdgeId, runActive, colorMode),
    [props.graph, props.selectedEdgeId, runActive, colorMode],
  );

  const graphSignature = useMemo(() => graphStructureSignature(props.graph), [props.graph]);
  const selectedNode = useMemo(
    () => props.graph?.nodes.find((node) => node.id === props.selectedNodeId) ?? null,
    [props.graph, props.selectedNodeId],
  );
  const selectedNodeConnectionCount = useMemo(
    () =>
      selectedNode
        ? (props.graph?.edges.filter(
            (edge) => edge.from === selectedNode.id || edge.to === selectedNode.id,
          ).length ?? 0)
        : 0,
    [props.graph, selectedNode],
  );
  const selectedEdge = useMemo(
    () => props.graph?.edges.find((edge) => edge.id === props.selectedEdgeId) ?? null,
    [props.graph, props.selectedEdgeId],
  );
  const selectedEdgeLabel = useMemo(() => {
    if (!selectedEdge || !props.graph) {
      return "Connection";
    }
    const source = props.graph.nodes.find((node) => node.id === selectedEdge.from)?.label;
    const target = props.graph.nodes.find((node) => node.id === selectedEdge.to)?.label;
    return source && target ? `${source} → ${target}` : "Connection";
  }, [props.graph, selectedEdge]);

  const flowEdgeDefaults = useMemo(() => defaultEdgeOptions(colorMode), [colorMode]);

  const [nodes, setNodes, onNodesChange] = useNodesState<WorkflowCanvasNode>(externalNodes);
  const [edges, setEdges, onEdgesChange] = useEdgesState<WorkflowCanvasEdge>(externalEdges);

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
      if (editingLocked) {
        return;
      }
      const allowedChanges = withoutProgrammaticNodeChanges(changes);
      if (allowedChanges.length === 0) {
        return;
      }

      onNodesChange(allowedChanges);
      forEachNodePositionChange(allowedChanges, props.onUpdateNodePosition);
    },
    [editingLocked, onNodesChange, props.onUpdateNodePosition],
  );

  const handleBeforeDelete = useCallback(
    ({
      nodes: nodesToDelete,
      edges: edgesToDelete,
    }: {
      nodes: WorkflowCanvasNode[];
      edges: WorkflowCanvasEdge[];
    }) => {
      if (editingLocked) {
        return Promise.resolve(false);
      }
      const node = nodesToDelete[0];
      if (node) {
        props.onDeleteNode(node.id);
        return Promise.resolve(false);
      }
      for (const edge of edgesToDelete) {
        props.onDeleteEdge(edge.id);
      }
      return Promise.resolve(false);
    },
    [editingLocked, props.onDeleteEdge, props.onDeleteNode],
  );

  const handleEdgesChange = useCallback(
    (changes: EdgeChange<WorkflowCanvasEdge>[]) => {
      if (editingLocked) {
        return;
      }
      const allowedChanges = withoutProgrammaticEdgeChanges(changes);
      if (allowedChanges.length > 0) {
        onEdgesChange(allowedChanges);
      }
      forEachRemovedEdge(changes, props.onDeleteEdge);
    },
    [editingLocked, onEdgesChange, props.onDeleteEdge],
  );

  const handleConnect = useCallback(
    (connection: Connection) => {
      if (editingLocked || !connection.source || !connection.target) {
        return;
      }

      props.onCreateEdge(connection.source, connection.target);
    },
    [editingLocked, props.onCreateEdge],
  );

  const handleReconnect = useCallback(
    (edge: WorkflowCanvasEdge, connection: Connection) => {
      if (editingLocked || !connection.source || !connection.target) {
        return;
      }

      props.onReconnectEdge(edge.id, connection.source, connection.target);
    },
    [editingLocked, props.onReconnectEdge],
  );

  const handlePaneClick = useCallback(() => {
    props.onSelectEdge(null);
    props.onSelectNode(null);
  }, [props.onSelectEdge, props.onSelectNode]);

  const handleAddNode = useCallback(() => {
    props.onAddNode();
  }, [props.onAddNode]);

  const handleAutoLayout = useCallback(() => {
    props.onAutoLayout();
  }, [props.onAutoLayout]);

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
          deleteKeyCode={["Backspace", "Delete"]}
          fitView={false}
          fitViewOptions={FIT_ALL_VIEWPORT_OPTIONS}
          minZoom={0.4}
          maxZoom={1.8}
          panOnScroll
          selectionOnDrag={false}
          nodesDraggable={!editingLocked}
          nodesConnectable={!editingLocked}
          edgesReconnectable={!editingLocked}
          isValidConnection={isValidCanvasConnection}
          snapToGrid={!editingLocked}
          snapGrid={[16, 16]}
        >
          <CanvasViewportController
            workflowId={props.graph?.id ?? null}
            graphSignature={graphSignature}
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
          {!editingLocked ? (
            <Panel position="top-left" className="workflow-flow-panel">
              <div
                className="workflow-flow-toolbar"
                role="toolbar"
                aria-label="Workflow editing tools"
                aria-orientation="vertical"
              >
                <button
                  type="button"
                  className="workflow-flow-action-button"
                  onClick={handleAddNode}
                  aria-label="Add node"
                  title="Add node"
                >
                  <AddNodeIcon />
                </button>
                <button
                  type="button"
                  className="workflow-flow-action-button"
                  onClick={handleAutoLayout}
                  aria-label="Auto layout"
                  title="Auto layout"
                >
                  <AutoLayoutIcon />
                </button>
              </div>
            </Panel>
          ) : null}
          {runActive ? (
            <Panel position="top-left" className="workflow-flow-lock-panel">
              <span className="workflow-flow-lock-dot" aria-hidden="true" />
              Running · Editing locked
            </Panel>
          ) : null}
          {!editingLocked && selectedNode ? (
            <Panel position="top-center" className="workflow-flow-selection-panel">
              <span className="workflow-flow-selection-label">{selectedNode.label}</span>
              <button
                type="button"
                className="workflow-flow-delete-button"
                onClick={() => props.onDeleteNode(selectedNode.id)}
                aria-label={`Delete ${selectedNode.label}${
                  selectedNodeConnectionCount === 0
                    ? ""
                    : ` and ${selectedNodeConnectionCount} connection${
                        selectedNodeConnectionCount === 1 ? "" : "s"
                      }`
                }`}
              >
                Delete
              </button>
            </Panel>
          ) : null}
          {!editingLocked && selectedEdge ? (
            <Panel position="top-center" className="workflow-flow-selection-panel">
              <span className="workflow-flow-selection-label">{selectedEdgeLabel}</span>
              <button
                type="button"
                className="workflow-flow-delete-button"
                onClick={() => props.onDeleteEdge(selectedEdge.id)}
                aria-label="Delete connection"
              >
                Delete
              </button>
            </Panel>
          ) : null}
          <Controls showInteractive={false} position="bottom-left" />
        </ReactFlow>
      </ReactFlowProvider>
    </div>
  );
}
