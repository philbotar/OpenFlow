import type { Node, NodeId, Workflow } from "../types";
import { cloneWorkflow } from "./clone";

export * from "./canvas";
export * from "./chatLayout";
export * from "./clone";
export * from "./layout";
export * from "./reasoning";
export * from "./runState";

export function selectedNode(
  workflow: Workflow | undefined,
  selectedNodeId: NodeId | null,
): Node | undefined {
  return workflow?.nodes.find((node) => node.id === selectedNodeId);
}

export function replaceWorkflow(
  workflows: Workflow[],
  nextWorkflow: Workflow,
): Workflow[] {
  const next = workflows.map((workflow) =>
    workflow.id === nextWorkflow.id ? nextWorkflow : workflow,
  );
  return next.some((workflow) => workflow.id === nextWorkflow.id)
    ? next
    : [...next, nextWorkflow];
}

export function removeSelectedNode(
  workflow: Workflow,
  selectedNodeId: NodeId | null,
): Workflow {
  if (!selectedNodeId) {
    return workflow;
  }
  const next = cloneWorkflow(workflow);
  next.nodes = next.nodes.filter((node) => node.id !== selectedNodeId);
  next.edges = next.edges.filter(
    (edge) => edge.from !== selectedNodeId && edge.to !== selectedNodeId,
  );
  return next;
}
