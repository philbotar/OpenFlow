/** @jsxImportSource react */
/** @jsxRuntime automatic */
import { useNodesInitialized, useReactFlow } from "@xyflow/react";
import { useEffect, useRef } from "react";
import type { NodeId } from "../lib/types";

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

const GRAPH_AUTO_CENTER_DEBOUNCE_MS = 120;
const NODE_FOCUS_SUPPRESS_MS = 400;

export function CanvasViewportController(props: {
  workflowId: string | null;
  graphSignature: string;
  selectedNodeId: NodeId | null;
  chatFocusNode?: { nodeId: NodeId; tick: number } | null;
  viewportEnabled?: boolean;
}) {
  const { fitView } = useReactFlow();
  const nodesInitialized = useNodesInitialized();
  const nodesReadyRef = useRef(false);
  const previousWorkflowIdRef = useRef<string | null>(null);
  const previousGraphSignatureRef = useRef<string | null>(null);
  const previousSelectedNodeIdRef = useRef<NodeId | null>(null);
  const previousChatFocusTickRef = useRef(0);
  const suppressNodeFocusUntilRef = useRef(0);
  const graphAutoCenterTimerRef = useRef<number | null>(null);

  if (nodesInitialized) {
    nodesReadyRef.current = true;
  }

  useEffect(() => {
    return () => {
      if (graphAutoCenterTimerRef.current) {
        window.clearTimeout(graphAutoCenterTimerRef.current);
      }
    };
  }, []);

  useEffect(() => {
    if (!nodesReadyRef.current || props.viewportEnabled === false) {
      return;
    }

    const workflowId = props.workflowId;
    if (workflowId && workflowId !== previousWorkflowIdRef.current) {
      previousWorkflowIdRef.current = workflowId;
      previousGraphSignatureRef.current = props.graphSignature;
      previousSelectedNodeIdRef.current = props.selectedNodeId;
      previousChatFocusTickRef.current = props.chatFocusNode?.tick ?? 0;
      suppressNodeFocusUntilRef.current = performance.now() + NODE_FOCUS_SUPPRESS_MS;
      void fitView(FIT_ALL_VIEWPORT_OPTIONS);
      return;
    }

    if (
      previousGraphSignatureRef.current !== null &&
      props.graphSignature !== previousGraphSignatureRef.current
    ) {
      previousGraphSignatureRef.current = props.graphSignature;
      suppressNodeFocusUntilRef.current = performance.now() + NODE_FOCUS_SUPPRESS_MS;
      if (graphAutoCenterTimerRef.current) {
        window.clearTimeout(graphAutoCenterTimerRef.current);
      }
      graphAutoCenterTimerRef.current = window.setTimeout(() => {
        graphAutoCenterTimerRef.current = null;
        void fitView(FIT_ALL_VIEWPORT_OPTIONS);
      }, GRAPH_AUTO_CENTER_DEBOUNCE_MS);
      return;
    }
    previousGraphSignatureRef.current = props.graphSignature;

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
    props.graphSignature,
    props.selectedNodeId,
    props.viewportEnabled,
    props.workflowId,
  ]);

  return null;
}
