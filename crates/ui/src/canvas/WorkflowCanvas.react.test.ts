// @vitest-environment jsdom
import type { EdgeChange, NodeChange } from "@xyflow/react";
import { fireEvent, render, screen } from "@testing-library/react";
import { createElement } from "react";
import { afterEach, describe, expect, test, vi } from "vitest";
import type { Workflow } from "../lib/types";
import {
  createEmptyToolConfig,
  projectWorkflowCanvasGraph,
  type WorkflowCanvasStatusByNode,
} from "../lib/workflow";
import {
  WorkflowCanvas,
  buildFlowEdges,
  buildFlowNodes,
  forEachNodePositionChange,
  forEachRemovedEdge,
  isValidCanvasConnection,
  reconcileFlowNodes,
  selectionIdsFromChange,
  type WorkflowCanvasEdge,
  type WorkflowCanvasNode,
} from "./WorkflowCanvas.react";

const workflow: Workflow = {
  id: "workflow-1",
  name: "Smoke workflow",
  nodes: [
    {
      id: "node-1",
      label: "Plan",
      kind: "Agent",
      position: { x: 96, y: 96 },
      agent: {
        system_prompt: "system",
        task_prompt: "task",
        model: "gpt-4o-mini",
        output_schema: { type: "object" },
        auto_start: true,
        tools: createEmptyToolConfig(),
      },
    },
    {
      id: "node-2",
      label: "Draft",
      kind: "Agent",
      position: { x: 496, y: 96 },
      agent: {
        system_prompt: "system",
        task_prompt: "task",
        model: "gpt-4o-mini",
        output_schema: { type: "object" },
        auto_start: true,
        tools: createEmptyToolConfig(),
      },
    },
  ],
  edges: [{ id: "edge-1", from: "node-1", to: "node-2" }],
};

const statusByNode: WorkflowCanvasStatusByNode = {
  "node-1": "completed",
  "node-2": "awaiting_input",
};

const graph = projectWorkflowCanvasGraph(workflow)!;

afterEach(() => {
  document.body.innerHTML = "";
});

if (!("ResizeObserver" in globalThis)) {
  class ResizeObserver {
    observe() {}

    unobserve() {}

    disconnect() {}
  }

  vi.stubGlobal("ResizeObserver", ResizeObserver);
}

describe("WorkflowCanvas adapter helpers", () => {
  test("buildFlowNodes preserves positions and selected status", () => {
    const nodes = buildFlowNodes(graph, "node-2", statusByNode);

    expect(nodes).toHaveLength(2);
    expect(nodes[0]).toMatchObject({
      id: "node-1",
      position: { x: 96, y: 96 },
      selected: false,
      data: { label: "Plan", status: "completed" },
      width: 320,
      height: 88,
    });
    expect(nodes[1]).toMatchObject({
      id: "node-2",
      selected: true,
      data: { label: "Draft", status: "awaiting_input" },
    });
  });

  test("buildFlowEdges preserves direction and edge selection", () => {
    const edges = buildFlowEdges(graph, "edge-1");

    expect(edges).toEqual([
      expect.objectContaining({
        id: "edge-1",
        source: "node-1",
        target: "node-2",
        selected: true,
        reconnectable: true,
        deletable: true,
      }),
    ]);
  });

  test("reconcileFlowNodes keeps local drag position while applying external state", () => {
    const current = buildFlowNodes(graph, null, statusByNode);
    current[0] = {
      ...current[0],
      position: { x: 640, y: 180 },
      dragging: true,
    };

    const incoming = buildFlowNodes(graph, "node-1", {
      ...statusByNode,
      "node-1": "started",
    });

    const reconciled = reconcileFlowNodes(current, incoming);

    expect(reconciled[0]).toMatchObject({
      id: "node-1",
      position: { x: 640, y: 180 },
      selected: true,
      data: { label: "Plan", status: "started" },
    });
  });

  test("forEachNodePositionChange ignores in-flight drag updates", () => {
    const onPositionChange = vi.fn();
    const changes: NodeChange<WorkflowCanvasNode>[] = [
      { id: "node-1", type: "dimensions", dimensions: { width: 320, height: 104 } },
      { id: "node-2", type: "position", position: { x: 640, y: 180 }, positionAbsolute: { x: 640, y: 180 }, dragging: true },
      { id: "node-2", type: "position", position: { x: 672, y: 224 }, positionAbsolute: { x: 672, y: 224 }, dragging: false },
    ];

    forEachNodePositionChange(changes, onPositionChange);

    expect(onPositionChange).toHaveBeenCalledTimes(1);
    expect(onPositionChange).toHaveBeenCalledWith("node-2", 672, 224);
  });

  test("forEachRemovedEdge only forwards removals", () => {
    const onDeleteEdge = vi.fn();
    const changes: EdgeChange<WorkflowCanvasEdge>[] = [
      { id: "edge-1", type: "select", selected: true },
      { id: "edge-2", type: "remove" },
    ];

    forEachRemovedEdge(changes, onDeleteEdge);

    expect(onDeleteEdge).toHaveBeenCalledTimes(1);
    expect(onDeleteEdge).toHaveBeenCalledWith("edge-2");
  });

  test("selectionIdsFromChange prefers the first selected node and edge", () => {
    expect(
      selectionIdsFromChange({
        nodes: [{ id: "node-2" } as WorkflowCanvasNode],
        edges: [{ id: "edge-1" } as WorkflowCanvasEdge],
      }),
    ).toEqual({
      selectedNodeId: "node-2",
      selectedEdgeId: "edge-1",
    });
  });

  test("isValidCanvasConnection rejects self loops", () => {
    expect(isValidCanvasConnection({ source: "node-1", target: "node-2" })).toBe(true);
    expect(isValidCanvasConnection({ source: "node-1", target: "node-1" })).toBe(false);
    expect(isValidCanvasConnection({ source: null, target: "node-2" })).toBe(false);
  });
});

describe("WorkflowCanvas component", () => {
  test("renders an add node panel button that triggers the callback", () => {
    const onAddNode = vi.fn();

    render(
      createElement(
        "div",
        { style: { width: "960px", height: "640px" } },
        createElement(WorkflowCanvas, {
          graph,
          selectedNodeId: null,
          selectedEdgeId: null,
          statusByNode,
          onSelectNode: vi.fn(),
          onSelectEdge: vi.fn(),
          onUpdateNodePosition: vi.fn(),
          onCreateEdge: vi.fn(),
          onReconnectEdge: vi.fn(),
          onDeleteEdge: vi.fn(),
          onAddNode,
        }),
      ),
    );

    fireEvent.click(screen.getByRole("button", { name: "Add node" }));

    expect(onAddNode).toHaveBeenCalledTimes(1);
  });
});