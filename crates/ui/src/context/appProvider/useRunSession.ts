import { createMemo, createSignal, type Accessor, type Setter } from "solid-js";
import * as desktop from "../../api";
import type {
  AppSettings,
  BottomTab,
  Chat,
  ChatConfig,
  NodeId,
  ProjectFileReference,
  ProviderReadiness,
  RunSummary,
  Workflow,
  WorkflowRunState,
  NodeRuntimeConfigUpdate,
  UserMessageInput,
} from "../../lib/types";
import {
  activeProfile,
  canSendIdleRunKickoff,
  createIdleRunState,
  isGlobalRunEntryNodeId,
  replayContinuationNodeId,
} from "../../lib/workflow";
import { clampDockHeight, normalizeError, viewportHeight } from "../../lib/utils";

type ToastHandler = (message: string, context?: string) => void;

interface UseRunSessionParams {
  activeWorkflow: Accessor<Workflow | undefined>;
  activeChat: Accessor<Chat | null>;
  activeWorkflowId: Accessor<string | null>;
  settings: Accessor<AppSettings>;
  readiness: Accessor<ProviderReadiness | null>;
  activeProviderKeyInput: Accessor<string>;
  projectIdForActiveWorkflow: Accessor<string | null>;
  executionCwdForActiveWorkflow: Accessor<string | null>;
  applySchemaEditor: () => boolean;
  runState: Accessor<WorkflowRunState | null>;
  setBackendRunWorkflowId: Setter<string | null>;
  publishBackendRunState: (nextRunState: WorkflowRunState) => void;
  replaceBackendRunState: (nextRunState: WorkflowRunState) => void;
  clearStatusToast: () => void;
  showErrorToast: ToastHandler;
  setDockOpen: Setter<boolean>;
  setBottomTab: Setter<BottomTab>;
  setDockHeight: Setter<number>;
  uiZoom: Accessor<number>;
  isCompactViewport: Accessor<boolean>;
  cacheRunStateForWorkflow: (workflowId: string, state: WorkflowRunState) => void;
  runStateByWorkflowId: Record<string, WorkflowRunState>;
  applyRunStateSnapshot: (next: WorkflowRunState | null) => void;
  chatSubmissionFor: (nodeId: NodeId) => {
    submittedText: string;
    invokedSkills?: readonly string[];
  };
  resolveChatSubmissionPayload: (nodeId: NodeId) => Promise<UserMessageInput>;
  clearChatSubmission: (nodeId: NodeId) => void;
  handleRefreshRunHistoryRef: () => Promise<void>;
  updateActiveWorkflow: (mutator: (draft: Workflow) => void) => Workflow | null;
  updateChat: (chat: Chat) => void;
}

