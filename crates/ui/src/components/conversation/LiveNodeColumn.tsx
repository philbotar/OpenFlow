import { Show } from "solid-js";
import { labelForAgentStatus } from "../../lib/agentStatus";
import { pendingApprovalForNode } from "../../lib/workflow";
import type { TranscriptSegment } from "../../lib/workflow";
import { useAppContext } from "../../context/AppContext";
import {
  Conversation,
  ConversationContent,
  ConversationScrollButton,
} from "./Conversation";
import { ConversationComposer } from "./ConversationComposer";
import { ConversationSegmentMessages } from "./ConversationSegmentMessages";
import { ToolApprovalCardBody } from "./ToolApprovalCard";

export function LiveNodeColumn(props: { segment: TranscriptSegment }) {
  const ctx = useAppContext();
  const approval = () => pendingApprovalForNode(ctx.runState(), props.segment.nodeId);
  const awaitingInput = () =>
    ctx.runState()?.active === true &&
    (ctx.runState()?.awaitingNodeIds?.includes(props.segment.nodeId) ||
      ctx.runState()?.awaitingNodeId === props.segment.nodeId);
  const isBusy = () => ctx.composerBusyFor(props.segment.nodeId);

  return (
    <div class="chat-live-column" data-node-id={props.segment.nodeId}>
      <div class="chat-live-column-header">
        <span class="chat-live-column-label">{props.segment.label}</span>
        <span class={`chat-live-status-pill status-${props.segment.status}`}>
          {labelForAgentStatus(props.segment.status)}
        </span>
        <Show when={isBusy()}>
          <span class="chat-live-streaming-dot" aria-label="Streaming" />
        </Show>
      </div>
      <Conversation class="chat-live-conversation">
        {(conversation) => (
          <>
            <ConversationContent conversation={conversation}>
              <ConversationSegmentMessages
                nodeId={props.segment.nodeId}
                label={props.segment.label}
                messages={props.segment.messages}
              />
            </ConversationContent>
            <ConversationScrollButton conversation={conversation} />
          </>
        )}
      </Conversation>
      <div class="chat-live-column-footer">
        <Show when={approval()}>
          {(item) => (
            <ToolApprovalCardBody
              approval={item()}
              onApprove={(allow) => void ctx.handleToolApproval(item().approvalId, allow)}
            />
          )}
        </Show>
        <Show when={awaitingInput() && !approval()}>
          <ConversationComposer
            nodeId={props.segment.nodeId}
            label={props.segment.label}
          />
        </Show>
      </div>
    </div>
  );
}
