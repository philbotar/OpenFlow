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

function WorkflowCanvasHost(props: WorkflowCanvasHostProps) {
  let containerRef: HTMLDivElement | undefined;
  let root: Root | undefined;

  const renderCanvas = () => {
    if (!root) {
      return;
    }
    root.render(
      createElement(WorkflowCanvas, {
        graph: props.graph,
        selectedNodeId: props.selectedNodeId,
        selectedEdgeId: props.selectedEdgeId,
        statusByNode: props.statusByNode,
        subagentsByNode: props.subagentsByNode,
        chatFocusNode: props.chatFocusNode,
        viewportEnabled: props.viewportEnabled,
        previewMode: props.previewMode,
        runActive: props.runActive,
        colorMode: props.colorMode,
        onSelectNode: props.onSelectNode,
        onSelectEdge: props.onSelectEdge,
        onUpdateNodePosition: props.onUpdateNodePosition,
        onAutoLayout: props.onAutoLayout,
        onCreateEdge: props.onCreateEdge,
        onReconnectEdge: props.onReconnectEdge,
        onDeleteEdge: props.onDeleteEdge,
        onAddNode: props.onAddNode,
        onInterruptNode: props.onInterruptNode,
        onRetryNode: props.onRetryNode,
      }),
    );
  };

  onMount(() => {
    root = createRoot(containerRef!);
    renderCanvas();
  });

  createEffect(() => {
    renderCanvas();
  });

  onCleanup(() => {
    root?.unmount();
  });

  return <div class="canvas-board" ref={containerRef} />;
}

export default WorkflowCanvasHost;
