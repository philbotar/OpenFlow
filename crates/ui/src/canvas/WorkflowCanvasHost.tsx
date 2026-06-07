import { createEffect, onCleanup, onMount } from "solid-js";
import { createElement } from "react";
import { createRoot, type Root } from "react-dom/client";
import type { EdgeId, NodeId, SubagentSummary } from "../lib/types";
import type { WorkflowCanvasGraph, WorkflowCanvasStatusByNode, WorkflowCanvasSubagentsByNode } from "../lib/workflow";
import { WorkflowCanvas } from "./WorkflowCanvas.react";

type WorkflowCanvasHostProps = {
  graph: WorkflowCanvasGraph | null;
  selectedNodeId: NodeId | null;
  selectedEdgeId: EdgeId | null;
  statusByNode: WorkflowCanvasStatusByNode | null;
  subagentsByNode: WorkflowCanvasSubagentsByNode | null;
  onSelectNode: (nodeId: NodeId | null) => void;
  onSelectEdge: (edgeId: EdgeId | null) => void;
  onUpdateNodePosition: (nodeId: NodeId, x: number, y: number) => void;
  onCreateEdge: (from: NodeId, to: NodeId) => void;
  onReconnectEdge: (edgeId: EdgeId, from: NodeId, to: NodeId) => void;
  onDeleteEdge: (edgeId: EdgeId) => void;
  onAddNode: () => void;
};

function WorkflowCanvasHost(props: WorkflowCanvasHostProps) {
  let containerRef: HTMLDivElement | undefined;
  let root: Root | undefined;

  onMount(() => {
    root = createRoot(containerRef!);
  });

  createEffect(() => {
    root?.render(
      createElement(WorkflowCanvas, {
        graph: props.graph,
        selectedNodeId: props.selectedNodeId,
        selectedEdgeId: props.selectedEdgeId,
        statusByNode: props.statusByNode,
        subagentsByNode: props.subagentsByNode,
        onSelectNode: props.onSelectNode,
        onSelectEdge: props.onSelectEdge,
        onUpdateNodePosition: props.onUpdateNodePosition,
        onCreateEdge: props.onCreateEdge,
        onReconnectEdge: props.onReconnectEdge,
        onDeleteEdge: props.onDeleteEdge,
        onAddNode: props.onAddNode,
      }),
    );
  });

  onCleanup(() => {
    root?.unmount();
  });

  return <div class="canvas-board" ref={containerRef} />;
}

export default WorkflowCanvasHost;
