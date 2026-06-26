import { For, Show } from "solid-js";
import Sparkles from "lucide-solid/icons/sparkles";
import { useAppContext } from "../context/AppContext";
import {
  AuthoringComposer,
  AuthoringDraftPreview,
  AuthoringValidationBanner,
  Message,
  PanelEmptyState,
  ThinkingBubble,
} from "@/components";

export function WorkflowAuthoringScreen() {
  const ctx = useAppContext();
  const showDraftPreview = () => {
    const draft = ctx.workflowAuthoringDraft();
    return Boolean(draft && draft.nodes.length > 0);
  };

  return (
    <section class="workflow-authoring-screen">
      <p class="workflow-authoring-intro">
        Describe your workflow in natural language. Iterate until validation passes, then apply
        to the editor.
      </p>

      <div
        class="workflow-authoring-body"
        classList={{ "workflow-authoring-body--with-preview": showDraftPreview() }}
      >
        <div class="workflow-authoring-chat">
          <div class="workflow-authoring-messages">
            <Show
              when={ctx.workflowAuthoringMessages().length > 0}
              fallback={
                <PanelEmptyState
                  icon={<Sparkles width={22} height={22} />}
                  title="Start with a goal"
                  description="Example: clarify an idea, run plan and risk in parallel, then write a brief."
                />
              }
            >
              <For each={ctx.workflowAuthoringMessages()}>
                {(message) => (
                  <Show
                    when={message.role.toLowerCase() === "thinking"}
                    fallback={
                      <Message
                        from={message.role === "assistant" ? "assistant" : "user"}
                        label={message.role === "assistant" ? "Assistant" : "You"}
                        content={message.content}
                      />
                    }
                  >
                    <ThinkingBubble message={{ role: "thinking", content: message.content }} />
                  </Show>
                )}
              </For>
            </Show>
          </div>

          <AuthoringValidationBanner validation={ctx.workflowAuthoringValidation()} />

          <Show when={ctx.readiness()?.ready === false ? ctx.readiness() : undefined}>
            {(readiness) => (
              <p class="workflow-authoring-status workflow-authoring-status--warn" role="status">
                {readiness().message}
              </p>
            )}
          </Show>

          <footer class="workflow-authoring-footer">
            <div class="workflow-authoring-composer-container">
              <AuthoringComposer
                busy={ctx.workflowAuthoringBusy()}
                sessionReady={ctx.workflowAuthoringSessionReady()}
                providerReady={ctx.readiness()?.ready === true}
                providerMessage={ctx.readiness()?.message ?? "Checking provider..."}
                onSend={(message) => void ctx.handleWorkflowAuthoringSend(message)}
              />
              <div class="workflow-authoring-actions">
                <button
                  type="button"
                  class="primary-button workflow-authoring-apply"
                  disabled={
                    ctx.workflowAuthoringValidation()?.valid !== true || ctx.workflowAuthoringBusy()
                  }
                  onClick={() => void ctx.handleApplyWorkflowAuthoringDraft()}
                >
                  Create Workflow
                </button>
              </div>
            </div>
          </footer>
        </div>

        <Show when={showDraftPreview()}>
          <AuthoringDraftPreview
            draft={ctx.workflowAuthoringDraft()!}
            validation={ctx.workflowAuthoringValidation()}
            busy={ctx.workflowAuthoringBusy()}
            colorMode={ctx.resolvedTheme()}
          />
        </Show>
      </div>
    </section>
  );
}
