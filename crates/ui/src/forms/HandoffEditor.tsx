import { Show } from "solid-js";
import { Button, ButtonRow, TextSelect } from "@/components";
import type { HandoffSpec } from "@/lib/types";

export const DEFAULT_MARKDOWN_HANDOFF_TEMPLATE = `# Handoff

## Summary
<!-- Required: concise result -->

## Findings
<!-- Required: key findings, decisions, or completed work -->

## Files
<!-- Relevant files or artifacts -->

## Risks
<!-- Unknowns, blockers, or follow-up concerns -->

## Recommended Next Step
<!-- Concrete action for the downstream node -->
`;

export function HandoffEditor(props: {
  handoff?: HandoffSpec;
  schemaJson: string;
  onHandoffChange: (handoff: HandoffSpec) => void;
  onSchemaChange: (value: string) => void;
  onApplySchema?: () => boolean;
}) {
  const format = () => (props.handoff?.format === "markdown" ? "markdown" : "json");

  return (
    <>
      <label>
        <span>Format</span>
        <TextSelect
          value={format()}
          options={[
            { value: "markdown", label: "Markdown" },
            { value: "json", label: "JSON" },
          ]}
          onChange={(event) => {
            if (event.currentTarget.value === "markdown") {
              props.onHandoffChange({
                format: "markdown",
                template:
                  props.handoff?.format === "markdown"
                    ? props.handoff.template
                    : DEFAULT_MARKDOWN_HANDOFF_TEMPLATE,
              });
              return;
            }
            props.onHandoffChange({ format: "json" });
          }}
        />
      </label>
      <Show
        when={format() === "markdown"}
        fallback={
          <>
            <label>
              <span>JSON output schema</span>
              <textarea
                class="text-area code"
                rows={14}
                value={props.schemaJson}
                onInput={(event) => props.onSchemaChange(event.currentTarget.value)}
              />
            </label>
            <p class="field-help">
              The host validates this object, then stores it as <code>HANDOFF.json</code>.
            </p>
            <Show when={props.onApplySchema}>
              {(onApplySchema) => (
                <ButtonRow>
                  <Button variant="secondary" onClick={onApplySchema()}>
                    Apply schema
                  </Button>
                </ButtonRow>
              )}
            </Show>
          </>
        }
      >
        <label>
          <span>Markdown template</span>
          <textarea
            aria-label="Markdown handoff template"
            class="text-area code"
            rows={16}
            value={
              props.handoff?.format === "markdown"
                ? props.handoff.template
                : DEFAULT_MARKDOWN_HANDOFF_TEMPLATE
            }
            onInput={(event) =>
              props.onHandoffChange({
                format: "markdown",
                template: event.currentTarget.value,
              })
            }
          />
        </label>
        <p class="field-help">
          The node fills every heading. The host validates and stores <code>HANDOFF.md</code>.
        </p>
      </Show>
    </>
  );
}
