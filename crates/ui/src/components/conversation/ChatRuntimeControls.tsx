import {
  createEffect,
  createMemo,
  createSignal,
  onCleanup,
  Show,
} from "solid-js";
import ChevronDown from "lucide-solid/icons/chevron-down";
import { TextSelect } from "@/components";
import { useAppContext } from "../../context/AppContext";
import type {
  ApprovalMode,
  Project,
  ProviderProfile,
  WorkflowAuthoringRuntimeConfig,
} from "../../lib/types";
import {
  APPROVAL_MODE_OPTIONS,
  writeStoredApprovalMode,
} from "../../forms/approvalModeOptions";
import {
  defaultReasoningBudgetTokens,
  defaultReasoningEffort,
  fastModeAvailable,
  reasoningEffortOptions,
} from "@/lib/workflow";

const ADD_PROJECT_VALUE = "__openflow_add_project__";
const RUNTIME_MENU_GAP_PX = 6;
const RUNTIME_MENU_MARGIN_PX = 8;
const RUNTIME_MENU_WIDTH_PX = 320;

type RuntimeMenuStyle = {
  top: string;
  left: string;
  width: string;
  transform: string;
  "max-height"?: string;
};

type RuntimeConfig = WorkflowAuthoringRuntimeConfig;

interface ConversationRuntimeControlsProps {
  label: "Chat" | "Authoring";
  profile: ProviderProfile;
  projects: Project[];
  projectId: string | null;
  projectDisabled: boolean;
  controlsDisabled: boolean;
  config: RuntimeConfig;
  approvalMode?: ApprovalMode;
  onProjectChange: (projectId: string | null) => void;
  onAddProject: () => void;
  onConfigChange: (config: RuntimeConfig) => void;
  onApprovalModeChange?: (approvalMode: ApprovalMode) => void;
}

