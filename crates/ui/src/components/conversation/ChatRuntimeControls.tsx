import { createMemo, Show } from "solid-js";
import { TextSelect } from "@/components";
import { useAppContext } from "../../context/AppContext";
import { APPROVAL_MODE_OPTIONS } from "../../forms/approvalModeOptions";
import {
  defaultReasoningBudgetTokens,
  defaultReasoningEffort,
  fastModeAvailable,
  reasoningEffortOptions,
} from "@/lib/workflow";

const ADD_PROJECT_VALUE = "__openflow_add_project__";

export function ChatRuntimeControls() {
  const ctx = useAppContext();
  const chat = createMemo(() => ctx.activeChat());
  const modelOptions = createMemo(() => {
    const models = [...ctx.activeProfileMemo().known_models];
    const current = chat()?.config.model;
    if (current && !models.includes(current)) {
      models.unshift(current);
    }
    const defaultModel = ctx.activeProfileMemo().default_model;
    if (defaultModel && !models.includes(defaultModel)) {
      models.unshift(defaultModel);
    }
    return [
      {
        value: "",
        label: defaultModel ? `Default (${defaultModel})` : "Default model",
      },
      ...models.map((model) => ({ value: model, label: model })),
    ];
  });
  const selectedModel = createMemo(() => chat()?.config.model ?? "");
  const effortOptions = createMemo(() =>
    reasoningEffortOptions(ctx.activeProfileMemo()),
  );
  const defaultEffortLabel = createMemo(() => {
    const effort = defaultReasoningEffort(ctx.activeProfileMemo());
    if (!effort) {
      return "Default effort";
    }
    return (
      effortOptions().find((option) => option.value === effort)?.label ?? effort
    );
  });
  const effortSelectOptions = createMemo(() => [
    { value: "", label: defaultEffortLabel() },
    ...effortOptions().map((option) => ({
      value: option.value,
      label: option.label,
    })),
  ]);
  const approvalOptions = APPROVAL_MODE_OPTIONS;
  const projectOptions = createMemo(() => [
    { value: "", label: "None" },
    ...ctx.projects().map((project) => ({
      value: project.id,
      label: project.name,
    })),
    { value: ADD_PROJECT_VALUE, label: "Add Project…" },
  ]);
  const selectedEffortOption = createMemo(() =>
    effortOptions().find(
      (option) => option.value === chat()?.config.reasoningEffort,
    ),
  );
  const controlsDisabled = () => ctx.startingRun();

  return (
    <Show when={chat()}>
      {(currentChat) => (
        <div
          class="composer-runtime-controls direct-chat-runtime-controls"
          aria-label="Chat runtime settings"
        >
          <TextSelect
            class="composer-runtime-select direct-chat-project-select"
            menuPlacement="above"
            valuePrefix="Project: "
            value={currentChat().config.projectId ?? ""}
            options={projectOptions()}
            disabled={controlsDisabled() || currentChat().runId !== null}
            aria-label="Chat project"
            onChange={(event) => {
              if (event.currentTarget.value === ADD_PROJECT_VALUE) {
                const chatId = currentChat().id;
                const projectCount = ctx.projects().length;
                void (async () => {
                  await ctx.handleAddProject();
                  const addedProject = ctx.projects()[projectCount];
                  const activeChat = ctx.activeChat();
                  if (!addedProject || activeChat?.id !== chatId) return;
                  await ctx.handleUpdateChatConfig({
                    ...activeChat.config,
                    projectId: addedProject.id,
                  });
                })();
                return;
              }
              void ctx.handleUpdateChatConfig({
                ...currentChat().config,
                projectId: event.currentTarget.value || null,
              });
            }}
          />
          <TextSelect
            class="composer-runtime-select"
            menuPlacement="above"
            valuePrefix="Model: "
            value={selectedModel()}
            options={modelOptions()}
            disabled={controlsDisabled() || modelOptions().length === 0}
            aria-label="Chat model"
            onChange={(event) => {
              void ctx.handleUpdateChatConfig({
                ...currentChat().config,
                model: event.currentTarget.value || null,
              });
            }}
          />
          <Show when={fastModeAvailable(ctx.activeProfileMemo())}>
            <TextSelect
              class="composer-runtime-select"
              menuPlacement="above"
              valuePrefix="Speed: "
              value={currentChat().config.fastMode ? "fast" : "standard"}
              options={[
                { value: "standard", label: "Standard" },
                { value: "fast", label: "Fast" },
              ]}
              disabled={controlsDisabled()}
              aria-label="Chat speed"
              onChange={(event) => {
                void ctx.handleUpdateChatConfig({
                  ...currentChat().config,
                  fastMode: event.currentTarget.value === "fast",
                });
              }}
            />
          </Show>
          <TextSelect
            class="composer-runtime-select"
            menuPlacement="above"
            valuePrefix="Approval: "
            value={currentChat().config.approvalMode}
            options={approvalOptions}
            disabled={controlsDisabled()}
            aria-label="Chat tool approval mode"
            onChange={(event) => {
              void ctx.handleUpdateChatConfig({
                ...currentChat().config,
                approvalMode:
                  event.currentTarget
                    .value as (typeof APPROVAL_MODE_OPTIONS)[number]["value"],
              });
            }}
          />
          <TextSelect
            class="composer-runtime-select"
            menuPlacement="above"
            valuePrefix="Effort: "
            value={currentChat().config.reasoningEffort ?? ""}
            options={effortSelectOptions()}
            disabled={controlsDisabled()}
            aria-label="Chat reasoning effort"
            onChange={(event) => {
              const nextEffort = event.currentTarget.value || null;
              const option = effortOptions().find(
                (entry) => entry.value === nextEffort,
              );
              const defaultBudget =
                nextEffort === null
                  ? null
                  : defaultReasoningBudgetTokens(ctx.activeProfileMemo())[
                      nextEffort
                    ] ?? null;
              void ctx.handleUpdateChatConfig({
                ...currentChat().config,
                reasoningEffort: nextEffort,
                reasoningBudgetTokens: option?.uses_budget_tokens
                  ? currentChat().config.reasoningBudgetTokens ?? defaultBudget
                  : null,
              });
            }}
          />
          <Show when={selectedEffortOption()?.uses_budget_tokens}>
            <input
              class="composer-runtime-budget"
              type="number"
              min={1}
              step={1}
              disabled={controlsDisabled()}
              aria-label="Chat reasoning budget tokens"
              value={currentChat().config.reasoningBudgetTokens ?? ""}
              onInput={(event) => {
                const parsed = Number.parseInt(event.currentTarget.value, 10);
                void ctx.handleUpdateChatConfig({
                  ...currentChat().config,
                  reasoningBudgetTokens:
                    Number.isFinite(parsed) && parsed > 0 ? parsed : null,
                });
              }}
            />
          </Show>
        </div>
      )}
    </Show>
  );
}
