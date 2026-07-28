import { createMemo, Show } from "solid-js";
import {
  Conversation,
  ConversationComposer,
  ConversationContent,
  ConversationScrollButton,
  ConversationSegmentMessages,
  PanelEmptyState,
  StructuredAskCard,
  ToolApprovalCardBody,
} from "@/components";
import { useAppContext } from "@/context/AppContext";
import { GLOBAL_RUN_ENTRY_NODE_ID } from "@/lib/workflow";

export function ChatScreen() {
  const ctx = useAppContext();
  const awaitingNodeId = createMemo(() => {
    const state = ctx.runState();
    if (!state?.active) {
      return null;
    }
    return state.awaitingNodeIds?.[0] ?? state.awaitingNodeId ?? null;
  });
  const transcriptNodeId = createMemo(
    () =>
      awaitingNodeId() ??
      Object.keys(ctx.runState()?.chatLogs ?? {})[0] ??
      GLOBAL_RUN_ENTRY_NODE_ID,
  );
  const messages = createMemo(() =>
    Object.values(ctx.runState()?.chatLogs ?? {})
      .flat()
      .filter((message) => message.messageKind !== "node_completed"),
  );
  const pendingApproval = createMemo(
    () => ctx.runState()?.pendingApprovals[0] ?? null,
  );
  const structuredInput = createMemo(() => {
    const nodeId = awaitingNodeId();
    return nodeId
      ? ctx.runState()?.structuredInputByNode?.[nodeId] ?? null
      : null;
  });
  const generating = createMemo(
    () => ctx.runState()?.active === true && awaitingNodeId() === null,
  );

  return (
    <section class="chat-screen">
      <div class="direct-chat-layout">
        <Conversation class="direct-chat-conversation">
          {(conversation) => (
            <>
              <ConversationContent
                conversation={conversation}
                class="direct-chat-transcript"
              >
                <div class="direct-chat-transcript-lane">
                  <Show
                    when={messages().length > 0}
                    fallback={
                      <PanelEmptyState
                        class="direct-chat-empty"
                        title="What can I help with?"
                        description="Send a message to start a conversation."
                      />
                    }
                  >
                    <ConversationSegmentMessages
                      nodeId={transcriptNodeId()}
                      label="Assistant"
                      messages={messages()}
                    />
                  </Show>
                </div>
              </ConversationContent>
              <ConversationScrollButton conversation={conversation} />
            </>
          )}
        </Conversation>
        <div class="direct-chat-composer-bar">
          <Show when={pendingApproval()}>
            {(approval) => (
              <ToolApprovalCardBody
                approval={approval()}
                showNodeLabel={false}
                onApprove={(allow) =>
                  void ctx.handleToolApproval(approval().approvalId, allow)
                }
              />
            )}
          </Show>
          <Show when={structuredInput() && awaitingNodeId()}>
            <StructuredAskCard
              nodeId={awaitingNodeId()!}
              request={structuredInput()!}
            />
          </Show>
          <Show when={generating()}>
            <p class="direct-chat-generating" role="status" aria-live="polite">
              Thinking…
            </p>
          </Show>
          <ConversationComposer
            nodeId={awaitingNodeId() ?? GLOBAL_RUN_ENTRY_NODE_ID}
            label="chat"
            kickoff={awaitingNodeId() === null}
            disabled={generating() || pendingApproval() !== null}
            directChat
          />
        </div>
      </div>
    </section>
  );
}
