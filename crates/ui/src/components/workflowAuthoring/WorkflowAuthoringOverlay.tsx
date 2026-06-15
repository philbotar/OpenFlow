import { For, Show } from "solid-js";
import X from "lucide-solid/icons/x";
import type {
  WorkflowAuthoringMessage,
  WorkflowAuthoringValidation,
} from "../../lib/types";
import { Message } from "../conversation/Message";
import { AuthoringComposer } from "./AuthoringComposer";
import { AuthoringValidationBanner } from "./AuthoringValidationBanner";

export function WorkflowAuthoringOverlay(props: {
  open: boolean;
  busy: boolean;
  providerReady: boolean;
  messages: WorkflowAuthoringMessage[];
  validation: WorkflowAuthoringValidation | null;
  canApply: boolean;
  onClose: () => void;
  onSend: (message: string) => void;
  onApply: () => void;
}) {
  return (
    <Show when={props.open}>
      <div class="workflow-authoring-overlay" role="dialog" aria-modal="true">
        <header class="workflow-authoring-header">
          <div>
            <h2>Build workflow with AI</h2>
            <p class="workflow-authoring-subtitle">
              Describe your workflow in plain language. Iterate until validation passes.
            </p>
          </div>
          <button
            type="button"
            class="topbar-icon-button"
            aria-label="Close workflow authoring"
            onClick={props.onClose}
          >
            <X class="sidebar-icon" aria-hidden="true" />
          </button>
        </header>

        <div class="workflow-authoring-messages">
          <Show
            when={props.messages.length > 0}
            fallback={
              <div class="conversation-empty">
                <p class="conversation-empty-title">Start with a goal</p>
                <p class="conversation-empty-description">
                  Example: clarify an idea, run plan and risk in parallel, then write a brief.
                </p>
              </div>
            }
          >
            <For each={props.messages}>
              {(message) => (
                <Message
                  from={message.role === "assistant" ? "assistant" : "user"}
                  label={message.role === "assistant" ? "Assistant" : "You"}
                  content={message.content}
                />
              )}
            </For>
          </Show>
        </div>

        <AuthoringValidationBanner validation={props.validation} />

        <footer class="workflow-authoring-footer">
          <AuthoringComposer
            busy={props.busy}
            providerReady={props.providerReady}
            onSend={props.onSend}
          />
          <button
            type="button"
            class="primary-button workflow-authoring-apply"
            disabled={!props.canApply || props.busy}
            onClick={props.onApply}
          >
            Apply to editor
          </button>
        </footer>
      </div>
    </Show>
  );
}
