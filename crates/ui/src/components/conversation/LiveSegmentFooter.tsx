import { Show } from "solid-js";
import { pendingApprovalForNode } from "../../lib/workflow";
import type { TranscriptSegment } from "../../lib/workflow";
import { useAppContext } from "../../context/AppContext";
import { ConversationComposer } from "./ConversationComposer";
import { ToolApprovalCardBody } from "./ToolApprovalCard";

export function LiveSegmentFooter(props: { segment: TranscriptSegment }) {
  const ctx = useAppContext();
  const approval = () => pendingApprovalForNode(ctx.runState(), props.segment.nodeId);
  return (
    <div class="chat-segment-footer">
      <Show when={approval()}>
        {(item) => (
          <ToolApprovalCardBody
            approval={item()}
            onApprove={(allow) => void ctx.handleToolApproval(item().approvalId, allow)}
          />
        )}
      </Show>
      <ConversationComposer nodeId={props.segment.nodeId} label={props.segment.label} />
    </div>
  );
}
