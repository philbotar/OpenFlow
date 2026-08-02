import { createEffect, createMemo, createSignal } from "solid-js";
import { TextSelect } from "@/components";
import { HandoffEditor } from "../../forms/HandoffEditor";
import { SkillPromptTextarea } from "../../forms/SkillPromptTextarea";
import { ToolConfigEditor } from "../../forms/ToolConfigEditor";
import type {
  AppSettings,
  Node,
  SkillSummary,
  WorkflowSettings,
} from "../../lib/types";
import {
  nodeProviderProfile,
  providerDisplayOrder,
  workflowProviderProfile,
} from "../../lib/workflow";
import { InspectorSection } from "../InspectorSection";

type NodeMutator = (node: Node) => void;

const stringifySchema = (value: unknown) => JSON.stringify(value, null, 2) ?? "";

export function AuthoringDraftInspector(props: {
  node: Node;
  settings: AppSettings;
  workflowSettings: WorkflowSettings;
  availableSkills: readonly SkillSummary[];
  onNodeChange: (mutator: NodeMutator) => void;
}) {
  const sharedProviderProfile = createMemo(() =>
    workflowProviderProfile(props.settings, props.workflowSettings),
  );
  const providerProfile = createMemo(() =>
    nodeProviderProfile(props.settings, props.workflowSettings, props.node.agent),
  );
  const providerOptions = createMemo(() => [
    {
      value: "",
      label: `Use shared provider (${sharedProviderProfile().display_name})`,
    },
    ...providerDisplayOrder(props.settings).map((providerId) => ({
      value: providerId,
      label: props.settings.providers[providerId].display_name,
    })),
  ]);
  const modelOptions = createMemo(() => {
    const models = [...providerProfile().known_models];
    const current = props.node.agent.model;
    if (current && !models.includes(current)) models.unshift(current);
    return [
      {
        value: "",
        label: providerProfile().default_model
          ? `Workflow default (${providerProfile().default_model})`
          : "Workflow default",
      },
      ...models.map((model) => ({ value: model, label: model })),
    ];
  });

  const [schemaText, setSchemaText] = createSignal(
    stringifySchema(props.node.agent.output_schema),
  );
  let schemaNodeId = props.node.id;

  createEffect(() => {
    const nodeId = props.node.id;
    if (nodeId === schemaNodeId) return;
    schemaNodeId = nodeId;
    setSchemaText(stringifySchema(props.node.agent.output_schema));
  });

  const handleSchemaChange = (value: string) => {
    setSchemaText(value);
    try {
      const parsed = JSON.parse(value);
      props.onNodeChange((node) => {
        node.agent.output_schema = parsed;
      });
    } catch {
      // Keep malformed text local until the user fixes it.
    }
  };

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
        <label>
          <span>Provider</span>
          <TextSelect
            value={props.node.agent.providerId ?? ""}
            options={providerOptions()}
            onChange={(event) =>
              props.onNodeChange((node) => {
                node.agent.providerId = event.currentTarget.value || null;
                node.agent.model = "";
              })
            }
          />
        </label>
        <label>
          <span>Model</span>
          <TextSelect
            value={props.node.agent.model}
            options={modelOptions()}
            onChange={(event) =>
              props.onNodeChange((node) => {
                node.agent.model = event.currentTarget.value;
              })
            }
          />
        </label>
        <label class="checkbox-row">
          <input
            type="checkbox"
            checked={props.node.agent.requestUserInput ?? false}
            onChange={(event) =>
              props.onNodeChange((node) => {
                node.agent.requestUserInput = event.currentTarget.checked;
              })
            }
          />
          <span>Allow follow-up questions</span>
        </label>
        <label>
          <span>System prompt</span>
          <textarea
            class="text-area"
            rows={4}
            value={props.node.agent.system_prompt}
            onInput={(event) =>
              props.onNodeChange((node) => {
                node.agent.system_prompt = event.currentTarget.value;
              })
            }
          />
        </label>
        <div class="agent-prompt-field">
          <span>Task prompt</span>
          <SkillPromptTextarea
            value={props.node.agent.task_prompt}
            skills={props.availableSkills}
            rows={4}
            onInput={(value) =>
              props.onNodeChange((node) => {
                node.agent.task_prompt = value;
              })
            }
          />
        </div>
      </InspectorSection>

      <InspectorSection
        title="Handoff"
        summary={props.node.agent.handoff?.format === "markdown" ? "markdown" : "json"}
      >
        <HandoffEditor
          handoff={props.node.agent.handoff}
          schemaJson={schemaText()}
          onHandoffChange={(handoff) =>
            props.onNodeChange((node) => {
              node.agent.handoff = handoff;
            })
          }
          onSchemaChange={handleSchemaChange}
        />
      </InspectorSection>

      <InspectorSection title="Tools">
        <ToolConfigEditor
          config={props.node.agent.tools}
          onApprovalModeChange={(value) =>
            props.onNodeChange((node) => {
              node.agent.tools.approvalMode = value;
            })
          }
        />
      </InspectorSection>
    </section>
  );
}
