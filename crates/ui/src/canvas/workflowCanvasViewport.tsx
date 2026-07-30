/** @jsxImportSource react */
/** @jsxRuntime automatic */
import {
  useNodesInitialized,
  useReactFlow,
  useStore,
  type FitViewOptions,
} from "@xyflow/react";
import { useCallback, useEffect, useRef, type RefObject } from "react";
import type { NodeId } from "../lib/types";

const CANVAS_TOOLBAR_BASE_CLEARANCE_PX = 84;
const CANVAS_TOOLBAR_NODE_GAP_PX = 24;

export const FIT_ALL_VIEWPORT_OPTIONS = {
  padding: {
    top: 0.2,
    right: 0.2,
    bottom: 0.2,
    // 16px panel margin + 44px toolbar + 24px node gap at 100% UI zoom.
    left: `${CANVAS_TOOLBAR_BASE_CLEARANCE_PX}px`,
  },
  maxZoom: 1,
  duration: 200,
} as const;

export const FIT_NODE_VIEWPORT_OPTIONS = {
  padding: 0.35,
  maxZoom: 1.2,
  duration: 200,
} as const;

export const CANVAS_MIN_ZOOM = 0.4;
export const CANVAS_MAX_ZOOM = 1.8;

const GRAPH_AUTO_CENTER_DEBOUNCE_MS = 120;
const NODE_FOCUS_SUPPRESS_MS = 400;

function withCanvasToolbarClearance(
  options: FitViewOptions | undefined,
  uiZoom: number,
  paneWidth: number,
  canvasRef?: RefObject<HTMLElement | null>,
  toolbarRef?: RefObject<HTMLElement | null>,
): FitViewOptions | undefined {
  if (!options?.padding || typeof options.padding !== "object") {
    return options;
  }

  const canvas = canvasRef?.current;
  const toolbar = toolbarRef?.current;
  if (!canvas || !toolbar || paneWidth <= 0 || uiZoom <= 0) {
    return options;
  }

  const canvasRect = canvas.getBoundingClientRect();
  const toolbarRect = toolbar.getBoundingClientRect();
  const visibleClearance =
    toolbarRect.right - canvasRect.left + CANVAS_TOOLBAR_NODE_GAP_PX;
  const paneCenter = paneWidth / 2;
  const preZoomClearance =
    paneCenter + (visibleClearance - paneCenter) / uiZoom;
  const leftPadding = Math.max(
    CANVAS_TOOLBAR_BASE_CLEARANCE_PX,
    Math.ceil(preZoomClearance),
  );

  return {
    ...options,
    padding: {
      ...options.padding,
      left: `${leftPadding}px`,
    },
  };
}

