export interface NodeCompletedBubbleProps {
  summary: string;
}

export function NodeCompletedBubble(props: NodeCompletedBubbleProps) {
  return (
    <div class="node-completed-row" role="status" aria-live="polite">
      <div class="node-completed-bubble">
        <div class="node-completed-header">
          <span class="node-completed-icon" aria-hidden="true">
            ✓
          </span>
          <span>Node completed</span>
        </div>
        <div class="node-completed-summary">{props.summary}</div>
      </div>
    </div>
  );
}
