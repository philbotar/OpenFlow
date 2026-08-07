import { createEffect, createMemo, createSignal, onCleanup, onMount, type Accessor } from "solid-js";
import * as desktop from "../../api";
import type {
  AppSettings,
  ProviderReadiness,
  Screen,
  Workflow,
  WorkflowAuthoringMessage,
  WorkflowAuthoringRuntimeConfig,
  WorkflowAuthoringValidation,
} from "../../lib/types";
import { cloneWorkflow, normalizeWorkflowLayout, replaceWorkflow } from "../../lib/workflow";
import { normalizeError } from "../../lib/utils";

type ToastHandler = (message: string, context?: string) => void;

const defaultWorkflowAuthoringRuntimeConfig = (): WorkflowAuthoringRuntimeConfig => ({
  model: null,
  reasoningEffort: null,
  reasoningBudgetTokens: null,
  fastMode: false,
});

interface UseWorkflowAuthoringParams {
  screen: Accessor<Screen>;
  navigateToScreen: (screen: Screen) => void;
  settings: Accessor<AppSettings>;
  activeProviderKeyInput: Accessor<string>;
  readiness: Accessor<ProviderReadiness | null>;
  refreshReadiness: () => Promise<void>;
  workflows: Accessor<Workflow[]>;
  setWorkflows: (next: Workflow[]) => void;
  selectWorkflow: (workflow: Workflow) => void;
  persistWorkflowAuthoringDraft: (
    workflow: Workflow,
    targetProjectId: string | null,
  ) => Promise<Workflow>;
  showErrorToast: ToastHandler;
  showSuccessToast: ToastHandler;
}

