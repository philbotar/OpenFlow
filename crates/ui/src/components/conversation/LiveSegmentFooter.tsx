import { Show } from "solid-js";
import { useAppContext } from "../../context/AppContext";
import type { TranscriptSegment } from "../../lib/workflow";
import { ConversationComposer } from "./ConversationComposer";
import { StructuredAskCard } from "./StructuredAskCard";

export function LiveSegmentFooter(props: { segment: TranscriptSegment }) {
  const ctx = useAppContext();
  const structuredInput = () =>
    ctx.runState()?.structuredInputByNode?.[props.segment.nodeId] ?? null;

  return (
    <div class="chat-segment-footer">
      <Show when={structuredInput()}>
        {(request) => (
          <StructuredAskCard nodeId={props.segment.nodeId} request={request()} />
        )}
      </Show>
      <ConversationComposer nodeId={props.segment.nodeId} label={props.segment.label} />
    </div>
  );
}
