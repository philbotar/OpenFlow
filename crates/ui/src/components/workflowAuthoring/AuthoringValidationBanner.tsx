import { For, Show } from "solid-js";
import type { WorkflowAuthoringValidation } from "../../lib/types";

export function AuthoringValidationBanner(props: {
  validation: WorkflowAuthoringValidation | null;
}) {
  return (
    <Show when={props.validation}>
      {(validation) => (
        <div
          class="workflow-authoring-validation"
          classList={{
            "workflow-authoring-validation--valid": validation().valid,
            "workflow-authoring-validation--invalid": !validation().valid,
          }}
          role="status"
        >
          <Show
            when={validation().valid}
            fallback={
              <div class="workflow-authoring-validation-errors">
                <p class="workflow-authoring-validation-title">Fix these issues:</p>
                <ul>
                  <For each={validation().errors}>
                    {(error) => <li>{error}</li>}
                  </For>
                </ul>
              </div>
            }
          >
            <p class="workflow-authoring-validation-title">
              Valid workflow
              <Show when={validation().dag}>
                {(dag) => (
                  <span class="workflow-authoring-validation-meta">
                    {" "}
                    · {dag().layerCount} layer{dag().layerCount === 1 ? "" : "s"}
                  </span>
                )}
              </Show>
            </p>
          </Show>
          <Show when={validation().warnings.length > 0}>
            <ul class="workflow-authoring-validation-warnings">
              <For each={validation().warnings}>
                {(warning) => <li>{warning}</li>}
              </For>
            </ul>
          </Show>
        </div>
      )}
    </Show>
  );
}
