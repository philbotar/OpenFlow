// @vitest-environment jsdom
import type { EdgeChange, NodeChange } from "@xyflow/react";
import { fireEvent, render, screen, within } from "@testing-library/react";
import { createElement } from "react";
import { afterEach, describe, expect, test, vi } from "vitest";
import type { SubagentStatus, Workflow } from "../lib/types";
import { createEmptyToolConfig } from "../lib/workflow/testHelpers";
import {
  projectWorkflowCanvasGraph,
  type WorkflowCanvasStatusByNode,
  type WorkflowCanvasSubagentsByNode,
} from "../lib/workflow";
import {
  WorkflowCanvas,
  buildFlowEdges,
  buildFlowNodes,
  forEachNodePositionChange,
  forEachRemovedEdge,
  graphStructureSignature,
  isValidCanvasConnection,
  reconcileFlowNodes,
  withoutProgrammaticNodeChanges,
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
        callable_agents: [],
        allow_all_callable_agents: false,
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
        callable_agents: [],
        allow_all_callable_agents: false,
      },
    },
  ],
  edges: [{ id: "edge-1", from: "node-1", to: "node-2" }],
  settings: { shared_context: "" },
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
  test("buildFlowNodes preserves positions and leaves final dimensions to DOM measurement", () => {
    const nodes = buildFlowNodes(graph, "node-2", statusByNode, null);

    expect(nodes).toHaveLength(2);
    expect(nodes[0]).toMatchObject({
      id: "node-1",
      position: { x: 96, y: 96 },
      selected: false,
      deletable: true,
      data: { label: "Plan", status: "completed" },
      initialWidth: 320,
      initialHeight: 88,
    });
    expect(nodes[0]).not.toHaveProperty("width");
    expect(nodes[0]).not.toHaveProperty("height");
    expect(nodes[1]).toMatchObject({
      id: "node-2",
      selected: true,
      data: { label: "Draft", status: "awaiting_input" },
    });
  });

  test("buildFlowNodes includes subagents when provided", () => {
    const subagentsByNode: WorkflowCanvasSubagentsByNode = {
      "node-1": [
        { id: "n1-sub-1", name: "Researcher", purpose: "Investigate", status: "declared" as SubagentStatus },
        { id: "n1-sub-2", name: "Writer", purpose: "Summarize", status: "active" as SubagentStatus },
      ],
    };
    const nodes = buildFlowNodes(graph, null, statusByNode, subagentsByNode);
    expect(nodes[0].data.subagents).toEqual([
      { id: "n1-sub-1", name: "Researcher", purpose: "Investigate", status: "declared" as SubagentStatus },
      { id: "n1-sub-2", name: "Writer", purpose: "Summarize", status: "active" as SubagentStatus },
    ]);
    expect(nodes[1].data.subagents).toEqual([]);
  });

  test("buildFlowNodes with null subagentsByNode yields empty arrays", () => {
    const nodes = buildFlowNodes(graph, null, statusByNode, null);
    expect(nodes[0].data.subagents).toEqual([]);
    expect(nodes[1].data.subagents).toEqual([]);
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
        interactionWidth: 28,
        animated: false,
      }),
    ]);
  });

  test("buildFlowEdges animates edges while a run is active", () => {
    const edges = buildFlowEdges(graph, null, true, "dark");
    expect(edges[0].animated).toBe(true);
    expect(edges[0].style).toEqual({ stroke: "#55555c", strokeWidth: 2 });
  });

  test("graphStructureSignature ignores node positions", () => {
    const movedGraph = {
      ...graph,
      nodes: graph.nodes.map((node) =>
        node.id === "node-1"
          ? { ...node, position: { x: node.position.x + 160, y: node.position.y + 80 } }
          : node,
      ),
    };

    expect(graphStructureSignature(movedGraph)).toBe(graphStructureSignature(graph));
  });

  test("graphStructureSignature changes for structural graph edits", () => {
    const reconnectedGraph = {
      ...graph,
      edges: graph.edges.map((edge) =>
        edge.id === "edge-1" ? { ...edge, to: "node-1" } : edge,
      ),
    };
    const addedNodeGraph = {
      ...graph,
      nodes: [
        ...graph.nodes,
        { ...graph.nodes[0], id: "node-3", label: "Review" },
      ],
    };

    expect(graphStructureSignature(reconnectedGraph)).not.toBe(graphStructureSignature(graph));
    expect(graphStructureSignature(addedNodeGraph)).not.toBe(graphStructureSignature(graph));
  });

  test("reconcileFlowNodes keeps local drag position while applying external state", () => {
    const current = buildFlowNodes(graph, null, statusByNode, null);
    current[0] = {
      ...current[0],
      position: { x: 640, y: 180 },
      dragging: true,
    };

    const incoming = buildFlowNodes(graph, "node-1", {
      ...statusByNode,
      "node-1": "started",
    }, null);

    const reconciled = reconcileFlowNodes(current, incoming);

    expect(reconciled[0]).toMatchObject({
      id: "node-1",
      position: { x: 640, y: 180 },
      selected: true,
      data: { label: "Plan", status: "started" },
    });
  });

  test("reconcileFlowNodes returns the same array reference when nothing changed", () => {
    const current = buildFlowNodes(graph, "node-1", statusByNode, null);
    const incoming = buildFlowNodes(graph, "node-1", statusByNode, null);

    expect(reconcileFlowNodes(current, incoming)).toBe(current);
  });

  test("withoutProgrammaticNodeChanges drops select and remove changes", () => {
    const changes: NodeChange<WorkflowCanvasNode>[] = [
      { id: "node-1", type: "select", selected: true },
      { id: "node-2", type: "remove" },
      { id: "node-1", type: "position", position: { x: 128, y: 128 }, positionAbsolute: { x: 128, y: 128 }, dragging: false },
    ];

    expect(withoutProgrammaticNodeChanges(changes)).toEqual([
      { id: "node-1", type: "position", position: { x: 128, y: 128 }, positionAbsolute: { x: 128, y: 128 }, dragging: false },
    ]);
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

  test("isValidCanvasConnection rejects self loops", () => {
    expect(isValidCanvasConnection({ source: "node-1", target: "node-2" })).toBe(true);
    expect(isValidCanvasConnection({ source: "node-1", target: "node-1" })).toBe(false);
    expect(isValidCanvasConnection({ source: null, target: "node-2" })).toBe(false);
  });
});

