import { createEffect, For } from "solid-js";

export function AgentConfigForm(props: {
  model: string;
  onModelChange: (value: string) => void;
  autoStart: boolean;
  onAutoStartChange: (value: boolean) => void;
  systemPrompt: string;
  onSystemPromptChange: (value: string) => void;
  taskPrompt: string;
  onTaskPromptChange: (value: string) => void;
  schemaJson: string;
  onSchemaChange: (value: string) => void;
  knownModels: readonly string[];
  defaultModel: string | null;
  listId: string;
  systemPromptRows?: number;
  taskPromptRows?: number;
  schemaRows?: number;
}) {
  const effectiveModel = () => props.model || props.defaultModel || "";
  createEffect(() => {
    if (!props.model && props.defaultModel) {
      props.onModelChange(props.defaultModel);
    }
  });
  return (
    <>
      <label>
        <span>Model</span>
        <input
          class="text-input"
          value={effectiveModel()}
          list={props.listId}
          onInput={(event) => props.onModelChange(event.currentTarget.value)}
        />
        <datalist id={props.listId}>
          <For each={props.knownModels}>{(model) => <option value={model} />}</For>
        </datalist>
      </label>
      <label class="checkbox-row">
        <input
          type="checkbox"
          checked={props.autoStart}
          onChange={(event) => props.onAutoStartChange(event.currentTarget.checked)}
        />
        <span>Auto-start without pausing for human input</span>
      </label>
      <label>
        <span>System prompt</span>
        <textarea
          class="text-area"
          rows={props.systemPromptRows ?? 4}
          value={props.systemPrompt}
          onInput={(event) => props.onSystemPromptChange(event.currentTarget.value)}
        />
      </label>
      <label>
        <span>Task prompt</span>
        <textarea
          class="text-area"
          rows={props.taskPromptRows ?? 3}
          value={props.taskPrompt}
          onInput={(event) => props.onTaskPromptChange(event.currentTarget.value)}
        />
      </label>
      <label>
        <span>JSON output schema</span>
        <textarea
          class="text-area code"
          rows={props.schemaRows ?? 8}
          value={props.schemaJson}
          onInput={(event) => props.onSchemaChange(event.currentTarget.value)}
        />
      </label>
    </>
  );
}