export function useRunSession(params: UseRunSessionParams) {
  const [selectedTraceIndex, setSelectedTraceIndex] = createSignal<number | null>(null);
  const [startingRun, setStartingRun] = createSignal(false);
  const [stoppingRun, setStoppingRun] = createSignal(false);
  const [continuableRunBackend, setContinuableRunBackend] = createSignal(false);
  const selectedRunId = () => params.runState()?.runId ?? null;
  const continuableRun = createMemo(
    () => continuableRunBackend() && selectedRunId() !== null,
  );
  const [runHistory, setRunHistory] = createSignal<RunSummary[]>([]);
  const [runHistoryLoading, setRunHistoryLoading] = createSignal(false);
  const [replayRunId, setReplayRunId] = createSignal<string | null>(null);

  const selectedTrace = createMemo(() => {
    const index = selectedTraceIndex();
    if (index === null) return null;
    return params.runState()?.runTrace[index] ?? null;
  });
  const hasRunTraceMemo = createMemo(() => (params.runState()?.runTrace.length ?? 0) > 0);

  const focusChatTab = () => {
    params.setDockOpen(true);
    params.setBottomTab("chat");
    params.setDockHeight((current) =>
      clampDockHeight(
        current,
        "chat",
        viewportHeight(),
        params.isCompactViewport(),
        params.uiZoom(),
      ),
    );
  };

  const beginRunSession = (nextRunState: WorkflowRunState) => {
    const workflowId = params.activeWorkflowId();
    if (workflowId) {
      params.setBackendRunWorkflowId(workflowId);
    }
    const addressedState = nextRunState.workflowId || !workflowId
      ? nextRunState
      : { ...nextRunState, workflowId };
    setReplayRunId(null);
    params.replaceBackendRunState(addressedState);
    setContinuableRunBackend(false);
    setSelectedTraceIndex(null);
    focusChatTab();
    params.clearStatusToast();
  };

  const beginSynchronizedRunSession = async (
    initialState: WorkflowRunState,
  ): Promise<WorkflowRunState> => {
    beginRunSession(initialState);
    if (!initialState.runId) {
      return initialState;
    }
    try {
      const liveState = await desktop.getRunState(initialState.runId);
      if (liveState?.runId === initialState.runId) {
        params.publishBackendRunState(liveState);
        return liveState;
      }
    } catch {
      // The run-state listener remains the live source if this reconciliation read fails.
    }
    return initialState;
  };

  const refreshContinuableRun = async () => {
    const runId = selectedRunId();
    if (!runId) {
      setContinuableRunBackend(false);
      return;
    }
    try {
      setContinuableRunBackend(await desktop.isRunContinuable(runId));
    } catch {
      setContinuableRunBackend(false);
    }
  };

  const runtimeUpdateForChat = (config: ChatConfig): NodeRuntimeConfigUpdate => ({
    model: config.model ?? activeProfile(params.settings()).default_model ?? undefined,
    approvalMode: config.approvalMode,
    reasoningEffort: config.reasoningEffort,
    reasoningBudgetTokens: config.reasoningBudgetTokens,
    fastMode: config.fastMode ?? false,
  });

  const startRunFromChat = (
    workflow: Workflow,
    message: UserMessageInput | null,
    invokedSkillIds: readonly string[],
  ) =>
    invokedSkillIds.length > 0
      ? desktop.startRun(
          workflow,
          params.settings(),
          params.projectIdForActiveWorkflow(),
          params.activeProviderKeyInput() || null,
          message,
          invokedSkillIds,
        )
      : desktop.startRun(
          workflow,
          params.settings(),
          params.projectIdForActiveWorkflow(),
          params.activeProviderKeyInput() || null,
          message,
        );

  const handleRun = async () => {
    const workflow = params.activeWorkflow();
    if (
      !workflow ||
      !params.applySchemaEditor() ||
      stoppingRun() ||
      startingRun() ||
      replayRunId()
    ) {
      return;
    }
    setStartingRun(true);
    try {
      const nextRunState = await desktop.startRun(
        workflow,
        params.settings(),
        params.projectIdForActiveWorkflow(),
        params.activeProviderKeyInput() || null,
        null,
      );
      await beginSynchronizedRunSession(nextRunState);
    } catch (error) {
      params.showErrorToast(normalizeError(error));
    } finally {
      setStartingRun(false);
    }
  };

  const resumeChatWithInput = async (
    chat: Chat,
    draftNodeId: NodeId,
    message: UserMessageInput,
    targetNodeId?: NodeId,
    invokedSkillIds: readonly string[] = [],
  ) => {
    if (!chat.runId) {
      throw new Error("Chat has no durable run to resume.");
    }
    const currentState = params.runState();
    const continuationNodeId =
      targetNodeId ??
      Object.keys(currentState?.statusByNode ?? {})[0] ??
      Object.keys(currentState?.chatLogs ?? {})[0];
    if (!continuationNodeId) {
      throw new Error("Chat run has no message target.");
    }
    const resumed = await desktop.resumeDurableRun(
      chat.runId,
      params.settings(),
      params.activeProviderKeyInput() || null,
      {
        nodeId: continuationNodeId,
        text: message.text,
        invokedSkillIds: [...invokedSkillIds],
        attachmentSourcePaths: [...message.attachmentSourcePaths],
      },
    );
    await beginSynchronizedRunSession(resumed);
    params.clearChatSubmission(draftNodeId);
  };

  const handleStartRunFromChat = async (nodeId: NodeId) => {
    const workflow = params.activeWorkflow();
    const chat = params.activeChat();
    if (
      (!workflow && !chat) ||
      !isGlobalRunEntryNodeId(nodeId) ||
      (!chat && !params.applySchemaEditor()) ||
      stoppingRun() ||
      startingRun()
    ) {
      return;
    }
    const submission = params.chatSubmissionFor(nodeId);
    const invokedSkillIds = submission.invokedSkills ?? [];
    const sendableText = submission.submittedText || (invokedSkillIds.length > 0 ? "/skill" : "");
    let message: UserMessageInput;
    try {
      message = await params.resolveChatSubmissionPayload(nodeId);
    } catch (error) {
      params.showErrorToast(normalizeError(error));
      return;
    }
    if (
      !canSendIdleRunKickoff(
        params.runState(),
        params.readiness()?.ready ?? false,
        true,
        startingRun(),
        sendableText,
        message.attachmentSourcePaths.length > 0,
        !!workflow && !chat,
      )
    ) {
      return;
    }
    setStartingRun(true);
    if (chat?.runId && params.runState()) {
      try {
        await resumeChatWithInput(chat, nodeId, message, undefined, invokedSkillIds);
      } catch (error) {
        params.showErrorToast(normalizeError(error));
      } finally {
        setStartingRun(false);
      }
      return;
    }
    try {
      let nextRunState: WorkflowRunState;
      if (chat) {
        const result =
          invokedSkillIds.length > 0
            ? await desktop.startChat(
                chat.id,
                params.settings(),
                params.activeProviderKeyInput() || null,
                message,
                invokedSkillIds,
              )
            : await desktop.startChat(
                chat.id,
                params.settings(),
                params.activeProviderKeyInput() || null,
                message,
              );
        params.updateChat(result.chat);
        nextRunState = result.runState;
      } else {
        const kickoffMessage =
          sendableText.trim() !== "" || message.attachmentSourcePaths.length > 0
            ? message
            : null;
        nextRunState = await startRunFromChat(workflow!, kickoffMessage, invokedSkillIds);
      }
      params.clearChatSubmission(nodeId);
      await beginSynchronizedRunSession(nextRunState);
    } catch (error) {
      params.showErrorToast(normalizeError(error));
    } finally {
      setStartingRun(false);
    }
  };

  const handleResumeChatFromInput = async (nodeId: NodeId) => {
    const chat = params.activeChat();
    if (!chat?.runId || stoppingRun() || startingRun()) {
      return;
    }
    let message: UserMessageInput;
    const submission = params.chatSubmissionFor(nodeId);
    try {
      message = await params.resolveChatSubmissionPayload(nodeId);
    } catch (error) {
      params.showErrorToast(normalizeError(error));
      return;
    }
    setStartingRun(true);
    try {
      await resumeChatWithInput(
        chat,
        nodeId,
        message,
        nodeId,
        submission.invokedSkills ?? [],
      );
    } catch (error) {
      params.showErrorToast(normalizeError(error));
    } finally {
      setStartingRun(false);
    }
  };

  const handleResumeReplayFromInput = async (draftNodeId: NodeId) => {
    const runId = replayRunId();
    const workflow = params.activeWorkflow();
    const targetNodeId = replayContinuationNodeId(workflow, params.runState());
    if (
      !runId ||
      !workflow ||
      !targetNodeId ||
      !params.applySchemaEditor() ||
      stoppingRun() ||
      startingRun()
    ) {
      return;
    }
    const submission = params.chatSubmissionFor(draftNodeId);
    let message: UserMessageInput;
    try {
      message = await params.resolveChatSubmissionPayload(draftNodeId);
    } catch (error) {
      params.showErrorToast(normalizeError(error));
      return;
    }
    setStartingRun(true);
    try {
      const resumed = await desktop.resumeDurableRun(
        runId,
        params.settings(),
        params.activeProviderKeyInput() || null,
        {
          nodeId: targetNodeId,
          text: message.text,
          invokedSkillIds: [...(submission.invokedSkills ?? [])],
          attachmentSourcePaths: [...message.attachmentSourcePaths],
        },
      );
      await beginSynchronizedRunSession(resumed);
      params.clearChatSubmission(draftNodeId);
      await params.handleRefreshRunHistoryRef();
    } catch (error) {
      params.showErrorToast(normalizeError(error));
    } finally {
      setStartingRun(false);
    }
  };

  const handleContinueRun = async () => {
    const workflow = params.activeWorkflow();
    if (!workflow || !continuableRun() || stoppingRun() || startingRun()) return;
    setStartingRun(true);
    try {
      const nextRunState = await desktop.continueRun(
        selectedRunId()!,
        workflow,
        params.settings(),
        params.activeProviderKeyInput() || null,
      );
      await beginSynchronizedRunSession(nextRunState);
    } catch (error) {
      params.showErrorToast(normalizeError(error));
    } finally {
      setStartingRun(false);
    }
  };

  const handleStopRun = async () => {
    const runId = selectedRunId();
    if (!runId || !params.runState()?.active || stoppingRun()) return;
    setStoppingRun(true);
    try {
      const nextRunState = await desktop.stopRun(runId);
      params.publishBackendRunState(nextRunState);
      await refreshContinuableRun();
      params.clearStatusToast();
    } catch (error) {
      params.showErrorToast(normalizeError(error));
    } finally {
      setStoppingRun(false);
    }
  };

  const handleInterruptNode = async (nodeId: NodeId) => {
    const runId = selectedRunId();
    if (!runId || !params.runState()?.active) return;
    try {
      await desktop.interruptNode(runId, nodeId);
    } catch (error) {
      params.showErrorToast(normalizeError(error));
    }
  };

  const handleRetryNode = async (nodeId: NodeId) => {
    const runId = selectedRunId();
    if (!runId || !params.runState()?.active) return;
    try {
      await desktop.retryNode(runId, nodeId);
    } catch (error) {
      params.showErrorToast(normalizeError(error));
    }
  };

  const handleClearRunTrace = async () => {
    const runId = selectedRunId();
    if (!runId) return;
    try {
      const nextRunState = await desktop.clearRunTrace(runId);
      if (nextRunState) params.publishBackendRunState(nextRunState);
      setContinuableRunBackend(false);
      setSelectedTraceIndex(null);
    } catch (error) {
      params.showErrorToast(normalizeError(error));
    }
  };

  const handleRefreshRunHistory = async () => {
    const workflow = params.activeWorkflow();
    if (!workflow) {
      setRunHistory([]);
      return;
    }
    setRunHistoryLoading(true);
    try {
      setRunHistory(await desktop.listRuns(workflow.id));
    } catch (error) {
      params.showErrorToast(normalizeError(error));
    } finally {
      setRunHistoryLoading(false);
    }
  };

  const handleReplayRun = async (runId: string) => {
    const workflow = params.activeWorkflow();
    if (!workflow) {
      return;
    }
    try {
      const replay = await desktop.replayRun(runId);
      const replayState: WorkflowRunState = { ...replay, active: false };
      setReplayRunId(runId);
      // Display-only: keep workflow cache so exit can restore live/idle.
      params.applyRunStateSnapshot(replayState);
      setContinuableRunBackend(false);
      focusChatTab();
    } catch (error) {
      params.showErrorToast(normalizeError(error));
    }
  };

  const handleExitReplay = async () => {
    if (!replayRunId()) {
      return;
    }
    const workflow = params.activeWorkflow();
    setReplayRunId(null);
    if (!workflow) {
      return;
    }
    const workflowId = workflow.id;
    const cachedRunId = params.runStateByWorkflowId[workflowId]?.runId;
    if (cachedRunId) {
      try {
        const live = await desktop.getRunState(cachedRunId);
        if (live && params.activeWorkflowId() === workflowId && !replayRunId()) {
          params.cacheRunStateForWorkflow(workflowId, live);
          params.applyRunStateSnapshot(live);
          await refreshContinuableRun();
          return;
        }
      } catch {
        // Fall through to idle.
      }
    }
    const idle = createIdleRunState(workflow);
    params.cacheRunStateForWorkflow(workflowId, idle);
    params.applyRunStateSnapshot(idle);
    setContinuableRunBackend(false);
  };

  const handleResumeDurableRun = async (runId: string) => {
    const workflow = params.activeWorkflow();
    if (!workflow || !params.applySchemaEditor() || startingRun() || stoppingRun()) {
      return;
    }
    setStartingRun(true);
    try {
      const nextRunState = await desktop.resumeDurableRun(
        runId,
        params.settings(),
        params.activeProviderKeyInput() || null,
      );
      setReplayRunId(null);
      await beginSynchronizedRunSession(nextRunState);
      await params.handleRefreshRunHistoryRef();
    } catch (error) {
      params.showErrorToast(normalizeError(error));
    } finally {
      setStartingRun(false);
    }
  };

  const searchProjectFileReferences = async (
    query: string,
  ): Promise<ProjectFileReference[]> => {
    const executionCwd = params.executionCwdForActiveWorkflow();
    if (!executionCwd) {
      return [];
    }
    return desktop.listProjectFileReferences(executionCwd, query, 30);
  };

  const handleToolApproval = async (approvalId: string, allow: boolean) => {
    const runId = selectedRunId();
    if (!runId) return;
    try {
      const nextRunState = await desktop.submitToolApproval(runId, approvalId, allow);
      params.publishBackendRunState(nextRunState);
    } catch (error) {
      params.showErrorToast(normalizeError(error));
    }
  };

  const handleMcpClientRequest = async (
    requestId: string,
    decision: import("../../lib/types").McpClientRequestDecision,
  ) => {
    const runId = selectedRunId();
    if (!runId) return;
    try {
      const nextRunState = await desktop.resolveMcpClientRequest(runId, requestId, decision);
      params.publishBackendRunState(nextRunState);
    } catch (error) {
      params.showErrorToast(normalizeError(error));
    }
  };

  const handleUpdateChatConfig = async (config: ChatConfig) => {
    const chat = params.activeChat();
    if (!chat) {
      return;
    }
    try {
      const updated = await desktop.updateChatConfig(chat.id, config);
      params.updateChat(updated);
      const state = params.runState();
      if (!state?.active) {
        return;
      }
      const nodeId =
        state.awaitingNodeIds?.[0] ??
        state.awaitingNodeId ??
        Object.keys(state.statusByNode)[0];
      if (!nodeId) {
        return;
      }
      const next = await desktop.updateNodeRuntimeConfig(
        state.runId!,
        nodeId,
        runtimeUpdateForChat(updated.config),
      );
      params.publishBackendRunState(next);
    } catch (error) {
      params.showErrorToast(normalizeError(error));
    }
  };

  const handleUpdateNodeRuntimeConfig = async (
    nodeId: NodeId,
    update: NodeRuntimeConfigUpdate,
  ) => {
    params.updateActiveWorkflow((draft) => {
      const node = draft.nodes.find((entry) => entry.id === nodeId);
      if (!node) {
        return;
      }
      if (update.approvalMode !== undefined) {
        node.agent.tools.approvalMode = update.approvalMode;
      }
      if (update.reasoningEffort !== undefined) {
        node.agent.reasoning_effort = update.reasoningEffort;
        node.agent.reasoningEffort = update.reasoningEffort;
        if (update.reasoningEffort === null) {
          node.agent.reasoning_budget_tokens = null;
          node.agent.reasoningBudgetTokens = null;
        }
      }
      if (update.reasoningBudgetTokens !== undefined) {
        node.agent.reasoning_budget_tokens = update.reasoningBudgetTokens;
        node.agent.reasoningBudgetTokens = update.reasoningBudgetTokens;
      }
    });
    const runId = selectedRunId();
    if (!runId || !params.runState()?.active) {
      return;
    }
    try {
      await desktop.updateNodeRuntimeConfig(runId, nodeId, update);
    } catch (error) {
      params.showErrorToast(normalizeError(error));
    }
  };

  return {
    selectedTraceIndex,
    setSelectedTraceIndex,
    selectedTrace,
    hasRunTraceMemo,
    startingRun,
    stoppingRun,
    continuableRun,
    setContinuableRunBackend,
    runHistory,
    runHistoryLoading,
    replayRunId,
    setReplayRunId,
    refreshContinuableRun,
    beginRunSession,
    handleRun,
    handleStartRunFromChat,
    handleResumeChatFromInput,
    handleResumeReplayFromInput,
    handleContinueRun,
    handleStopRun,
    handleInterruptNode,
    handleRetryNode,
    handleClearRunTrace,
    handleRefreshRunHistory,
    handleReplayRun,
    handleExitReplay,
    handleResumeDurableRun,
    searchProjectFileReferences,
    handleToolApproval,
    handleMcpClientRequest,
    handleUpdateChatConfig,
    handleUpdateNodeRuntimeConfig,
  };
}
