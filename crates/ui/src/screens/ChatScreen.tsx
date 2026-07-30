import { createMemo, Show } from "solid-js";
import {
  Conversation,
  ConversationComposer,
  ConversationContent,
  ConversationScrollButton,
  ConversationSegmentMessages,
  Button,
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
  const executionNodeId = createMemo(
    () =>
      Object.keys(ctx.runState()?.statusByNode ?? {})[0] ??
      Object.keys(ctx.runState()?.chatLogs ?? {})[0] ??
      null,
  );
  const transcriptNodeId = createMemo(
    () =>
      awaitingNodeId() ??
      executionNodeId() ??
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
  const failedNodeId = createMemo(() => {
    const state = ctx.runState();
    if (!state?.active) {
      return null;
    }
    return (
      Object.entries(state.statusByNode).find(
        ([, status]) => status === "failed",
      )?.[0] ?? null
    );
  });
  const generating = createMemo(
    () =>
      ctx.runState()?.active === true &&
      awaitingNodeId() === null &&
      failedNodeId() === null,
  );
  const contextWindow = createMemo(() => {
    const snapshots = ctx.runState()?.contextWindowByNode ?? {};
    const nodeId = executionNodeId();
    return (nodeId ? snapshots[nodeId] : undefined) ?? Object.values(snapshots)[0] ?? null;
  });

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
          <Show when={failedNodeId()}>
            {(nodeId) => (
              <div class="direct-chat-error" role="alert">
                <div>
                  <strong>Message failed</strong>
                  <span>
                    {ctx.runState()?.lastError ??
                      "The provider request failed. Retry when the provider is available."}
                  </span>
                </div>
                <Button
                  variant="secondary"
                  aria-label="Retry failed chat"
                  onClick={() => void ctx.handleRetryNode(nodeId())}
                >
                  Retry
                </Button>
              </div>
            )}
          </Show>
          <Show when={contextWindow()}>
            {(usage) => (
              <span
                class="direct-chat-token-usage"
                title={`Context usage for ${usage().model}`}
              >
                {formatTokenUsage(usage().usedTokens, usage().maxTokens)}
              </span>
            )}
          </Show>
          <ConversationComposer
            nodeId={
              ctx.runState()?.active === true
                ? awaitingNodeId() ??
                  executionNodeId() ??
                  GLOBAL_RUN_ENTRY_NODE_ID
                : GLOBAL_RUN_ENTRY_NODE_ID
            }
            label="chat"
            kickoff={ctx.runState()?.active !== true}
            disabled={pendingApproval() !== null}
            directChat
          />
        </div>
      </div>
    </section>
  );
}

function formatTokenUsage(usedTokens: number, maxTokens: number): string {
  const used = formatTokenCount(usedTokens);
  if (maxTokens <= 0) {
    return `${used} tokens`;
  }
  return `${used} / ${formatTokenCount(maxTokens)} tokens`;
}

function formatTokenCount(tokens: number): string {
  if (tokens < 1_000) {
    return String(tokens);
  }
  const thousands = tokens / 1_000;
  return `${Number.isInteger(thousands) ? thousands.toFixed(0) : thousands.toFixed(1)}k`;
}
