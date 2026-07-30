import { Show, createMemo } from "solid-js";
import type { Node } from "../../lib/types";
import { InspectorSection } from "../InspectorSection";

function TextValue(props: { label: string; value: string }) {
  return (
    <div class="workflow-authoring-inspector-field">
      <span>{props.label}</span>
      <pre class="workflow-authoring-inspector-text">{props.value || "(empty)"}</pre>
    </div>
  );
}

export function AuthoringDraftInspector(props: { node: Node }) {
  const handoffFormat = () => props.node.agent.handoff?.format ?? "json";
  const outputSchema = createMemo(() =>
    JSON.stringify(props.node.agent.output_schema, null, 2),
  );

  return (
    <section
      class="workflow-authoring-inspector inspector-panel"
      aria-label="Proposed workflow inspector"
    >
      <div class="panel-header">
        <div class="panel-header-copy">
          <div class="eyebrow">Inspector</div>
          <div class="panel-header-title-row">
            <h3>{props.node.label}</h3>
          </div>
        </div>
      </div>

      <InspectorSection title="Agent" defaultOpen>
        <dl class="workflow-authoring-inspector-facts">
          <div>
            <dt>Model</dt>
            <dd>{props.node.agent.model || "Workflow default"}</dd>
          </div>
          <div>
            <dt>Follow-up questions</dt>
            <dd>{props.node.agent.requestUserInput ? "Allowed" : "Disabled"}</dd>
          </div>
        </dl>
        <TextValue label="System prompt" value={props.node.agent.system_prompt} />
        <TextValue label="Task prompt" value={props.node.agent.task_prompt} />
      </InspectorSection>

      <InspectorSection title="Handoff" defaultOpen summary={handoffFormat()}>
        <Show
          when={props.node.agent.handoff?.format === "markdown"}
          fallback={<TextValue label="JSON output schema" value={outputSchema()} />}
        >
          <TextValue
            label="Markdown template"
            value={
              props.node.agent.handoff?.format === "markdown"
                ? props.node.agent.handoff.template
                : ""
            }
          />
        </Show>
      </InspectorSection>

      <InspectorSection title="Tools">
        <dl class="workflow-authoring-inspector-facts">
          <div>
            <dt>Approval mode</dt>
            <dd>{props.node.agent.tools.approvalMode ?? "Workflow default"}</dd>
          </div>
        </dl>
      </InspectorSection>
    </section>
  );
}
