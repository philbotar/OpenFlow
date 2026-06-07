import { For, Show } from "solid-js";
import { useAppContext } from "../../context/AppContext";
import { chatRoleToMessageFrom, messageLabel } from "./chatRole";
import {
  Conversation,
  ConversationContent,
  ConversationEmptyState,
  ConversationScrollButton,
} from "./Conversation";
import { Message } from "./Message";

export function ConversationMessages() {
  const ctx = useAppContext();

  return (
    <Conversation>
      {(conversation) => (
        <>
          <ConversationContent conversation={conversation}>
            <Show
              when={ctx.chatMessages().length > 0}
              fallback={<ConversationEmptyState />}
            >
              <For each={ctx.chatMessages()}>
                {(message) => (
                  <Message
                    from={chatRoleToMessageFrom(message.role)}
                    label={messageLabel(message.role, ctx.currentNode()?.label)}
                  >
                    {message.content}
                  </Message>
                )}
              </For>
            </Show>
          </ConversationContent>
          <ConversationScrollButton conversation={conversation} />
        </>
      )}
    </Conversation>
  );
}
