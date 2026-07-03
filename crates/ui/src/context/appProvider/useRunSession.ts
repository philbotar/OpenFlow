import { createMemo, createSignal, type Accessor, type Setter } from "solid-js";
import * as desktop from "../../api";
import type {
  AppSettings,
  BottomTab,
  NodeId,
  ProjectFileReference,
  ProviderReadiness,
  RunSummary,
  Workflow,
  WorkflowRunState,
} from "../../lib/types";
import { canSendIdleRunKickoff, isGlobalRunEntryNodeId } from "../../lib/workflow";
import { clampDockHeight, normalizeError } from "../../lib/utils";

type ToastHandler = (message: string, context?: string) => void;

interface UseRunSessionParams {
  activeWorkflow: Accessor<Workflow | undefined>;
  activeWorkflowId: Accessor<string | null>;
  settings: Accessor<AppSettings>;
  readiness: Accessor<ProviderReadiness | null>;
  activeProviderKeyInput: Accessor<string>;
  executionCwdForActiveWorkflow: Accessor<string | null>;
  applySchemaEditor: () => boolean;
  runState: Accessor<WorkflowRunState | null>;
  backendRunWorkflowId: Accessor<string | null>;
  setBackendRunWorkflowId: Setter<string | null>;
  publishBackendRunState: (nextRunState: WorkflowRunState) => void;
  clearStatusToast: () => void;
  showErrorToast: ToastHandler;
  setDockOpen: Setter<boolean>;
  setBottomTab: Setter<BottomTab>;
  setDockHeight: Setter<number>;
  cacheRunStateForWorkflow: (workflowId: string, state: WorkflowRunState) => void;
  applyRunStateSnapshot: (next: WorkflowRunState | null) => void;
  chatSubmissionFor: (nodeId: NodeId) => { submittedText: string };
  resolveChatSubmittedText: (nodeId: NodeId) => Promise<string>;
  setChatDraft: (nodeId: NodeId, text: string) => void;
  setPendingKickoff: (text: string | null) => void;
  flushPendingKickoff: (state: WorkflowRunState) => Promise<void>;
  handleRefreshRunHistoryRef: () => Promise<void>;
}

export function useRunSession(params: UseRunSessionParams) {
  const [selectedTraceIndex, setSelectedTraceIndex] = createSignal<number | null>(null);
  const [startingRun, setStartingRun] = createSignal(false);
  const [stoppingRun, setStoppingRun] = createSignal(false);
  const [continuableRunBackend, setContinuableRunBackend] = createSignal(false);
  const continuableRun = createMemo(
    () => continuableRunBackend() && params.backendRunWorkflowId() === params.activeWorkflowId(),
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
    params.setDockHeight((current) => clampDockHeight(current, "chat"));
  };

  const beginRunSession = (nextRunState: WorkflowRunState) => {
    const workflowId = params.activeWorkflowId();
    if (workflowId) {
      params.setBackendRunWorkflowId(workflowId);
    }
    setReplayRunId(null);
    params.publishBackendRunState(nextRunState);
    setContinuableRunBackend(false);
    setSelectedTraceIndex(null);
    focusChatTab();
    params.clearStatusToast();
  };

  const refreshContinuableRun = async () => {
    try {
      setContinuableRunBackend(await desktop.isRunContinuable());
    } catch {
      setContinuableRunBackend(false);
    }
  };

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
        params.executionCwdForActiveWorkflow(),
        params.activeProviderKeyInput() || null,
        null,
      );
      beginRunSession(nextRunState);
    } catch (error) {
      params.showErrorToast(normalizeError(error));
    } finally {
      setStartingRun(false);
    }
  };

  const handleStartRunFromChat = async (nodeId: NodeId) => {
    const workflow = params.activeWorkflow();
    if (
      !workflow ||
      !isGlobalRunEntryNodeId(nodeId) ||
      !params.applySchemaEditor() ||
      stoppingRun() ||
      startingRun()
    ) {
      return;
    }
    const submission = params.chatSubmissionFor(nodeId);
    let submittedText = submission.submittedText;
    if (
      !canSendIdleRunKickoff(
        params.runState(),
        params.readiness()?.ready ?? false,
        true,
        startingRun(),
        submission.submittedText,
      )
    ) {
      return;
    }
    try {
      submittedText = await params.resolveChatSubmittedText(nodeId);
    } catch (error) {
      params.showErrorToast(normalizeError(error));
      return;
    }
    setStartingRun(true);
    params.setPendingKickoff(submittedText);
    try {
      const nextRunState = await desktop.startRun(
        workflow,
        params.settings(),
        params.executionCwdForActiveWorkflow(),
        params.activeProviderKeyInput() || null,
        submittedText,
      );
      params.setChatDraft(nodeId, "");
      beginRunSession(nextRunState);
      await params.flushPendingKickoff(nextRunState);
    } catch (error) {
      params.setPendingKickoff(null);
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
        workflow,
        params.settings(),
        params.activeProviderKeyInput() || null,
      );
      beginRunSession(nextRunState);
    } catch (error) {
      params.showErrorToast(normalizeError(error));
    } finally {
      setStartingRun(false);
    }
  };

  const handleStopRun = async () => {
    if (!params.runState()?.active || stoppingRun()) return;
    setStoppingRun(true);
    params.setPendingKickoff(null);
    try {
      const nextRunState = await desktop.stopRun();
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
    if (!params.runState()?.active) return;
    try {
      await desktop.interruptNode(nodeId);
    } catch (error) {
      params.showErrorToast(normalizeError(error));
    }
  };

  const handleRetryNode = async (nodeId: NodeId) => {
    if (!params.runState()?.active) return;
    try {
      await desktop.retryNode(nodeId);
    } catch (error) {
      params.showErrorToast(normalizeError(error));
    }
  };

  const handleClearRunTrace = async () => {
    try {
      const nextRunState = await desktop.clearRunTrace();
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
      params.cacheRunStateForWorkflow(workflow.id, replayState);
      params.applyRunStateSnapshot(replayState);
      setContinuableRunBackend(false);
      focusChatTab();
    } catch (error) {
      params.showErrorToast(normalizeError(error));
    }
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
      beginRunSession(nextRunState);
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
    try {
      const nextRunState = await desktop.submitToolApproval(approvalId, allow);
      params.publishBackendRunState(nextRunState);
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
    handleContinueRun,
    handleStopRun,
    handleInterruptNode,
    handleRetryNode,
    handleClearRunTrace,
    handleRefreshRunHistory,
    handleReplayRun,
    handleResumeDurableRun,
    searchProjectFileReferences,
    handleToolApproval,
  };
}
