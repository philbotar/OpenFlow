import { createSignal, Show } from "solid-js";
import ArrowUp from "lucide-solid/icons/arrow-up";
import { Spinner } from "../Spinner";

export function AuthoringComposer(props: {
  busy: boolean;
  providerReady: boolean;
  onSend: (message: string) => void;
}) {
  const [draft, setDraft] = createSignal("");

  const canSend = () =>
    props.providerReady && !props.busy && draft().trim().length > 0;

  const handleSend = () => {
    const message = draft().trim();
    if (!canSend() || !message) return;
    props.onSend(message);
    setDraft("");
  };

  return (
    <div class="workflow-authoring-composer">
      <div
        class="chat-composer-pill workflow-authoring-composer-pill"
        classList={{ "is-busy": props.busy }}
      >
        <textarea
          class="text-area composer-input"
          rows={2}
          value={draft()}
          placeholder={
            props.providerReady
              ? "Describe the workflow you want to build..."
              : "Configure a provider in Settings first."
          }
          disabled={!props.providerReady || props.busy}
          onInput={(event) => setDraft(event.currentTarget.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter" && !event.shiftKey) {
              event.preventDefault();
              handleSend();
            }
          }}
        />
        <button
          type="button"
          class="composer-send-button"
          aria-label="Send message"
          disabled={!canSend()}
          onClick={handleSend}
        >
          <Show when={props.busy} fallback={<ArrowUp class="composer-send-icon" />}>
            <Spinner size="sm" />
          </Show>
        </button>
      </div>
    </div>
  );
}
