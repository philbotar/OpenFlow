import type { TranscriptSegment } from "../../lib/workflow";
import { ConversationComposer } from "./ConversationComposer";

export function LiveSegmentFooter(props: { segment: TranscriptSegment }) {
  return (
    <div class="chat-segment-footer">
      <ConversationComposer nodeId={props.segment.nodeId} label={props.segment.label} />
    </div>
  );
}
