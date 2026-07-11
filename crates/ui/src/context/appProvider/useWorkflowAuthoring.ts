import { createEffect, createMemo, createSignal, onCleanup, onMount, type Accessor } from "solid-js";
import * as desktop from "../../api";
import type {
  AppSettings,
  ProviderReadiness,
  Screen,
  Workflow,
  WorkflowAuthoringMessage,
  WorkflowAuthoringValidation,
} from "../../lib/types";
import { normalizeWorkflowLayout, replaceWorkflow } from "../../lib/workflow";
import { normalizeError } from "../../lib/utils";

type ToastHandler = (message: string, context?: string) => void;

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
  const [workflowAuthoringTargetProjectId, setWorkflowAuthoringTargetProjectId] =
    createSignal<string | null>(null);
  const [workflowAuthoringBusy, setWorkflowAuthoringBusy] = createSignal(false);
  const [workflowAuthoringThinkingContent, setWorkflowAuthoringThinkingContent] =
    createSignal("");
  const workflowAuthoringSessionReady = createMemo(
    () => workflowAuthoringSessionId() !== null,
  );

  const resetWorkflowAuthoringSession = () => {
    setWorkflowAuthoringSessionId(null);
    setWorkflowAuthoringBusy(false);
    setWorkflowAuthoringThinkingContent("");
    setWorkflowAuthoringTargetProjectId(null);
  };

  const releaseWorkflowAuthoringSession = (sessionId: string | null) => {
    if (sessionId) {
      void desktop.endWorkflowAuthoring(sessionId);
    }
    resetWorkflowAuthoringSession();
  };

  createEffect(() => {
    if (params.screen() !== "workflow-authoring" && workflowAuthoringSessionId() !== null) {
      releaseWorkflowAuthoringSession(workflowAuthoringSessionId());
    }
  });

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
      }
    }).then((stop) => {
      unlistenDraft = stop;
    });
    onCleanup(() => {
      unlistenThinking?.();
      unlistenDraft?.();
    });
  });

  const handleOpenWorkflowAuthoring = async (
    baseWorkflow?: Workflow,
    targetProjectId: string | null = null,
  ) => {
    if (
      params.screen() === "workflow-authoring" &&
      workflowAuthoringSessionId() !== null &&
      baseWorkflow === undefined
    ) {
      return;
    }
    const priorSessionId = workflowAuthoringSessionId();
    if (priorSessionId !== null) {
      void desktop.endWorkflowAuthoring(priorSessionId);
    }
    resetWorkflowAuthoringSession();
    setWorkflowAuthoringMessages([]);
    setWorkflowAuthoringValidation(null);
    setWorkflowAuthoringDraft(baseWorkflow ?? null);
    setWorkflowAuthoringTargetProjectId(targetProjectId);
    params.navigateToScreen("workflow-authoring");
    void params.refreshReadiness();
    try {
      const started = await desktop.startWorkflowAuthoring(
        baseWorkflow ?? null,
        targetProjectId,
      );
      setWorkflowAuthoringSessionId(started.sessionId);
      if (started.draft) {
        setWorkflowAuthoringDraft(normalizeWorkflowLayout(started.draft));
      }
    } catch (error) {
      params.showErrorToast(normalizeError(error));
      params.navigateToScreen("editor");
    }
  };

  const handleCloseWorkflowAuthoring = () => {
    releaseWorkflowAuthoringSession(workflowAuthoringSessionId());
    params.navigateToScreen("editor");
  };

  const handleWorkflowAuthoringSend = async (message: string) => {
    const sessionId = workflowAuthoringSessionId();
    const trimmed = message.trim();
    if (!trimmed || workflowAuthoringBusy()) return;
    if (!sessionId) {
      params.showErrorToast("Authoring session is not ready yet. Try opening Build with AI again.");
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
      );
      setWorkflowAuthoringMessages(result.messages);
      setWorkflowAuthoringValidation(result.validation);
      setWorkflowAuthoringDraft(result.draft ? normalizeWorkflowLayout(result.draft) : null);
    } catch (error) {
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
      setWorkflowAuthoringBusy(false);
      setWorkflowAuthoringThinkingContent("");
    }
  };

  const handleApplyWorkflowAuthoringDraft = async () => {
    const draft = workflowAuthoringDraft();
    const validation = workflowAuthoringValidation();
    if (!draft || !validation?.valid) return;
    const normalizedDraft = normalizeWorkflowLayout(draft);
    const targetProjectId = workflowAuthoringTargetProjectId();
    if (params.workflows().some((workflow) => workflow.id === draft.id)) {
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
      resetWorkflowAuthoringSession();
      setWorkflowAuthoringTargetProjectId(null);
      params.navigateToScreen("editor");
      params.showSuccessToast(`Applied workflow "${saved.name}"`);
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
    handleOpenWorkflowAuthoring,
    handleCloseWorkflowAuthoring,
    handleWorkflowAuthoringSend,
    handleApplyWorkflowAuthoringDraft,
  };
}
