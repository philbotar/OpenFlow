import type { AgentStatus, ChatMessage, NodeId, Workflow, WorkflowRunState } from "../types";
import { executionLayers } from "./layout";
import { statusForNode } from "./runState";

const LIVE_AGENT_STATUSES: ReadonlySet<AgentStatus> = new Set([
  "queued",
  "started",
  "running_tool",
  "awaiting_input",
  "awaiting_tool_approval",
]);

export type TranscriptSegment = {
  nodeId: NodeId;
  label: string;
  messages: ChatMessage[];
  status: AgentStatus;
};

export type ChatLayoutProjection = {
  settled: TranscriptSegment[];
  live: TranscriptSegment[];
};

export type ChatNavigationTarget =
  | { mode: "live"; nodeId: NodeId }
  | { mode: "settled"; nodeId: NodeId };

/** Map a canvas node click to live-pick vs settled-filter chat navigation. */
export function chatNavigationForNode(
  layout: ChatLayoutProjection,
  nodeId: NodeId,
): ChatNavigationTarget | null {
  const liveIds = new Set(layout.live.map((segment) => segment.nodeId));
  if (liveIds.has(nodeId)) {
    return { mode: "live", nodeId };
  }
  if (layout.settled.some((segment) => segment.nodeId === nodeId)) {
    return { mode: "settled", nodeId };
  }
  return null;
}

/** True when chat filter/pick already shows this node's transcript. */
export function isChatNavigatedToNode(
  layout: ChatLayoutProjection,
  nodeId: NodeId,
  chatFilterNodeId: NodeId | null,
  pickedLiveNodeId: NodeId | null,
): boolean {
  const nav = chatNavigationForNode(layout, nodeId);
  if (nav?.mode === "live") {
    return pickedLiveNodeId === nodeId && chatFilterNodeId === null;
  }
  if (nav?.mode === "settled") {
    return chatFilterNodeId === nodeId && pickedLiveNodeId === null;
  }
  return chatFilterNodeId === null && pickedLiveNodeId === null;
}

function isLiveAgentStatus(runState: WorkflowRunState, status: AgentStatus): boolean {
  return runState.active === true && LIVE_AGENT_STATUSES.has(status);
}

/** True when a transcript segment is an actively running node (not yet folded into history). */
export function isLiveTranscriptSegment(
  runState: WorkflowRunState | null,
  segment: Pick<TranscriptSegment, "status">,
): boolean {
  if (!runState) {
    return false;
  }
  return isLiveAgentStatus(runState, segment.status);
}

/** First time each node appears in the run trace (fallback: DAG traversal). */
export function nodeRunAppearanceOrder(
  runState: WorkflowRunState,
  traversalOrder: NodeId[],
): NodeId[] {
  const ordered: NodeId[] = [];
  const seen = new Set<NodeId>();

  for (const entry of runState.runTrace) {
    if (!seen.has(entry.nodeId)) {
      seen.add(entry.nodeId);
      ordered.push(entry.nodeId);
    }
  }

  for (const nodeId of traversalOrder) {
    if (!seen.has(nodeId) && (runState.chatLogs[nodeId]?.length ?? 0) > 0) {
      seen.add(nodeId);
      ordered.push(nodeId);
    }
  }

  for (const nodeId of Object.keys(runState.chatLogs)) {
    if (!seen.has(nodeId) && (runState.chatLogs[nodeId]?.length ?? 0) > 0) {
      seen.add(nodeId);
      ordered.push(nodeId);
    }
  }

  return ordered;
}

export function sortTranscriptSegmentsByNodeOrder(
  segments: TranscriptSegment[],
  nodeOrder: NodeId[],
): TranscriptSegment[] {
  if (nodeOrder.length === 0) {
    return segments;
  }
  const rank = new Map(nodeOrder.map((nodeId, index) => [nodeId, index]));
  return [...segments].sort((left, right) => {
    const leftRank = rank.get(left.nodeId);
    const rightRank = rank.get(right.nodeId);
    if (leftRank !== undefined && rightRank !== undefined) {
      return leftRank - rightRank;
    }
    if (leftRank !== undefined) {
      return -1;
    }
    if (rightRank !== undefined) {
      return 1;
    }
    return 0;
  });
}

