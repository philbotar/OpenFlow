import { For, Show } from "solid-js";
import type { ChatMessage, WorkflowAuthoringMessage } from "../../lib/types";
import { Message } from "../conversation/Message";
import { ThinkingBubble } from "../conversation/ThinkingBubble";

function AuthoringMessageItem(props: { message: WorkflowAuthoringMessage }) {
  if (props.message.role === "thinking") {
    return (
      <ThinkingBubble
        message={{ role: "thinking", content: props.message.content } satisfies ChatMessage}
      />
    );
  }
  return (
    <Message
      from={props.message.role === "assistant" ? "assistant" : "user"}
      label={props.message.role === "assistant" ? "Assistant" : "You"}
      content={props.message.content}
    />
  );
}

export function AuthoringMessages(props: {
  messages: WorkflowAuthoringMessage[];
  busy: boolean;
}) {
  return (
    <>
      <For each={props.messages}>
        {(message) => <AuthoringMessageItem message={message} />}
      </For>
      <Show when={props.busy}>
        <ThinkingBubble
          message={{ role: "thinking", content: "", streaming: true } satisfies ChatMessage}
        />
      </Show>
    </>
  );
}
