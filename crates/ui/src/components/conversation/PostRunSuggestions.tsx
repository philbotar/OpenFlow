import Lightbulb from "lucide-solid/icons/lightbulb";
import { For, Show } from "solid-js";
import type { Node, RunReport } from "../../lib/types";

interface PostRunSuggestionsProps {
  report: RunReport;
  nodes: Node[];
}

const categoryLabels = {
  prompt: "Prompt",
  tools: "Tools",
  workflow: "Workflow",
  model: "Model",
  coordination: "Coordination",
} as const;

export function PostRunSuggestions(props: PostRunSuggestionsProps) {
  const suggestions = () => props.report.suggestions ?? [];
  const targetLabel = (nodeId: string | null) =>
    nodeId ? props.nodes.find((node) => node.id === nodeId)?.label ?? nodeId : null;

  return (
    <section class="post-run-suggestions" aria-labelledby="post-run-suggestions-title">
      <header class="post-run-suggestions-header">
        <span class="post-run-suggestions-icon" aria-hidden="true">
          <Lightbulb width={16} height={16} />
        </span>
        <div>
          <span class="eyebrow">Run review</span>
          <h3 id="post-run-suggestions-title">Suggestions</h3>
        </div>
      </header>

      <Show
        when={!props.report.suggestions_error}
        fallback={
          <p class="post-run-suggestions-empty">
            Suggestions unavailable. {props.report.suggestions_error}
          </p>
        }
      >
        <Show
          when={suggestions().length > 0}
          fallback={
            <p class="post-run-suggestions-empty">
              No evidence-backed improvements identified.
            </p>
          }
        >
          <ol class="post-run-suggestions-list">
            <For each={suggestions()}>
              {(suggestion) => (
                <li class="post-run-suggestion">
                  <div class="post-run-suggestion-meta">
                    <span>{categoryLabels[suggestion.category]}</span>
                    <Show when={targetLabel(suggestion.targetNodeId)}>
                      {(label) => <span>{label()}</span>}
                    </Show>
                  </div>
                  <h4>{suggestion.title}</h4>
                  <p class="post-run-suggestion-evidence">{suggestion.evidence}</p>
                  <p>{suggestion.recommendation}</p>
                </li>
              )}
            </For>
          </ol>
        </Show>
      </Show>
    </section>
  );
}
