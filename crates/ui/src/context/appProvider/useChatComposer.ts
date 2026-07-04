import { createEffect, createMemo, createSignal, type Accessor } from "solid-js";
import { createStore } from "solid-js/store";
import * as desktop from "../../api";
import { resolveChatSubmission } from "../../lib/chatCommands";
import {
  extractReferencedFilePaths,
  formatSubmissionWithFileReferences,
} from "../../lib/fileReferences";
import type {
  NodeId,
  ProviderReadiness,
  SkillSummary,
  Workflow,
  WorkflowRunState,
} from "../../lib/types";
import {
  canSendChat,
  canSendIdleRunKickoff,
  chatNavigationForNode,
  isChatComposerBusy,
  isGlobalRunEntryNodeId,
  isLiveTranscriptSegment,
  projectChatLayout,
  statusForNode,
} from "../../lib/workflow";
import { normalizeError } from "../../lib/utils";

type ToastHandler = (message: string, context?: string) => void;

interface UseChatComposerParams {
  activeWorkflow: Accessor<Workflow | undefined>;
  activeWorkflowId: Accessor<string | null>;
  runState: Accessor<WorkflowRunState | null>;
  readiness: Accessor<ProviderReadiness | null>;
  startingRun: Accessor<boolean>;
  replayRunId: Accessor<string | null>;
  availableSkills: Accessor<SkillSummary[]>;
  executionCwdForActiveWorkflow: Accessor<string | null>;
  publishBackendRunState: (nextRunState: WorkflowRunState) => void;
  showErrorToast: ToastHandler;
}