function appendSettledSegment(settled: TranscriptSegment[], segment: TranscriptSegment): void {
  if (settled.some((existing) => existing.nodeId === segment.nodeId)) {
    return;
  }
  settled.push(segment);
}

function effectiveAgentStatus(
  runState: WorkflowRunState,
  nodeId: NodeId,
): AgentStatus {
  const status = statusForNode(runState.statusByNode, nodeId);
  if (!runState.active) {
    return status;
  }
  const awaiting =
    runState.awaitingNodeIds?.includes(nodeId) || runState.awaitingNodeId === nodeId;
  if (awaiting && status === "idle") {
    return "awaiting_input";
  }
  return status;
}

function buildTranscriptSegment(
  nodeId: NodeId,
  label: string,
  messages: ChatMessage[],
  status: AgentStatus,
): TranscriptSegment {
  return { nodeId, label, messages, status };
}

export function projectChatLayout(
  workflow: Workflow | undefined,
  runState: WorkflowRunState | null,
  pickedLiveNodeId?: NodeId | null,
  segmentOrder?: NodeId[] | null,
): ChatLayoutProjection {
  if (!runState) {
    return { settled: [], live: [] };
  }

  const labels = new Map<NodeId, string>();
  const workflowNodeIds = new Set<NodeId>();
  if (workflow) {
    for (const node of workflow.nodes) {
      labels.set(node.id, node.label);
      workflowNodeIds.add(node.id);
    }
  }

  const layers = workflow ? executionLayers(workflow) : [];
  const orderedNodeIds = layers.flat();
  const deletedNodeIds = Object.keys(runState.chatLogs).filter((id) => !workflowNodeIds.has(id));
  const traversalOrder = [...orderedNodeIds, ...deletedNodeIds];
  const appearanceOrder = nodeRunAppearanceOrder(runState, traversalOrder);
  const displayOrder =
    segmentOrder && segmentOrder.length > 0
      ? [
          ...segmentOrder,
          ...appearanceOrder.filter((nodeId) => !segmentOrder.includes(nodeId)),
          ...traversalOrder.filter(
            (nodeId) =>
              !segmentOrder.includes(nodeId) && !appearanceOrder.includes(nodeId),
          ),
        ]
      : [
          ...appearanceOrder,
          ...traversalOrder.filter((nodeId) => !appearanceOrder.includes(nodeId)),
        ];

  const settled: TranscriptSegment[] = [];
  const live: TranscriptSegment[] = [];

  for (const nodeId of traversalOrder) {
    const messages = runState.chatLogs[nodeId] ?? [];
    const status = effectiveAgentStatus(runState, nodeId);
    const label = labels.get(nodeId) ?? nodeId;
    const segment = buildTranscriptSegment(nodeId, label, messages, status);

    if (isLiveAgentStatus(runState, status)) {
      live.push(segment);
      continue;
    }
    if (messages.length > 0) {
      settled.push(segment);
    }
  }

  // One running node streams into the main history. With parallel nodes the chat
  // blocks until the user picks one to talk to; the picked node streams inline and
  // the rest stay in `live` (rendered as a picker) until each completes in turn.
  if (live.length === 1) {
    appendSettledSegment(settled, live[0]);
    live.length = 0;
  } else if (live.length > 1 && pickedLiveNodeId) {
    const pickedIndex = live.findIndex((segment) => segment.nodeId === pickedLiveNodeId);
    if (pickedIndex !== -1) {
      const [picked] = live.splice(pickedIndex, 1);
      appendSettledSegment(settled, picked);
    }
  }

  return {
    settled: sortTranscriptSegmentsByNodeOrder(settled, displayOrder),
    live,
  };
}