export function useWorkflowAuthoring(params: UseWorkflowAuthoringParams) {
  const [workflowAuthoringSessionId, setWorkflowAuthoringSessionId] = createSignal<
    string | null
  >(null);
  const [workflowAuthoringMessages, setWorkflowAuthoringMessages] = createSignal<
    WorkflowAuthoringMessage[]
  >([]);
  const [workflowAuthoringValidation, setWorkflowAuthoringValidation] =
    createSignal<WorkflowAuthoringValidation | null>(null);
  const [workflowAuthoringDraft, setWorkflowAuthoringDraft] = createSignal<Workflow | null>(
    null,
  );
  const [workflowAuthoringDraftPending, setWorkflowAuthoringDraftPending] =
    createSignal(false);
  const [workflowAuthoringTargetProjectId, setWorkflowAuthoringTargetProjectId] =
    createSignal<string | null>(null);
  const [workflowAuthoringRuntimeConfig, setWorkflowAuthoringRuntimeConfig] =
    createSignal<WorkflowAuthoringRuntimeConfig>(
      defaultWorkflowAuthoringRuntimeConfig(),
    );
  const [workflowAuthoringBusy, setWorkflowAuthoringBusy] = createSignal(false);
  const [workflowAuthoringThinkingContent, setWorkflowAuthoringThinkingContent] =
    createSignal("");
  const workflowAuthoringSessionReady = createMemo(
    () => workflowAuthoringSessionId() !== null,
  );
  let authoringGeneration = 0;

  const updateWorkflowAuthoringDraft = (mutator: (draft: Workflow) => void) => {
    setWorkflowAuthoringDraft((current) => {
      if (!current) return current;
      const next = cloneWorkflow(current);
      mutator(next);
      setWorkflowAuthoringDraftPending(true);
      return next;
    });
  };

  const resetWorkflowAuthoringSession = () => {
    setWorkflowAuthoringSessionId(null);
    setWorkflowAuthoringBusy(false);
    setWorkflowAuthoringThinkingContent("");
    setWorkflowAuthoringTargetProjectId(null);
    setWorkflowAuthoringDraftPending(false);
  };

  const clearWorkflowAuthoringContent = () => {
    setWorkflowAuthoringMessages([]);
    setWorkflowAuthoringValidation(null);
    setWorkflowAuthoringDraft(null);
  };

  const releaseWorkflowAuthoringSession = (sessionId: string | null) => {
    if (sessionId) {
      void desktop.endWorkflowAuthoring(sessionId);
    }
    resetWorkflowAuthoringSession();
  };

  const activeSessionId = { current: null as string | null };
  createEffect(() => {
    activeSessionId.current = workflowAuthoringSessionId();
  });

  onMount(() => {
    let unlistenThinking: (() => void) | undefined;
    let unlistenDraft: (() => void) | undefined;
    void desktop.listenToWorkflowAuthoringThinking((event) => {
      const sessionId = activeSessionId.current;
      if (!sessionId || event.sessionId !== sessionId) {
        return;
      }
      if (event.delta) {
        setWorkflowAuthoringThinkingContent((current) => current + event.delta);
      }
    }).then((stop) => {
      unlistenThinking = stop;
    });
    void desktop.listenToWorkflowAuthoringDraft((event) => {
      const sessionId = activeSessionId.current;
      if (!sessionId || event.sessionId !== sessionId) {
        return;
      }
      setWorkflowAuthoringValidation(event.validation);
      if (event.draft) {
        setWorkflowAuthoringDraft(normalizeWorkflowLayout(event.draft));
        setWorkflowAuthoringDraftPending(true);
      }
    }).then((stop) => {
      unlistenDraft = stop;
    });
    onCleanup(() => {
      authoringGeneration += 1;
      unlistenThinking?.();
      unlistenDraft?.();
      const sessionId = activeSessionId.current;
      if (sessionId) {
        void desktop.endWorkflowAuthoring(sessionId);
      }
    });
  });

  const submitWorkflowAuthoringTurn = async (
    sessionId: string,
    message: string,
    generation = authoringGeneration,
  ) => {
    const trimmed = message.trim();
    if (
      !trimmed ||
      workflowAuthoringBusy() ||
      generation !== authoringGeneration ||
      workflowAuthoringSessionId() !== sessionId
    ) {
      return;
    }
    if (params.readiness()?.ready !== true) {
      params.showErrorToast(
        params.readiness()?.message ?? "Configure a provider in Settings first.",
      );
      return;
    }
    setWorkflowAuthoringMessages((current) => [
      ...current,
      { role: "user", content: trimmed },
    ]);
    setWorkflowAuthoringThinkingContent("");
    setWorkflowAuthoringBusy(true);
    try {
      const result = await desktop.workflowAuthoringTurn(
        sessionId,
        trimmed,
        params.settings(),
        params.activeProviderKeyInput() || null,
        workflowAuthoringRuntimeConfig(),
      );
      if (
        generation !== authoringGeneration ||
        workflowAuthoringSessionId() !== sessionId
      ) {
        return;
      }
      setWorkflowAuthoringMessages(result.messages);
      setWorkflowAuthoringValidation(result.validation);
      setWorkflowAuthoringDraft(result.draft ? normalizeWorkflowLayout(result.draft) : null);
      if (result.draftChanged === true) {
        setWorkflowAuthoringDraftPending(true);
      }
    } catch (error) {
      if (
        generation !== authoringGeneration ||
        workflowAuthoringSessionId() !== sessionId
      ) {
        return;
      }
      setWorkflowAuthoringMessages((current) =>
        current.filter(
          (entry, index) =>
            !(
              index === current.length - 1 &&
              entry.role === "user" &&
              entry.content === trimmed
            ),
        ),
      );
      params.showErrorToast(normalizeError(error));
    } finally {
      if (
        generation === authoringGeneration &&
        workflowAuthoringSessionId() === sessionId
      ) {
        setWorkflowAuthoringBusy(false);
        setWorkflowAuthoringThinkingContent("");
      }
    }
  };

  const handleOpenWorkflowAuthoring = async (
    baseWorkflow?: Workflow,
    targetProjectId: string | null = null,
    initialMessage?: string,
  ) => {
    if (
      workflowAuthoringSessionId() !== null &&
      baseWorkflow === undefined
    ) {
      params.navigateToScreen("workflow-authoring");
      return;
    }
    const generation = ++authoringGeneration;
    const priorSessionId = workflowAuthoringSessionId();
    if (priorSessionId !== null) {
      void desktop.endWorkflowAuthoring(priorSessionId);
    }
    resetWorkflowAuthoringSession();
    clearWorkflowAuthoringContent();
    setWorkflowAuthoringRuntimeConfig(defaultWorkflowAuthoringRuntimeConfig());
    setWorkflowAuthoringDraft(baseWorkflow ?? null);
    setWorkflowAuthoringDraftPending(false);
    setWorkflowAuthoringTargetProjectId(targetProjectId);
    params.navigateToScreen("workflow-authoring");
    const readinessRefresh = params.refreshReadiness();
    try {
      const started = await desktop.startWorkflowAuthoring(
        baseWorkflow ?? null,
        targetProjectId,
      );
      if (generation !== authoringGeneration) {
        void desktop.endWorkflowAuthoring(started.sessionId);
        return;
      }
      setWorkflowAuthoringSessionId(started.sessionId);
      if (started.draft) {
        setWorkflowAuthoringDraft(normalizeWorkflowLayout(started.draft));
      }
      if (initialMessage?.trim()) {
        await readinessRefresh;
        if (generation !== authoringGeneration) {
          return;
        }
        await submitWorkflowAuthoringTurn(
          started.sessionId,
          initialMessage,
          generation,
        );
      }
    } catch (error) {
      if (generation !== authoringGeneration) {
        return;
      }
      params.showErrorToast(normalizeError(error));
      params.navigateToScreen("editor");
    }
  };

  const handleCloseWorkflowAuthoring = () => {
    authoringGeneration += 1;
    releaseWorkflowAuthoringSession(workflowAuthoringSessionId());
    clearWorkflowAuthoringContent();
    setWorkflowAuthoringRuntimeConfig(defaultWorkflowAuthoringRuntimeConfig());
    params.navigateToScreen("editor");
  };

  const handleWorkflowAuthoringProjectChange = async (
    targetProjectId: string | null,
  ) => {
    if (
      targetProjectId === workflowAuthoringTargetProjectId() ||
      workflowAuthoringBusy() ||
      workflowAuthoringMessages().length > 0 ||
      !workflowAuthoringSessionReady()
    ) {
      return;
    }

    const generation = ++authoringGeneration;
    const priorSessionId = workflowAuthoringSessionId();
    const baseWorkflow = workflowAuthoringDraft();
    if (priorSessionId) {
      void desktop.endWorkflowAuthoring(priorSessionId);
    }
    setWorkflowAuthoringSessionId(null);
    setWorkflowAuthoringThinkingContent("");
    setWorkflowAuthoringTargetProjectId(targetProjectId);

    try {
      const started = await desktop.startWorkflowAuthoring(
        baseWorkflow,
        targetProjectId,
      );
      if (generation !== authoringGeneration) {
        void desktop.endWorkflowAuthoring(started.sessionId);
        return;
      }
      setWorkflowAuthoringSessionId(started.sessionId);
      if (started.draft) {
        setWorkflowAuthoringDraft(normalizeWorkflowLayout(started.draft));
      }
    } catch (error) {
      if (generation !== authoringGeneration) {
        return;
      }
      params.showErrorToast(normalizeError(error));
      params.navigateToScreen("editor");
    }
  };

  const handleUpdateWorkflowAuthoringRuntimeConfig = (
    config: WorkflowAuthoringRuntimeConfig,
  ) => {
    setWorkflowAuthoringRuntimeConfig(config);
  };

  const handleWorkflowAuthoringSend = async (message: string) => {
    const sessionId = workflowAuthoringSessionId();
    if (!sessionId) {
      params.showErrorToast("Authoring session is not ready yet. Try opening Build with AI again.");
      return;
    }
    await submitWorkflowAuthoringTurn(sessionId, message);
  };

  const handleApplyWorkflowAuthoringDraft = async () => {
    const draft = workflowAuthoringDraft();
    const validation = workflowAuthoringValidation();
    if (!draft || !validation?.valid) return;
    const normalizedDraft = normalizeWorkflowLayout(draft);
    const targetProjectId = workflowAuthoringTargetProjectId();
    const updatingExistingWorkflow = params
      .workflows()
      .some((workflow) => workflow.id === draft.id);
    if (updatingExistingWorkflow) {
      params.setWorkflows(replaceWorkflow(params.workflows(), normalizedDraft));
    } else {
      params.setWorkflows([...params.workflows(), normalizedDraft]);
    }
    params.selectWorkflow(normalizedDraft);
    try {
      const saved = await params.persistWorkflowAuthoringDraft(
        normalizedDraft,
        targetProjectId,
      );
      params.setWorkflows(replaceWorkflow(params.workflows(), saved));
      params.selectWorkflow(saved);
      const sessionId = workflowAuthoringSessionId();
      if (sessionId) {
        void desktop.endWorkflowAuthoring(sessionId);
      }
      resetWorkflowAuthoringSession();
      setWorkflowAuthoringTargetProjectId(null);
      params.navigateToScreen("editor");
      params.showSuccessToast(
        updatingExistingWorkflow
          ? `Updated workflow "${saved.name}".`
          : `Created workflow "${saved.name}". Click Run to start it.`,
      );
    } catch (error) {
      params.showErrorToast(normalizeError(error));
    }
  };

  return {
    workflowAuthoringBusy,
    workflowAuthoringThinkingContent,
    workflowAuthoringSessionReady,
    workflowAuthoringMessages,
    workflowAuthoringValidation,
    workflowAuthoringDraft,
    workflowAuthoringDraftPending,
    workflowAuthoringTargetProjectId,
    workflowAuthoringRuntimeConfig,
    updateWorkflowAuthoringDraft,
    handleOpenWorkflowAuthoring,
    handleCloseWorkflowAuthoring,
    handleWorkflowAuthoringProjectChange,
    handleUpdateWorkflowAuthoringRuntimeConfig,
    handleWorkflowAuthoringSend,
    handleApplyWorkflowAuthoringDraft,
  };
}
