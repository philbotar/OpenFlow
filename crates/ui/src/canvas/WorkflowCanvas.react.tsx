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
  onAddNode: () => void;
  onInterruptNode?: (nodeId: NodeId) => void;
  onRetryNode?: (nodeId: NodeId) => void;
};

const NODE_TYPES = {
  workflowNode: WorkflowNode,
};

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

  const graphSignature = useMemo(() => graphStructureSignature(props.graph), [props.graph]);

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
          {!previewMode ? (
            <Panel position="top-left" className="workflow-flow-panel">
              <button type="button" className="secondary-button small workflow-flow-action-button" onClick={handleAddNode}>
                Add node
              </button>
              <button
                type="button"
                className="secondary-button small workflow-flow-action-button"
                onClick={handleAutoLayout}
                title="Arrange workflow left to right"
              >
                Auto layout
              </button>
            </Panel>
          ) : null}
          <Controls showInteractive={false} position="bottom-left" />
        </ReactFlow>
      </ReactFlowProvider>
    </div>
  );
}
