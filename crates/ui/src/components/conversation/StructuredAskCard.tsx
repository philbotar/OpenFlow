import { createEffect, createMemo, createSignal, For } from "solid-js";
import { useAppContext } from "../../context/AppContext";
import type { NodeId, StructuredUserInput } from "../../lib/types";
import { Button } from "../Button";

export function StructuredAskCard(props: {
  nodeId: NodeId;
  request: StructuredUserInput;
}) {
  const ctx = useAppContext();
  const [selectedById, setSelectedById] = createSignal<Record<string, string>>({});
  const [submitting, setSubmitting] = createSignal(false);
  let requestKey = "";

  createEffect(() => {
    const nextRequestKey = JSON.stringify(props.request);
    if (nextRequestKey === requestKey) {
      return;
    }
    requestKey = nextRequestKey;
    setSelectedById({});
    setSubmitting(false);
  });

  const answerFor = (questionId: string) =>
    selectedById()[questionId]?.trim() ?? "";

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
              <ul class="structured-ask-options" aria-labelledby={promptId}>
                <For each={question.options}>
                  {(option) => (
                    <li
                      class="structured-ask-option-row"
                      classList={{
                        selected: selectedById()[question.id] === option.label,
                      }}
                    >
                      <label class="structured-ask-option">
                        <input
                          class="structured-ask-option-control"
                          type="radio"
                          name={`${props.nodeId}-${question.id}`}
                          value={option.label}
                          checked={selectedById()[question.id] === option.label}
                          disabled={!inputEnabled() || submitting()}
                          onChange={() => select(question.id, option.label)}
                        />
                        <span class="structured-ask-option-copy">
                          <span class="structured-ask-option-label">{option.label}</span>
                          <span class="structured-ask-option-description">
                            {option.description}
                          </span>
                        </span>
                      </label>
                    </li>
                  )}
                </For>
              </ul>
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
