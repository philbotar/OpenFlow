// @vitest-environment jsdom
import { render } from "@testing-library/react";
import { createElement } from "react";
import { beforeEach, expect, test, vi } from "vitest";
import { CanvasViewportController, FIT_ALL_VIEWPORT_OPTIONS } from "./workflowCanvasViewport";

const flowMocks = vi.hoisted(() => ({
  fitView: vi.fn(),
  getZoom: vi.fn(() => 0.8),
  nodesInitialized: false,
  paneWidth: 800,
  paneHeight: 600,
  zoomTo: vi.fn(),
}));

vi.mock("@xyflow/react", () => ({
  useNodesInitialized: () => flowMocks.nodesInitialized,
  useReactFlow: () => ({
    fitView: flowMocks.fitView,
    getZoom: flowMocks.getZoom,
    zoomTo: flowMocks.zoomTo,
  }),
  useStore: (selector: (state: { width: number; height: number }) => unknown) =>
    selector({ width: flowMocks.paneWidth, height: flowMocks.paneHeight }),
}));

beforeEach(() => {
  flowMocks.fitView.mockReset();
  flowMocks.getZoom.mockReset();
  flowMocks.getZoom.mockReturnValue(0.8);
  flowMocks.nodesInitialized = false;
  flowMocks.paneWidth = 800;
  flowMocks.paneHeight = 600;
  flowMocks.zoomTo.mockReset();
  vi.useRealTimers();
});

test("reserves left clearance for the canvas toolbar when fitting a workflow", () => {
  expect(FIT_ALL_VIEWPORT_OPTIONS.padding).toEqual({
    top: 0.2,
    right: 0.2,
    bottom: 0.2,
    left: "84px",
  });
});

test("fits a workflow after React Flow finishes measuring its nodes", () => {
  const props = {
    workflowId: "workflow-1",
    graphSignature: "nodes:node-1,node-2|edges:edge-1:node-1->node-2",
    selectedNodeId: null,
  };
  const view = render(createElement(CanvasViewportController, props));

  expect(flowMocks.fitView).not.toHaveBeenCalled();

  flowMocks.nodesInitialized = true;
  view.rerender(createElement(CanvasViewportController, props));

  expect(flowMocks.fitView).toHaveBeenCalledWith(FIT_ALL_VIEWPORT_OPTIONS);
});

test("fits a workflow at the app zoom inside React Flow", async () => {
  flowMocks.nodesInitialized = true;
  const canvasRef = {
    current: {
      getBoundingClientRect: () => ({ left: 0 }) as DOMRect,
    } as HTMLElement,
  };
  const toolbarRef = {
    current: {
      getBoundingClientRect: () => ({ right: 60 }) as DOMRect,
    } as HTMLElement,
  };

  render(
    createElement(CanvasViewportController, {
      workflowId: "workflow-1",
      graphSignature: "nodes:node-1,node-2|edges:edge-1:node-1->node-2",
      selectedNodeId: null,
      uiZoom: 1.3,
      canvasRef,
      toolbarRef,
    }),
  );

  await vi.waitFor(() => {
    expect(flowMocks.zoomTo).toHaveBeenCalledWith(1.04, { duration: 200 });
  });
  expect(flowMocks.fitView.mock.invocationCallOrder[0]).toBeLessThan(
    flowMocks.zoomTo.mock.invocationCallOrder[0],
  );
  expect(flowMocks.fitView).toHaveBeenCalledWith({
    ...FIT_ALL_VIEWPORT_OPTIONS,
    padding: {
      ...FIT_ALL_VIEWPORT_OPTIONS.padding,
      left: "157px",
    },
    minZoom: 0.4,
    duration: 0,
  });
});

test("rescales the current React Flow viewport when app zoom changes", async () => {
  flowMocks.nodesInitialized = true;
  flowMocks.getZoom.mockReturnValue(1.04);
  const props = {
    workflowId: null,
    graphSignature: "graph:none",
    selectedNodeId: null,
    uiZoom: 1.3,
  };
  const view = render(createElement(CanvasViewportController, props));

  view.rerender(
    createElement(CanvasViewportController, {
      ...props,
      uiZoom: 1.4,
    }),
  );

  await vi.waitFor(() => {
    expect(flowMocks.zoomTo).toHaveBeenCalledTimes(1);
  });
  expect(flowMocks.zoomTo.mock.calls[0][0]).toBeCloseTo(1.12);
  expect(flowMocks.zoomTo.mock.calls[0][1]).toEqual({ duration: 0 });
});

test("fits the graph when the React Flow pane resizes", () => {
  vi.useFakeTimers();
  flowMocks.nodesInitialized = true;
  const props = {
    workflowId: "workflow-1",
    graphSignature: "nodes:node-1|edges:",
    selectedNodeId: null,
  };
  const view = render(createElement(CanvasViewportController, props));
  flowMocks.fitView.mockClear();

  flowMocks.paneHeight = 400;
  view.rerender(createElement(CanvasViewportController, props));
  expect(flowMocks.fitView).not.toHaveBeenCalled();

  vi.advanceTimersByTime(120);
  expect(flowMocks.fitView).toHaveBeenCalledWith(FIT_ALL_VIEWPORT_OPTIONS);
});
