import { Show } from "solid-js";
import Sparkles from "lucide-solid/icons/sparkles";
import { useAppContext } from "../context/AppContext";
import {
  AuthoringComposer,
  AuthoringDraftPreview,
  AuthoringMessages,
  Button,
  Conversation,
  ConversationContent,
  ConversationScrollButton,
  PanelEmptyState,
} from "@/components";

export function WorkflowAuthoringScreen() {
  const ctx = useAppContext();
  const showDraftPreview = () => {
    const draft = ctx.workflowAuthoringDraft();
    return Boolean(draft && draft.nodes.length > 0);
  };
  const updatingExistingWorkflow = () => {
    const draft = ctx.workflowAuthoringDraft();
    return Boolean(
      draft && ctx.workflows().some((workflow) => workflow.id === draft.id),
    );
  };

  return (
    <section class="workflow-authoring-screen">
      <div
        class="workflow-authoring-body"
        classList={{ "workflow-authoring-body--with-preview": showDraftPreview() }}
      >
        <Show when={showDraftPreview()}>
          <AuthoringDraftPreview
            draft={ctx.workflowAuthoringDraft()!}
            validation={ctx.workflowAuthoringValidation()}
            busy={ctx.workflowAuthoringBusy()}
            colorMode={ctx.resolvedTheme()}
            uiZoom={ctx.uiZoom()}
          />
        </Show>

        <div class="chat-layout workflow-authoring-chat">
          <div class="chat-settled">
            <Conversation class="chat-settled-conversation">
              {(conversation) => (
                <>
                  <ConversationContent conversation={conversation} class="chat-transcript-scroll">
                    <div class="chat-transcript-lane">
                      <Show
                        when={ctx.workflowAuthoringMessages().length > 0 || ctx.workflowAuthoringBusy()}
                        fallback={
                          <PanelEmptyState
                            icon={<Sparkles width={22} height={22} />}
                            title="Start with a goal"
                            description="Example: clarify an idea, run plan and risk in parallel, then write a brief."
                          />
                        }
                      >
                        <AuthoringMessages
                          messages={ctx.workflowAuthoringMessages()}
                          busy={ctx.workflowAuthoringBusy()}
                          thinkingContent={ctx.workflowAuthoringThinkingContent()}
                        />
                      </Show>
                    </div>
                  </ConversationContent>
                  <ConversationScrollButton conversation={conversation} />
                </>
              )}
            </Conversation>
          </div>

          <Show when={ctx.readiness()?.ready === false ? ctx.readiness() : undefined}>
            {(readiness) => (
              <p class="workflow-authoring-status workflow-authoring-status--warn" role="status">
                {readiness().message}
              </p>
            )}
          </Show>

          <div class="chat-composer-bar">
            <Show
              when={ctx.workflowAuthoringSessionReady()}
              fallback={
                <div class="chat-live-strip chat-live-strip--pending" aria-live="polite">
                  <p class="chat-live-starting">Starting authoring session…</p>
                </div>
              }
            >
              <div class="workflow-authoring-composer-row">
                <AuthoringComposer
                  busy={ctx.workflowAuthoringBusy()}
                  sessionReady={ctx.workflowAuthoringSessionReady()}
                  providerReady={ctx.readiness()?.ready === true}
                  providerMessage={ctx.readiness()?.message ?? "Checking provider..."}
                  onSend={(message) => void ctx.handleWorkflowAuthoringSend(message)}
                />
                <div class="workflow-authoring-apply-group">
                  <p>
                    {updatingExistingWorkflow()
                      ? "AI prepares a revised draft. Review it, then click Apply Changes."
                      : "AI creates a draft. Click Create Workflow to save it. Then click Run in the editor to start it."}
                  </p>
                  <Button
                    variant="primary"
                    class="workflow-authoring-apply"
                    disabled={
                      ctx.workflowAuthoringValidation()?.valid !== true ||
                      ctx.workflowAuthoringBusy()
                    }
                    onClick={() => void ctx.handleApplyWorkflowAuthoringDraft()}
                  >
                    {updatingExistingWorkflow() ? "Apply Changes" : "Create Workflow"}
                  </Button>
                </div>
              </div>
            </Show>
          </div>
        </div>
      </div>
    </section>
  );
}
