import { createEffect, createMemo, createSignal, For, Show } from "solid-js";
import { useAppContext } from "../../context/AppContext";
import type { NodeId, StructuredUserInput } from "../../lib/types";
import { Button } from "../Button";

export function StructuredAskCard(props: {
  nodeId: NodeId;
  request: StructuredUserInput;
}) {
  const ctx = useAppContext();
  const [selectedById, setSelectedById] = createSignal<Record<string, string[]>>({});
  const [questionIndex, setQuestionIndex] = createSignal(0);
  const [submitting, setSubmitting] = createSignal(false);
  let currentQuestionElement: HTMLFieldSetElement | undefined;
  let requestKey = "";

  createEffect(() => {
    const nextRequestKey = JSON.stringify(props.request);
    if (nextRequestKey === requestKey) {
      return;
    }
    requestKey = nextRequestKey;
    setSelectedById({});
    setQuestionIndex(0);
    setSubmitting(false);
  });

  const answersFor = (questionId: string) => selectedById()[questionId] ?? [];
  const orderedAnswersFor = (question: StructuredUserInput["questions"][number]) => {
    const selected = new Set(answersFor(question.id));
    return question.options
      .filter((option) => selected.has(option.label))
      .map((option) => option.label);
  };

  const complete = createMemo(
    () =>
      props.request.questions.length > 0 &&
      props.request.questions.every((question) => answersFor(question.id).length > 0),
  );
  const currentQuestion = createMemo(() => props.request.questions[questionIndex()]);
  const currentAnswered = createMemo(() => {
    const question = currentQuestion();
    return question ? answersFor(question.id).length > 0 : false;
  });
  const onLastQuestion = createMemo(
    () => questionIndex() === props.request.questions.length - 1,
  );
  const inputEnabled = () => ctx.readiness()?.ready ?? false;

  const selectSingle = (questionId: string, value: string) => {
    setSelectedById((current) => ({ ...current, [questionId]: [value] }));
  };

  const toggleMultiple = (questionId: string, value: string) => {
    setSelectedById((current) => {
      const selected = current[questionId] ?? [];
      const next = selected.includes(value)
        ? selected.filter((label) => label !== value)
        : [...selected, value];
      return { ...current, [questionId]: next };
    });
  };

  const showQuestion = (index: number) => {
    const lastIndex = Math.max(0, props.request.questions.length - 1);
    setQuestionIndex(Math.min(Math.max(index, 0), lastIndex));
    queueMicrotask(() => currentQuestionElement?.focus());
  };

  const submit = async (event: SubmitEvent) => {
    event.preventDefault();
    if (!inputEnabled() || !complete() || submitting()) {
      return;
    }
    const text = [
      "Structured answers:",
      ...props.request.questions.map((question) => {
        const answers = orderedAnswersFor(question);
        const answer =
          question.selectionMode === "multiple"
            ? JSON.stringify(answers)
            : (answers[0] ?? "");
        return `- ${question.id}: ${answer}`;
      }),
    ].join("\n");
    setSubmitting(true);
    await ctx.handleSubmitStructuredInput(props.nodeId, text);
    setSubmitting(false);
  };

  return (
    <form class="structured-ask-card" onSubmit={(event) => void submit(event)}>
      <Show when={props.request.questions.length > 1}>
        <div class="structured-ask-progress" aria-live="polite">
          Question {questionIndex() + 1} of {props.request.questions.length}
        </div>
      </Show>
      <For each={currentQuestion() ? [currentQuestion()!] : []}>
        {(question) => {
          const promptId = `structured-ask-${props.nodeId}-${question.id}`;
          const hintId = `${promptId}-hint`;
          return (
            <fieldset
              class="structured-ask-question"
              ref={currentQuestionElement}
              tabIndex={-1}
            >
              <legend class="structured-ask-header">{question.header}</legend>
              <p class="structured-ask-prompt" id={promptId}>
                {question.question}
              </p>
              <Show when={question.selectionMode === "multiple"}>
                <span class="structured-ask-selection-hint" id={hintId}>
                  Select one or more.
                </span>
              </Show>
              <ul
                class="structured-ask-options"
                aria-labelledby={promptId}
                aria-describedby={
                  question.selectionMode === "multiple" ? hintId : undefined
                }
              >
                <For each={question.options}>
                  {(option) => (
                    <li
                      class="structured-ask-option-row"
                      classList={{
                        selected: answersFor(question.id).includes(option.label),
                      }}
                    >
                      <label class="structured-ask-option">
                        <input
                          class="structured-ask-option-control"
                          type={question.selectionMode === "multiple" ? "checkbox" : "radio"}
                          name={`${props.nodeId}-${question.id}`}
                          value={option.label}
                          checked={answersFor(question.id).includes(option.label)}
                          disabled={!inputEnabled() || submitting()}
                          onChange={() =>
                            question.selectionMode === "multiple"
                              ? toggleMultiple(question.id, option.label)
                              : selectSingle(question.id, option.label)
                          }
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
        <div class="structured-ask-navigation">
          <Show when={props.request.questions.length > 1}>
            <Button
              variant="secondary"
              size="small"
              disabled={questionIndex() === 0 || submitting()}
              onClick={() => showQuestion(questionIndex() - 1)}
            >
              Back
            </Button>
          </Show>
          <Show
            when={onLastQuestion()}
            fallback={
              <Button
                variant="primary"
                size="small"
                disabled={!currentAnswered() || submitting()}
                onClick={() => showQuestion(questionIndex() + 1)}
              >
                Next
              </Button>
            }
          >
            <Button
              type="submit"
              variant="primary"
              size="small"
              disabled={!inputEnabled() || !complete() || submitting()}
            >
              {submitting() ? "Sending…" : "Submit answers"}
            </Button>
          </Show>
        </div>
      </div>
    </form>
  );
}
