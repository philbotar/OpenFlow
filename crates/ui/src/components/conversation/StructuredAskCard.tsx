import { createEffect, createMemo, createSignal, For, Show } from "solid-js";
import { useAppContext } from "../../context/AppContext";
import type { NodeId, StructuredUserInput } from "../../lib/types";
import { Button } from "../Button";

const OTHER_VALUE = "__openflow_other__";

export function StructuredAskCard(props: {
  nodeId: NodeId;
  request: StructuredUserInput;
}) {
  const ctx = useAppContext();
  const [selectedById, setSelectedById] = createSignal<Record<string, string>>({});
  const [customById, setCustomById] = createSignal<Record<string, string>>({});
  const [submitting, setSubmitting] = createSignal(false);
  let requestKey = "";

  createEffect(() => {
    const nextRequestKey = JSON.stringify(props.request);
    if (nextRequestKey === requestKey) {
      return;
    }
    requestKey = nextRequestKey;
    setSelectedById({});
    setCustomById({});
    setSubmitting(false);
  });

  const answerFor = (questionId: string) => {
    const selected = selectedById()[questionId];
    if (selected === OTHER_VALUE) {
      return customById()[questionId]?.trim() ?? "";
    }
    return selected?.trim() ?? "";
  };

  const complete = createMemo(
    () =>
      props.request.questions.length > 0 &&
      props.request.questions.every((question) => answerFor(question.id).length > 0),
  );
  const inputEnabled = () => ctx.readiness()?.ready ?? false;

  const select = (questionId: string, value: string) => {
    setSelectedById((current) => ({ ...current, [questionId]: value }));
  };

  const submit = async (event: SubmitEvent) => {
    event.preventDefault();
    if (!inputEnabled() || !complete() || submitting()) {
      return;
    }
    const text = [
      "Structured answers:",
      ...props.request.questions.map(
        (question) => `- ${question.id}: ${answerFor(question.id)}`,
      ),
    ].join("\n");
    setSubmitting(true);
    await ctx.handleSubmitStructuredInput(props.nodeId, text);
    setSubmitting(false);
  };

  return (
    <form class="structured-ask-card" onSubmit={(event) => void submit(event)}>
      <For each={props.request.questions}>
        {(question) => {
          const promptId = `structured-ask-${props.nodeId}-${question.id}`;
          return (
            <fieldset class="structured-ask-question">
              <legend class="structured-ask-header">{question.header}</legend>
              <p class="structured-ask-prompt" id={promptId}>
                {question.question}
              </p>
              <div
                class="structured-ask-options"
                role="radiogroup"
                aria-labelledby={promptId}
              >
                <For each={question.options}>
                  {(option) => (
                    <button
                      type="button"
                      class="structured-ask-option"
                      classList={{
                        selected: selectedById()[question.id] === option.label,
                      }}
                      role="radio"
                      aria-checked={selectedById()[question.id] === option.label}
                      disabled={!inputEnabled() || submitting()}
                      onClick={() => select(question.id, option.label)}
                    >
                      <span class="structured-ask-option-label">{option.label}</span>
                      <span class="structured-ask-option-description">
                        {option.description}
                      </span>
                    </button>
                  )}
                </For>
                <button
                  type="button"
                  class="structured-ask-option"
                  classList={{
                    selected: selectedById()[question.id] === OTHER_VALUE,
                  }}
                  role="radio"
                  aria-checked={selectedById()[question.id] === OTHER_VALUE}
                  disabled={!inputEnabled() || submitting()}
                  onClick={() => select(question.id, OTHER_VALUE)}
                >
                  <span class="structured-ask-option-label">Other</span>
                  <span class="structured-ask-option-description">
                    Enter a different answer.
                  </span>
                </button>
              </div>
              <Show when={selectedById()[question.id] === OTHER_VALUE}>
                <input
                  class="structured-ask-custom-input"
                  type="text"
                  value={customById()[question.id] ?? ""}
                  aria-label={`Other answer for ${question.header}`}
                  placeholder="Type your answer"
                  disabled={!inputEnabled() || submitting()}
                  onInput={(event) =>
                    setCustomById((current) => ({
                      ...current,
                      [question.id]: event.currentTarget.value,
                    }))
                  }
                />
              </Show>
            </fieldset>
          );
        }}
      </For>
      <div class="structured-ask-actions">
        <span>Or reply in your own words below.</span>
        <Button
          type="submit"
          variant="primary"
          size="small"
          disabled={!inputEnabled() || !complete() || submitting()}
        >
          {submitting() ? "Sending…" : "Submit answers"}
        </Button>
      </div>
    </form>
  );
}