export function useChatComposer(params: UseChatComposerParams) {
  const [chatDraftsByWorkflowId, setChatDraftsByWorkflowId] = createStore<
    Record<string, Record<string, string>>
  >({});
  const [chatFilterNodeId, setChatFilterNodeId] = createSignal<NodeId | null>(null);
  const [pickedLiveNodeId, setPickedLiveNodeId] = createSignal<NodeId | null>(null);
  const [chatSegmentOrder, setChatSegmentOrder] = createSignal<NodeId[]>([]);
  const [chatFocusNode, setChatFocusNode] = createSignal<{
    nodeId: NodeId;
    tick: number;
  } | null>(null);
  let chatFocusTick = 0;
  let pendingKickoffText: string | null = null;

  const [startRunFromChatHandler, setStartRunFromChatHandler] = createSignal<
    ((nodeId: NodeId) => Promise<void>) | null
  >(null);

  const chatLayout = createMemo(() =>
    projectChatLayout(
      params.activeWorkflow(),
      params.runState(),
      pickedLiveNodeId(),
      chatSegmentOrder(),
    ),
  );

  const chatDraft = (nodeId: NodeId) => {
    const workflowId = params.activeWorkflowId();
    if (!workflowId) {
      return "";
    }
    return chatDraftsByWorkflowId[workflowId]?.[nodeId] ?? "";
  };

  const setChatDraft = (nodeId: NodeId, text: string) => {
    const workflowId = params.activeWorkflowId();
    if (!workflowId) {
      return;
    }
    const existing = chatDraftsByWorkflowId[workflowId];
    if (existing) {
      setChatDraftsByWorkflowId(workflowId, nodeId, text);
      return;
    }
    setChatDraftsByWorkflowId(workflowId, { [nodeId]: text });
  };

  const skillIdsMemo = createMemo(
    () => new Set(params.availableSkills().map((skill) => skill.id)),
  );
  const chatSubmissionFor = (nodeId: NodeId) =>
    resolveChatSubmission(chatDraft(nodeId), skillIdsMemo());

  const resolveChatSubmittedText = async (nodeId: NodeId): Promise<string> => {
    const submission = chatSubmissionFor(nodeId);
    const paths = extractReferencedFilePaths(chatDraft(nodeId));
    return formatSubmissionWithFileReferences(submission.submittedText, paths);
  };

  const canSendChatFor = (nodeId: NodeId) => {
    if (params.replayRunId()) {
      return false;
    }
    if (isGlobalRunEntryNodeId(nodeId)) {
      return canSendIdleRunKickoff(
        params.runState(),
        params.readiness()?.ready ?? false,
        !!params.activeWorkflow(),
        params.startingRun(),
        chatSubmissionFor(nodeId).submittedText,
      );
    }
    return canSendChat(
      params.runState(),
      nodeId,
      params.readiness()?.ready ?? false,
      chatSubmissionFor(nodeId).submittedText,
    );
  };

  const composerBusyFor = (nodeId: NodeId) => isChatComposerBusy(params.runState(), nodeId);

  const setPendingKickoff = (text: string | null) => {
    pendingKickoffText = text;
  };

  const flushPendingKickoff = async (state: WorkflowRunState) => {
    const text = pendingKickoffText;
    if (!text || !state.active) {
      return;
    }
    const awaitingIds =
      state.awaitingNodeIds && state.awaitingNodeIds.length > 0
        ? state.awaitingNodeIds
        : state.awaitingNodeId
          ? [state.awaitingNodeId]
          : [];
    if (awaitingIds.length === 1) {
      pendingKickoffText = null;
      try {
        const next = await desktop.submitUserInput(awaitingIds[0], text);
        params.publishBackendRunState(next);
      } catch (error) {
        params.showErrorToast(normalizeError(error));
      }
      return;
    }
    if (awaitingIds.length === 0 && !state.active) {
      pendingKickoffText = null;
    }
    if (awaitingIds.length > 1) {
      pendingKickoffText = null;
    }
  };

  const focusChatNode = (nodeId: NodeId) => {
    chatFocusTick += 1;
    setChatFocusNode({ nodeId, tick: chatFocusTick });
  };

  const navigateChatToNode = (nodeId: NodeId) => {
    const nav = chatNavigationForNode(chatLayout(), nodeId);
    if (nav?.mode === "live") {
      setPickedLiveNodeId(nav.nodeId);
      setChatFilterNodeId(null);
    } else if (nav?.mode === "settled") {
      setChatFilterNodeId(nav.nodeId);
      setPickedLiveNodeId(null);
    }
    focusChatNode(nodeId);
  };

  const resetWorkflowChatUi = () => {
    setChatFilterNodeId(null);
    setPickedLiveNodeId(null);
    setChatSegmentOrder([]);
    setChatFocusNode(null);
  };

  const bindStartRunFromChat = (handler: (nodeId: NodeId) => Promise<void>) => {
    setStartRunFromChatHandler(() => handler);
  };

  const handleSubmitChat = async (nodeId: NodeId) => {
    if (!canSendChatFor(nodeId)) return;
    if (isGlobalRunEntryNodeId(nodeId)) {
      const handler = startRunFromChatHandler();
      if (handler) {
        await handler(nodeId);
      }
      return;
    }
    try {
      const submittedText = await resolveChatSubmittedText(nodeId);
      const nextRunState = await desktop.submitUserInput(nodeId, submittedText);
      params.publishBackendRunState(nextRunState);
      setChatDraft(nodeId, "");
    } catch (error) {
      params.showErrorToast(normalizeError(error));
    }
  };

  createEffect(() => {
    const state = params.runState();
    if (!state?.active) {
      setChatSegmentOrder([]);
      return;
    }
    const orderLayout = projectChatLayout(params.activeWorkflow(), state, null);
    const order = chatSegmentOrder();
    let next = order;
    for (const segment of orderLayout.settled) {
      if (!next.includes(segment.nodeId)) {
        next = [...next, segment.nodeId];
      }
    }
    for (const segment of orderLayout.live) {
      if (!next.includes(segment.nodeId)) {
        next = [...next, segment.nodeId];
      }
    }
    if (next.length !== order.length) {
      setChatSegmentOrder(next);
    }
  });

  createEffect(() => {
    const picked = pickedLiveNodeId();
    if (!picked) {
      return;
    }
    const state = params.runState();
    if (!state || !state.active) {
      setPickedLiveNodeId(null);
      return;
    }
    const status = statusForNode(state.statusByNode, picked);
    if (!isLiveTranscriptSegment(state, { status })) {
      setPickedLiveNodeId(null);
    }
  });

  return {
    chatLayout,
    chatDraft,
    setChatDraft,
    chatSubmissionFor,
    canSendChatFor,
    composerBusyFor,
    resolveChatSubmittedText,
    handleSubmitChat,
    setPendingKickoff,
    flushPendingKickoff,
    bindStartRunFromChat,
    chatFilterNodeId,
    setChatFilterNodeId,
    pickedLiveNodeId,
    setPickedLiveNodeId,
    chatSegmentOrder,
    chatFocusNode,
    focusChatNode,
    navigateChatToNode,
    resetWorkflowChatUi,
  };
}
