/** @jsxImportSource react */
/** @jsxRuntime automatic */
import { Handle, Position } from "@xyflow/react";
import { labelForAgentStatus } from "../lib/agentStatus";
import type { AgentStatus, SubagentSummary } from "../lib/types";

const MAX_VISIBLE_SUBAGENTS = 3;

export type WorkflowNodeData = {
  label: string;
  status: AgentStatus;
  subagents: SubagentSummary[];
};

function subagentStatusDotClass(status: SubagentSummary["status"]): string {
  switch (status) {
    case "declared":
      return "subagent-dot-declared";
    case "active":
      return "subagent-dot-active";
    case "completed":
      return "subagent-dot-completed";
    case "failed":
      return "subagent-dot-failed";
    default:
      return "subagent-dot-declared";
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
  const subagents = data.subagents ?? [];
  const visible = subagents.slice(0, MAX_VISIBLE_SUBAGENTS);
  const overflow = subagents.length - MAX_VISIBLE_SUBAGENTS;

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
          <span className="node-status-label">{labelForAgentStatus(status)}</span>
        </div>
        <strong>{data.label}</strong>
        {subagents.length > 0 && (
          <div className="node-subagent-list">
            {visible.map((sub) => (
              <div
                key={sub.id}
                className="node-subagent-row"
                title={`${sub.name}: ${sub.purpose}`}
              >
                <span className={`node-subagent-dot ${subagentStatusDotClass(sub.status)}`} />
                <span className="node-subagent-name">{sub.name}</span>
              </div>
            ))}
            {overflow > 0 && (
              <div className="node-subagent-overflow">+{overflow} more</div>
            )}
          </div>
        )}
      </div>
      <Handle
        type="source"
        position={Position.Right}
        className={`workflow-flow-handle status-${status}`}
      />
    </>
  );
}