describe("WorkflowCanvas component", () => {
  test("keeps the workflow in place when wheel scrolling over the canvas", async () => {
    render(
      createElement(
        "div",
        { style: { width: "960px", height: "640px" } },
        createElement(WorkflowCanvas, {
          graph,
          selectedNodeId: null,
          selectedEdgeId: null,
          statusByNode,
          subagentsByNode: null,
          onSelectNode: vi.fn(),
          onSelectEdge: vi.fn(),
          onUpdateNodePosition: vi.fn(),
          onAutoLayout: vi.fn(),
          onCreateEdge: vi.fn(),
          onReconnectEdge: vi.fn(),
          onDeleteEdge: vi.fn(),
          onDeleteNode: vi.fn(),
          onAddNode: vi.fn(),
        }),
      ),
    );

    const pane = document.querySelector(".react-flow__pane");
    const viewport = document.querySelector(".react-flow__viewport");
    expect(pane).not.toBeNull();
    expect(viewport).not.toBeNull();
    const initialTransform = viewport!.getAttribute("style");

    fireEvent.wheel(pane!, { deltaX: 0, deltaY: 120, clientX: 480, clientY: 320 });

    await vi.waitFor(() => {
      expect(viewport!.getAttribute("style")).toBe(initialTransform);
    });
  });

  test("offers a contextual delete action for the selected node", () => {
    const onDeleteNode = vi.fn();

    render(
      createElement(
        "div",
        { style: { width: "960px", height: "640px" } },
        createElement(WorkflowCanvas, {
          graph,
          selectedNodeId: "node-1",
          selectedEdgeId: null,
          statusByNode,
          subagentsByNode: null,
          onSelectNode: vi.fn(),
          onSelectEdge: vi.fn(),
          onUpdateNodePosition: vi.fn(),
          onAutoLayout: vi.fn(),
          onCreateEdge: vi.fn(),
          onReconnectEdge: vi.fn(),
          onDeleteEdge: vi.fn(),
          onDeleteNode,
          onAddNode: vi.fn(),
        }),
      ),
    );

    fireEvent.click(
      screen.getByRole("button", {
        name: "Delete Plan and 1 connection",
      }),
    );

    expect(onDeleteNode).toHaveBeenCalledWith("node-1");
  });

  test("offers a contextual delete action for the selected edge", () => {
    const onDeleteEdge = vi.fn();

    render(
      createElement(
        "div",
        { style: { width: "960px", height: "640px" } },
        createElement(WorkflowCanvas, {
          graph,
          selectedNodeId: null,
          selectedEdgeId: "edge-1",
          statusByNode,
          subagentsByNode: null,
          onSelectNode: vi.fn(),
          onSelectEdge: vi.fn(),
          onUpdateNodePosition: vi.fn(),
          onAutoLayout: vi.fn(),
          onCreateEdge: vi.fn(),
          onReconnectEdge: vi.fn(),
          onDeleteEdge,
          onDeleteNode: vi.fn(),
          onAddNode: vi.fn(),
        }),
      ),
    );

    fireEvent.click(screen.getByRole("button", { name: "Delete connection Plan → Draft" }));

    expect(onDeleteEdge).toHaveBeenCalledWith("edge-1");
  });

  test("locks structural editing while a run is active", () => {
    render(
      createElement(
        "div",
        { style: { width: "960px", height: "640px" } },
        createElement(WorkflowCanvas, {
          graph,
          selectedNodeId: "node-1",
          selectedEdgeId: null,
          statusByNode,
          subagentsByNode: null,
          runActive: true,
          onSelectNode: vi.fn(),
          onSelectEdge: vi.fn(),
          onUpdateNodePosition: vi.fn(),
          onAutoLayout: vi.fn(),
          onCreateEdge: vi.fn(),
          onReconnectEdge: vi.fn(),
          onDeleteEdge: vi.fn(),
          onDeleteNode: vi.fn(),
          onAddNode: vi.fn(),
        }),
      ),
    );

    expect(screen.getByText("Running · Editing locked")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Add node" }).hasAttribute("disabled")).toBe(true);
    expect(screen.getByRole("button", { name: "Auto layout" }).hasAttribute("disabled")).toBe(
      true,
    );
    expect(screen.getByRole("button", { name: /Delete Plan/ }).hasAttribute("disabled")).toBe(
      true,
    );
    expect(screen.getByRole("button", { name: "Zoom in" }).hasAttribute("disabled")).toBe(false);
    expect(screen.getByRole("button", { name: "Zoom out" }).hasAttribute("disabled")).toBe(
      false,
    );
    expect(screen.getByTestId("rf__node-node-1").classList.contains("draggable")).toBe(false);
  });

  test("deletes the selected edge with Backspace", async () => {
    const onDeleteEdge = vi.fn();

    render(
      createElement(
        "div",
        { style: { width: "960px", height: "640px" } },
        createElement(WorkflowCanvas, {
          graph,
          selectedNodeId: null,
          selectedEdgeId: "edge-1",
          statusByNode,
          subagentsByNode: null,
          onSelectNode: vi.fn(),
          onSelectEdge: vi.fn(),
          onUpdateNodePosition: vi.fn(),
          onAutoLayout: vi.fn(),
          onCreateEdge: vi.fn(),
          onReconnectEdge: vi.fn(),
          onDeleteEdge,
          onDeleteNode: vi.fn(),
          onAddNode: vi.fn(),
        }),
      ),
    );

    fireEvent.keyDown(document, { key: "Backspace", code: "Backspace" });

    await vi.waitFor(() => expect(onDeleteEdge).toHaveBeenCalledWith("edge-1"));
  });

  test("deletes the selected node with Delete", async () => {
    const onDeleteNode = vi.fn();
    const onDeleteEdge = vi.fn();

    render(
      createElement(
        "div",
        { style: { width: "960px", height: "640px" } },
        createElement(WorkflowCanvas, {
          graph,
          selectedNodeId: "node-1",
          selectedEdgeId: null,
          statusByNode,
          subagentsByNode: null,
          onSelectNode: vi.fn(),
          onSelectEdge: vi.fn(),
          onUpdateNodePosition: vi.fn(),
          onAutoLayout: vi.fn(),
          onCreateEdge: vi.fn(),
          onReconnectEdge: vi.fn(),
          onDeleteEdge,
          onDeleteNode,
          onAddNode: vi.fn(),
        }),
      ),
    );

    fireEvent.keyDown(document, { key: "Delete", code: "Delete" });

    await vi.waitFor(() => expect(onDeleteNode).toHaveBeenCalledWith("node-1"));
    expect(onDeleteEdge).not.toHaveBeenCalled();
  });

  test("ignores deletion shortcuts while a run is active", async () => {
    const onDeleteNode = vi.fn();
    const onDeleteEdge = vi.fn();

    render(
      createElement(
        "div",
        { style: { width: "960px", height: "640px" } },
        createElement(WorkflowCanvas, {
          graph,
          selectedNodeId: "node-1",
          selectedEdgeId: null,
          statusByNode,
          subagentsByNode: null,
          runActive: true,
          onSelectNode: vi.fn(),
          onSelectEdge: vi.fn(),
          onUpdateNodePosition: vi.fn(),
          onAutoLayout: vi.fn(),
          onCreateEdge: vi.fn(),
          onReconnectEdge: vi.fn(),
          onDeleteEdge,
          onDeleteNode,
          onAddNode: vi.fn(),
        }),
      ),
    );

    fireEvent.keyDown(document, { key: "Delete", code: "Delete" });
    await new Promise((resolve) => setTimeout(resolve, 0));

    expect(onDeleteNode).not.toHaveBeenCalled();
    expect(onDeleteEdge).not.toHaveBeenCalled();
  });

  test("stacks canvas actions with hover tooltips in one compact toolbar", async () => {
    const onAddNode = vi.fn();
    const onAutoLayout = vi.fn();

    render(
      createElement(
        "div",
        { style: { width: "960px", height: "640px" } },
        createElement(WorkflowCanvas, {
          graph,
          selectedNodeId: null,
          selectedEdgeId: null,
          statusByNode,
          subagentsByNode: null,
          onSelectNode: vi.fn(),
          onSelectEdge: vi.fn(),
          onUpdateNodePosition: vi.fn(),
          onAutoLayout,
          onCreateEdge: vi.fn(),
          onReconnectEdge: vi.fn(),
          onDeleteEdge: vi.fn(),
          onDeleteNode: vi.fn(),
          onAddNode,
        }),
      ),
    );

    const toolbar = screen.getByRole("toolbar", { name: "Workflow canvas tools" });
    const addNodeButton = within(toolbar).getByRole("button", { name: "Add node" });
    const autoLayoutButton = within(toolbar).getByRole("button", { name: "Auto layout" });
    const zoomInButton = within(toolbar).getByRole("button", { name: "Zoom in" });
    const zoomOutButton = within(toolbar).getByRole("button", { name: "Zoom out" });
    const deleteButton = within(toolbar).getByRole("button", { name: "Delete selected" });

    expect(addNodeButton.textContent).toBe("");
    expect(autoLayoutButton.textContent).toBe("");
    expect(zoomInButton.textContent).toBe("");
    expect(zoomOutButton.textContent).toBe("");
    expect(deleteButton.textContent).toBe("");
    expect(deleteButton.hasAttribute("disabled")).toBe(true);
    expect(
      within(toolbar)
        .getAllByRole("button")
        .map((button) => button.getAttribute("aria-label")),
    ).toEqual(["Add node", "Auto layout", "Zoom in", "Zoom out", "Delete selected"]);
    expect(
      within(toolbar)
        .getAllByRole("button")
        .every(
          (button) =>
            button.closest(".app-tooltip-trigger") !== null &&
            button.getAttribute("title") === null,
        ),
    ).toBe(true);

    const deleteTooltipTrigger = deleteButton.closest(".app-tooltip-trigger");
    expect(deleteTooltipTrigger).not.toBeNull();
    fireEvent.pointerEnter(deleteTooltipTrigger!);
    const tooltip = await screen.findByRole("tooltip", {}, { timeout: 1_000 });
    expect(tooltip.textContent).toBe("Delete selected");
    expect(tooltip.getAttribute("data-side")).toBe("right");
    fireEvent.pointerLeave(deleteTooltipTrigger!);
    expect(screen.queryByRole("tooltip")).toBeNull();

    fireEvent.click(addNodeButton);
    expect(onAddNode).toHaveBeenCalledTimes(1);

    fireEvent.click(autoLayoutButton);
    expect(onAutoLayout).toHaveBeenCalledTimes(1);
  });
});