function ConversationRuntimeControls(props: ConversationRuntimeControlsProps) {
  const [runtimeMenuOpen, setRuntimeMenuOpen] = createSignal(false);
  const [runtimeMenuStyle, setRuntimeMenuStyle] =
    createSignal<RuntimeMenuStyle>({
      top: "0px",
      left: "0px",
      width: `${RUNTIME_MENU_WIDTH_PX}px`,
      transform: "translateY(-100%)",
    });
  let runtimeMenuRootRef: HTMLDivElement | undefined;
  let runtimeMenuTriggerRef: HTMLButtonElement | undefined;
  const modelOptions = createMemo(() => {
    const models = [...props.profile.known_models];
    const current = props.config.model;
    if (current && !models.includes(current)) {
      models.unshift(current);
    }
    const defaultModel = props.profile.default_model;
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
  const selectedModel = createMemo(() => props.config.model ?? "");
  const effortOptions = createMemo(() =>
    reasoningEffortOptions(props.profile),
  );
  const defaultEffortLabel = createMemo(() => {
    const effort = defaultReasoningEffort(props.profile);
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
    ...props.projects.map((project) => ({
      value: project.id,
      label: project.name,
    })),
    { value: ADD_PROJECT_VALUE, label: "Add Project…" },
  ]);
  const selectedEffortOption = createMemo(() =>
    effortOptions().find(
      (option) => option.value === props.config.reasoningEffort,
    ),
  );
  const selectedModelLabel = createMemo(
    () =>
      modelOptions().find((option) => option.value === selectedModel())?.label ??
      selectedModel(),
  );
  const selectedEffortLabel = createMemo(
    () =>
      effortSelectOptions().find(
        (option) => option.value === (props.config.reasoningEffort ?? ""),
      )?.label ?? defaultEffortLabel(),
  );

  const syncRuntimeMenuPosition = () => {
    const trigger = runtimeMenuTriggerRef;
    if (!trigger) return;
    const rect = trigger.getBoundingClientRect();
    const effectiveScale =
      trigger.offsetWidth > 0 ? rect.width / trigger.offsetWidth : 1;
    const scale =
      Number.isFinite(effectiveScale) && effectiveScale > 0 ? effectiveScale : 1;
    const viewport = window.visualViewport;
    const viewportLeft = (viewport?.offsetLeft ?? 0) / scale;
    const viewportTop = (viewport?.offsetTop ?? 0) / scale;
    const viewportWidth = (viewport?.width ?? window.innerWidth) / scale;
    const triggerLeft = rect.left / scale;
    const menuWidth = Math.min(
      RUNTIME_MENU_WIDTH_PX,
      viewportWidth - RUNTIME_MENU_MARGIN_PX * 2,
    );
    const maxLeft =
      viewportLeft + viewportWidth - menuWidth - RUNTIME_MENU_MARGIN_PX;
    const left = Math.max(
      viewportLeft + RUNTIME_MENU_MARGIN_PX,
      Math.min(triggerLeft, maxLeft),
    );
    const availableAbove =
      rect.top / scale -
      viewportTop -
      RUNTIME_MENU_GAP_PX -
      RUNTIME_MENU_MARGIN_PX;

    setRuntimeMenuStyle({
      top: `${rect.top / scale - RUNTIME_MENU_GAP_PX}px`,
      left: `${left}px`,
      width: `${menuWidth}px`,
      transform: "translateY(-100%)",
      "max-height":
        availableAbove > 0 ? `${availableAbove}px` : undefined,
    });
  };

  const closeRuntimeMenu = () => setRuntimeMenuOpen(false);

  createEffect(() => {
    if (!runtimeMenuOpen()) return;

    const onDocumentMouseDown = (event: MouseEvent) => {
      const root = runtimeMenuRootRef;
      const target = event.target;
      if (
        !root ||
        !(target instanceof Node) ||
        root.contains(target) ||
        (target instanceof Element && target.closest(".text-select-menu"))
      ) {
        return;
      }
      closeRuntimeMenu();
    };
    const onScroll = (event: Event) => {
      const root = runtimeMenuRootRef;
      const target = event.target;
      if (
        root &&
        target instanceof Node &&
        (root.contains(target) ||
          (target instanceof Element && target.closest(".text-select-menu")))
      ) {
        return;
      }
      closeRuntimeMenu();
    };

    document.addEventListener("mousedown", onDocumentMouseDown);
    window.addEventListener("scroll", onScroll, true);
    window.addEventListener("resize", closeRuntimeMenu);
    onCleanup(() => {
      document.removeEventListener("mousedown", onDocumentMouseDown);
      window.removeEventListener("scroll", onScroll, true);
      window.removeEventListener("resize", closeRuntimeMenu);
    });
  });

  return (
    <div
      class="composer-runtime-controls direct-chat-runtime-controls"
      aria-label={`${props.label} runtime settings`}
    >
      <TextSelect
        class="composer-runtime-select direct-chat-project-select"
        menuPlacement="above"
        valuePrefix="Project: "
        value={props.projectId ?? ""}
        options={projectOptions()}
        disabled={props.controlsDisabled || props.projectDisabled}
        aria-label={`${props.label} project`}
        onChange={(event) => {
          if (event.currentTarget.value === ADD_PROJECT_VALUE) {
            props.onAddProject();
            return;
          }
          props.onProjectChange(event.currentTarget.value || null);
        }}
      />
      <Show when={props.approvalMode !== undefined}>
        <TextSelect
          class="composer-runtime-select direct-chat-approval-select"
          menuPlacement="above"
          valuePrefix="Approval: "
          value={props.approvalMode ?? "read_only"}
          options={approvalOptions}
          disabled={props.controlsDisabled}
          aria-label={`${props.label} tool approval mode`}
          onChange={(event) => {
            props.onApprovalModeChange?.(
              event.currentTarget
                .value as (typeof APPROVAL_MODE_OPTIONS)[number]["value"],
            );
          }}
        />
      </Show>
      <div
        ref={runtimeMenuRootRef}
        class="chat-runtime-menu-root"
        onKeyDown={(event) => {
          if (event.key !== "Escape") return;
          closeRuntimeMenu();
          runtimeMenuTriggerRef?.focus();
        }}
      >
        <button
          ref={runtimeMenuTriggerRef}
          type="button"
          class="chat-runtime-menu-trigger"
          aria-label={`${props.label} runtime settings: model ${selectedModelLabel()}, effort ${selectedEffortLabel()}`}
          aria-haspopup="dialog"
          aria-expanded={runtimeMenuOpen()}
          disabled={props.controlsDisabled || modelOptions().length === 0}
          onClick={() => {
            if (runtimeMenuOpen()) {
              closeRuntimeMenu();
              return;
            }
            syncRuntimeMenuPosition();
            setRuntimeMenuOpen(true);
          }}
        >
          <span class="chat-runtime-menu-summary">
            <span>{selectedModelLabel()}</span>
            <span aria-hidden="true">·</span>
            <span>{selectedEffortLabel()}</span>
          </span>
          <ChevronDown
            class="chat-runtime-menu-chevron"
            classList={{ "is-open": runtimeMenuOpen() }}
            aria-hidden="true"
            width={14}
            height={14}
          />
        </button>
        <Show when={runtimeMenuOpen()}>
          <div
            ref={(_element) => {
              queueMicrotask(syncRuntimeMenuPosition);
            }}
            class="chat-runtime-menu-popover"
            role="dialog"
            aria-label={`${props.label} runtime settings`}
            style={runtimeMenuStyle()}
          >
            <div class="chat-runtime-menu-row">
              <span>Model</span>
              <TextSelect
                class="chat-runtime-menu-select"
                menuPlacement="horizontal"
                portalMenu
                openOnHover
                value={selectedModel()}
                options={modelOptions()}
                disabled={props.controlsDisabled || modelOptions().length === 0}
                aria-label={`${props.label} model`}
                onChange={(event) => {
                  props.onConfigChange({
                    ...props.config,
                    model: event.currentTarget.value || null,
                  });
                }}
              />
            </div>
            <div class="chat-runtime-menu-row">
              <span>Effort</span>
              <TextSelect
                class="chat-runtime-menu-select"
                menuPlacement="horizontal"
                portalMenu
                openOnHover
                value={props.config.reasoningEffort ?? ""}
                options={effortSelectOptions()}
                disabled={props.controlsDisabled}
                aria-label={`${props.label} reasoning effort`}
                onChange={(event) => {
                  const nextEffort = event.currentTarget.value || null;
                  const option = effortOptions().find(
                    (entry) => entry.value === nextEffort,
                  );
                  const defaultBudget =
                    nextEffort === null
                      ? null
                      : defaultReasoningBudgetTokens(props.profile)[nextEffort] ??
                        null;
                  props.onConfigChange({
                    ...props.config,
                    reasoningEffort: nextEffort,
                    reasoningBudgetTokens: option?.uses_budget_tokens
                      ? props.config.reasoningBudgetTokens ?? defaultBudget
                      : null,
                  });
                }}
              />
            </div>
            <Show when={fastModeAvailable(props.profile)}>
              <div class="chat-runtime-menu-row">
                <span>Speed</span>
                <TextSelect
                  class="chat-runtime-menu-select"
                  menuPlacement="horizontal"
                  portalMenu
                  openOnHover
                  value={props.config.fastMode ? "fast" : "standard"}
                  options={[
                    { value: "standard", label: "Standard" },
                    { value: "fast", label: "Fast" },
                  ]}
                  disabled={props.controlsDisabled}
                  aria-label={`${props.label} speed`}
                  onChange={(event) => {
                    props.onConfigChange({
                      ...props.config,
                      fastMode: event.currentTarget.value === "fast",
                    });
                  }}
                />
              </div>
            </Show>
            <Show when={selectedEffortOption()?.uses_budget_tokens}>
              <div class="chat-runtime-menu-row chat-runtime-menu-budget-row">
                <span>Reasoning budget</span>
                <input
                  class="chat-runtime-menu-budget"
                  type="number"
                  min={1}
                  step={1}
                  disabled={props.controlsDisabled}
                  aria-label={`${props.label} reasoning budget tokens`}
                  value={props.config.reasoningBudgetTokens ?? ""}
                  onInput={(event) => {
                    const parsed = Number.parseInt(event.currentTarget.value, 10);
                    props.onConfigChange({
                      ...props.config,
                      reasoningBudgetTokens:
                        Number.isFinite(parsed) && parsed > 0 ? parsed : null,
                    });
                  }}
                />
              </div>
            </Show>
          </div>
        </Show>
      </div>
    </div>
  );
}

export function ChatRuntimeControls() {
  const ctx = useAppContext();
  return (
    <Show when={ctx.activeChat()}>
      {(chat) => (
        <ConversationRuntimeControls
          label="Chat"
          profile={ctx.activeProfileMemo()}
          projects={ctx.projects()}
          projectId={chat().config.projectId}
          projectDisabled={chat().runId !== null}
          controlsDisabled={ctx.startingRun()}
          config={{
            model: chat().config.model,
            reasoningEffort: chat().config.reasoningEffort,
            reasoningBudgetTokens: chat().config.reasoningBudgetTokens,
            fastMode: chat().config.fastMode ?? false,
          }}
          approvalMode={chat().config.approvalMode}
          onProjectChange={(projectId) => {
            void ctx.handleUpdateChatConfig({ ...chat().config, projectId });
          }}
          onAddProject={() => {
            const chatId = chat().id;
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
          }}
          onConfigChange={(config) => {
            void ctx.handleUpdateChatConfig({ ...chat().config, ...config });
          }}
          onApprovalModeChange={(approvalMode) => {
            writeStoredApprovalMode(globalThis.localStorage, approvalMode);
            void ctx.handleUpdateChatConfig({ ...chat().config, approvalMode });
          }}
        />
      )}
    </Show>
  );
}

export function WorkflowAuthoringRuntimeControls() {
  const ctx = useAppContext();
  return (
    <ConversationRuntimeControls
      label="Authoring"
      profile={ctx.activeProfileMemo()}
      projects={ctx.projects()}
      projectId={ctx.workflowAuthoringTargetProjectId()}
      projectDisabled={
        ctx.workflowAuthoringBusy() ||
        ctx.workflowAuthoringMessages().length > 0
      }
      controlsDisabled={!ctx.workflowAuthoringSessionReady()}
      config={ctx.workflowAuthoringRuntimeConfig()}
      onProjectChange={(projectId) => {
        void ctx.handleWorkflowAuthoringProjectChange(projectId);
      }}
      onAddProject={() => {
        const projectCount = ctx.projects().length;
        void (async () => {
          await ctx.handleAddProject();
          const addedProject = ctx.projects()[projectCount];
          if (!addedProject) return;
          await ctx.handleWorkflowAuthoringProjectChange(addedProject.id);
        })();
      }}
      onConfigChange={ctx.handleUpdateWorkflowAuthoringRuntimeConfig}
    />
  );
}