export function CanvasViewportController(props: {
  workflowId: string | null;
  graphSignature: string;
  selectedNodeId: NodeId | null;
  chatFocusNode?: { nodeId: NodeId; tick: number } | null;
  viewportEnabled?: boolean;
  uiZoom?: number;
  canvasRef?: RefObject<HTMLElement | null>;
  toolbarRef?: RefObject<HTMLElement | null>;
}) {
  const { fitView, getZoom, zoomTo } = useReactFlow();
  const nodesInitialized = useNodesInitialized();
  const paneWidth = useStore((state) => state.width);
  const paneHeight = useStore((state) => state.height);
  const previousWorkflowIdRef = useRef<string | null>(null);
  const previousGraphSignatureRef = useRef<string | null>(null);
  const previousSelectedNodeIdRef = useRef<NodeId | null>(null);
  const previousChatFocusTickRef = useRef(0);
  const previousPaneSizeRef = useRef<{ width: number; height: number } | null>(null);
  const uiZoomRef = useRef(props.uiZoom ?? 1);
  const suppressNodeFocusUntilRef = useRef(0);
  const graphAutoCenterTimerRef = useRef<number | null>(null);
  const paneResizeTimerRef = useRef<number | null>(null);
  const fitViewAtUiZoom = useCallback(
    async (options: Parameters<typeof fitView>[0]) => {
      const uiZoom = uiZoomRef.current;
      const optionsWithToolbarClearance = withCanvasToolbarClearance(
        options,
        uiZoom,
        paneWidth,
        props.canvasRef,
        props.toolbarRef,
      );
      if (uiZoom === 1) {
        await fitView(optionsWithToolbarClearance);
        return;
      }

      await fitView({
        ...optionsWithToolbarClearance,
        minZoom: CANVAS_MIN_ZOOM,
        duration: 0,
      });
      await zoomTo(getZoom() * uiZoom, {
        duration: optionsWithToolbarClearance?.duration,
      });
    },
    [fitView, getZoom, paneWidth, props.canvasRef, props.toolbarRef, zoomTo],
  );

  useEffect(() => {
    return () => {
      if (graphAutoCenterTimerRef.current) {
        window.clearTimeout(graphAutoCenterTimerRef.current);
      }
      if (paneResizeTimerRef.current) {
        window.clearTimeout(paneResizeTimerRef.current);
      }
    };
  }, []);

  useEffect(() => {
    const nextUiZoom = props.uiZoom ?? 1;
    const previousUiZoom = uiZoomRef.current;
    uiZoomRef.current = nextUiZoom;
    if (!nodesInitialized || nextUiZoom === previousUiZoom) {
      return;
    }

    void zoomTo(getZoom() * (nextUiZoom / previousUiZoom), { duration: 0 });
  }, [getZoom, nodesInitialized, props.uiZoom, zoomTo]);

  useEffect(() => {
    if (!nodesInitialized || props.viewportEnabled === false) {
      return;
    }
    if (paneWidth <= 0 || paneHeight <= 0) {
      return;
    }

    const previous = previousPaneSizeRef.current;
    previousPaneSizeRef.current = { width: paneWidth, height: paneHeight };
    // First measure is covered by the workflow/graph fit effect below.
    if (!previous || (previous.width === paneWidth && previous.height === paneHeight)) {
      return;
    }

    if (paneResizeTimerRef.current) {
      window.clearTimeout(paneResizeTimerRef.current);
    }
    paneResizeTimerRef.current = window.setTimeout(() => {
      paneResizeTimerRef.current = null;
      suppressNodeFocusUntilRef.current = performance.now() + NODE_FOCUS_SUPPRESS_MS;
      void fitViewAtUiZoom(FIT_ALL_VIEWPORT_OPTIONS);
    }, GRAPH_AUTO_CENTER_DEBOUNCE_MS);
  }, [fitViewAtUiZoom, nodesInitialized, paneHeight, paneWidth, props.viewportEnabled]);

  useEffect(() => {
    if (!nodesInitialized || props.viewportEnabled === false) {
      return;
    }

    const workflowId = props.workflowId;
    if (workflowId && workflowId !== previousWorkflowIdRef.current) {
      previousWorkflowIdRef.current = workflowId;
      previousGraphSignatureRef.current = props.graphSignature;
      previousSelectedNodeIdRef.current = props.selectedNodeId;
      previousChatFocusTickRef.current = props.chatFocusNode?.tick ?? 0;
      suppressNodeFocusUntilRef.current = performance.now() + NODE_FOCUS_SUPPRESS_MS;
      void fitViewAtUiZoom(FIT_ALL_VIEWPORT_OPTIONS);
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
        void fitViewAtUiZoom(FIT_ALL_VIEWPORT_OPTIONS);
      }, GRAPH_AUTO_CENTER_DEBOUNCE_MS);
      return;
    }
    previousGraphSignatureRef.current = props.graphSignature;

    const chatFocus = props.chatFocusNode;
    if (chatFocus && chatFocus.tick !== previousChatFocusTickRef.current) {
      previousChatFocusTickRef.current = chatFocus.tick;
      void fitViewAtUiZoom({
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

    void fitViewAtUiZoom({
      ...FIT_NODE_VIEWPORT_OPTIONS,
      nodes: [{ id: selectedNodeId }],
    });
  }, [
    fitViewAtUiZoom,
    nodesInitialized,
    props.chatFocusNode,
    props.graphSignature,
    props.selectedNodeId,
    props.viewportEnabled,
    props.workflowId,
  ]);

  return null;
}
