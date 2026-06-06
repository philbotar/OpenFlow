/** @jsxImportSource react */
/** @jsxRuntime automatic */
import { Handle, Position } from "@xyflow/react";
import type { AgentStatus } from "../lib/types";

export type WorkflowNodeData = {
  label: string;
  status: AgentStatus;
};

function labelForStatus(status: AgentStatus): string {
  switch (status) {
    case "queued":
      return "Queued";
    case "started":
      return "Running";
    case "awaiting_input":
      return "Waiting";
    case "completed":
      return "Done";
    case "failed":
      return "Failed";
    default:
      return "Idle";
  }
}

export function WorkflowNode({
  id,
  data,
}: {
  id: string;
  data: WorkflowNodeData;
}) {
  const status = data.status;

  return (
    <>
      <Handle
        type="target"
        position={Position.Left}
        className={`workflow-flow-handle status-${status}`}
      />
      <div className={`workflow-flow-node workflow-flow-node-${status}`}>
        <div className="node-status-row">
          <span className={`node-dot status-${status}`} />
          <span className="node-status-label">{labelForStatus(status)}</span>
        </div>
        <strong>{data.label}</strong>
      </div>
      <Handle
        type="source"
        position={Position.Right}
        className={`workflow-flow-handle status-${status}`}
      />
    </>
  );
}
