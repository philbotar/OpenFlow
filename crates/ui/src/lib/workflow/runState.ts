import type {
  AgentStatus,
  EditBatch,
  FileChangeRecord,
  NodeId,
  Workflow,
  WorkflowRunState,
} from "../types";

/** Backend historically emitted camelCase multi-word statuses over IPC. */
const AGENT_STATUS_ALIASES: Record<string, AgentStatus> = {
  awaitingInput: "awaiting_input",
  awaitingToolApproval: "awaiting_tool_approval",
  runningTool: "running_tool",
};

function normalizeAgentStatus(status: string): AgentStatus {
  return AGENT_STATUS_ALIASES[status] ?? (status as AgentStatus);
}

export function normalizeRunState(state: WorkflowRunState): WorkflowRunState {
  const statusByNode = Object.fromEntries(
    Object.entries(state.statusByNode).map(([nodeId, status]) => [
      nodeId,
      normalizeAgentStatus(String(status)),
    ]),
  ) as Record<NodeId, AgentStatus>;
  return { ...state, statusByNode };
}

/** Match persisted backend run state to the workflow whose nodes it describes. */
export function inferRunStateWorkflowId(
  runState: WorkflowRunState | null | undefined,
  workflows: Workflow[],
): string | null {
  if (!runState) {
    return null;
  }
  const nodeIds = new Set([
    ...Object.keys(runState.statusByNode),
    ...Object.keys(runState.chatLogs),
  ]);
  if (nodeIds.size === 0) {
    return null;
  }

  for (const workflow of workflows) {
    const workflowNodeIds = workflow.nodes.map((node) => node.id);
    if (workflowNodeIds.length === 0) {
      continue;
    }
    if (workflowNodeIds.every((id) => nodeIds.has(id))) {
      return workflow.id;
    }
  }
  return null;
}

export function createIdleRunState(workflow: Workflow): WorkflowRunState {
  const statusByNode = workflow.nodes.reduce<Record<NodeId, AgentStatus>>((acc, node) => {
    acc[node.id] = "idle";
    return acc;
  }, {});
  const chatLogs = workflow.nodes.reduce<Record<NodeId, []>>((acc, node) => {
    acc[node.id] = [];
    return acc;
  }, {});
  return {
    active: false,
    awaitingNodeId: null,
    awaitingNodeIds: [],
    activeManualNodeId: null,
    activeToolCallId: null,
    pendingApprovals: [],
    toolCallsByNode: {},
    toolArtifacts: {},
    execApprovalGranted: false,
    statusByNode,
    subagentsByNode: {},
    lastReport: null,
    lastError: null,
    chatLogs,
    runTrace: [],
    outputs: {},
    changedFiles: [],
    changedFilesByNode: {},
    editBatches: [],
  };
}

export function nodeChangedFiles(
  runState: WorkflowRunState | null,
  nodeId: NodeId | null,
): FileChangeRecord[] {
  if (!runState || !nodeId) {
    return [];
  }
  return runState.changedFilesByNode[nodeId] ?? [];
}

export function effectiveChangePath(record: FileChangeRecord): string {
  if (record.op === "rename" && record.renameTo) {
    return record.renameTo;
  }
  return record.path;
}

export function latestChangesByPath(records: FileChangeRecord[]): FileChangeRecord[] {
  const byPath = new Map<string, FileChangeRecord>();
  for (const record of records) {
    if (record.op === "rename") {
      const stale = byPath.get(record.path);
      if (!stale || record.timestampMs >= stale.timestampMs) {
        byPath.delete(record.path);
      }
    }
    const key = effectiveChangePath(record);
    const existing = byPath.get(key);
    if (!existing || record.timestampMs >= existing.timestampMs) {
      byPath.set(key, record);
    }
  }
  return [...byPath.values()];
}

export function runChangedFilePaths(runState: WorkflowRunState | null): string[] {
  if (!runState) return [];
  return latestChangesByPath(runState.changedFiles).map(effectiveChangePath);
}

export function nodeEditBatches(
  runState: WorkflowRunState | null,
  nodeId: NodeId | null,
): EditBatch[] {
  if (!runState || !nodeId) {
    return [];
  }
  return runState.editBatches.filter((batch) => batch.nodeId === nodeId);
}

export function statusForNode(
  statusByNode: Readonly<Record<NodeId, AgentStatus>> | null,
  nodeId: NodeId,
): AgentStatus {
  const status = statusByNode?.[nodeId];
  return status ? normalizeAgentStatus(status) : "idle";
}

export function nodeOutput(
  runState: WorkflowRunState | null,
  nodeId: NodeId | null,
): unknown | null {
  if (!nodeId) {
    return null;
  }
  return runState?.outputs[nodeId] ?? null;
}

function isNodeAwaitingInput(
  runState: WorkflowRunState | null,
  nodeId: NodeId | null,
): boolean {
  if (!runState || !nodeId) {
    return false;
  }
  if (runState.awaitingNodeIds?.includes(nodeId)) {
    return true;
  }
  return runState.awaitingNodeId === nodeId;
}

export function pendingApprovalForNode(
  runState: WorkflowRunState | null,
  nodeId: NodeId | null,
) {
  if (!runState?.pendingApprovals || !nodeId) {
    return undefined;
  }
  return runState.pendingApprovals.find((approval) => approval.nodeId === nodeId);
}

export const GLOBAL_RUN_ENTRY_NODE_ID = "__run_entry__" as const;

export function isGlobalRunEntryNodeId(nodeId: NodeId): boolean {
  return nodeId === GLOBAL_RUN_ENTRY_NODE_ID;
}

export function canSendIdleRunKickoff(
  runState: WorkflowRunState | null,
  readinessReady: boolean,
  hasActiveWorkflow: boolean,
  startingRun: boolean,
  text: string,
): boolean {
  return (
    runState?.active !== true &&
    hasActiveWorkflow &&
    !startingRun &&
    readinessReady &&
    text.trim() !== ""
  );
}

export function canSendChat(
  runState: WorkflowRunState | null,
  selectedNodeId: NodeId | null,
  readinessReady: boolean,
  text: string,
): boolean {
  return (
    runState?.active === true &&
    isNodeAwaitingInput(runState, selectedNodeId) &&
    !pendingApprovalForNode(runState, selectedNodeId) &&
    readinessReady &&
    text.trim() !== ""
  );
}

export function isChatComposerBusy(
  runState: WorkflowRunState | null,
  selectedNodeId: NodeId | null,
): boolean {
  if (!runState || !selectedNodeId) {
    return false;
  }

  const status = runState.statusByNode[selectedNodeId];
  return status === "started" || status === "running_tool";
}
