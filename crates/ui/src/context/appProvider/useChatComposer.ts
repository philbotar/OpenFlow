import { createEffect, createMemo, createSignal, type Accessor } from "solid-js";
import { createStore } from "solid-js/store";
import * as desktop from "../../api";
import { resolveChatSubmission } from "../../lib/chatCommands";
import {
  extractReferencedFilePaths,
  formatSubmissionWithFileReferences,
} from "../../lib/fileReferences";
import type {
  Chat,
  NodeId,
  PendingChatAttachment,
  ProviderReadiness,
  SkillSummary,
  UserMessageInput,
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
  isChatNavigatedToNode,
  projectChatLayout,
  replayContinuationNodeId,
  statusForNode,
} from "../../lib/workflow";
import { normalizeError } from "../../lib/utils";

type ToastHandler = (message: string, context?: string) => void;

const MAX_CHAT_ATTACHMENTS = 4;
const MAX_CHAT_ATTACHMENT_BYTES = 10 * 1024 * 1024;
const MAX_CHAT_ATTACHMENT_TOTAL_BYTES = 25 * 1024 * 1024;
const IMAGE_EXTENSIONS = new Set(["jpg", "jpeg", "png", "gif", "webp"]);

function fileNameFromSource(sourcePath: string): string {
  return sourcePath.split(/[\\/]/).pop() || "attachment";
}

function attachmentKind(fileName: string): PendingChatAttachment["kind"] {
  const extension = fileName.split(".").pop()?.toLowerCase() ?? "";
  return IMAGE_EXTENSIONS.has(extension) ? "image" : "document";
}

function bytesToBase64(bytes: Uint8Array): string {
  let binary = "";
  const chunkSize = 32_768;
  for (let offset = 0; offset < bytes.length; offset += chunkSize) {
    binary += String.fromCharCode(...bytes.subarray(offset, offset + chunkSize));
  }
  return btoa(binary);
}

export type ChatNavigateOptions = {
  /** Default true: scroll when navigation target changed. */
  scroll?: boolean;
  /** Canvas/explicit picks — scroll even when already on this node. */
  forceScroll?: boolean;
};

interface UseChatComposerParams {
  activeWorkflow: Accessor<Workflow | undefined>;
  activeChat: Accessor<Chat | null>;
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
  const [chatAttachmentsByWorkflowId, setChatAttachmentsByWorkflowId] = createStore<
    Record<string, Record<string, PendingChatAttachment[]>>
  >({});
  const [chatFilterNodeId, setChatFilterNodeId] = createSignal<NodeId | null>(null);
  const [pickedLiveNodeId, setPickedLiveNodeId] = createSignal<NodeId | null>(null);
  const [chatSegmentOrder, setChatSegmentOrder] = createSignal<NodeId[]>([]);
  const [chatFocusNode, setChatFocusNode] = createSignal<{
    nodeId: NodeId;
    tick: number;
  } | null>(null);
  let chatFocusTick = 0;

  const [startRunFromChatHandler, setStartRunFromChatHandler] = createSignal<
    ((nodeId: NodeId) => Promise<void>) | null
  >(null);
  const [resumeChatFromInputHandler, setResumeChatFromInputHandler] = createSignal<
    ((nodeId: NodeId) => Promise<void>) | null
  >(null);
  const [resumeReplayFromInputHandler, setResumeReplayFromInputHandler] = createSignal<
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

  const pendingChatAttachments = (nodeId: NodeId): PendingChatAttachment[] => {
    const workflowId = params.activeWorkflowId();
    if (!workflowId) {
      return [];
    }
    return chatAttachmentsByWorkflowId[workflowId]?.[nodeId] ?? [];
  };

  const setPendingChatAttachments = (
    workflowId: string,
    nodeId: NodeId,
    attachments: PendingChatAttachment[],
  ) => {
    const existing = chatAttachmentsByWorkflowId[workflowId];
    if (existing) {
      setChatAttachmentsByWorkflowId(workflowId, nodeId, attachments);
      return;
    }
    setChatAttachmentsByWorkflowId(workflowId, { [nodeId]: attachments });
  };

  const addPendingAttachments = (
    workflowId: string,
    nodeId: NodeId,
    attachments: PendingChatAttachment[],
  ): PendingChatAttachment[] => {
    if (params.activeWorkflowId() !== workflowId) {
      return [];
    }
    const current = chatAttachmentsByWorkflowId[workflowId]?.[nodeId] ?? [];
    const seen = new Set(current.map((attachment) => attachment.sourcePath));
    const accepted: PendingChatAttachment[] = [];
    let uniqueCandidateCount = 0;
    for (const attachment of attachments) {
      if (seen.has(attachment.sourcePath)) {
        continue;
      }
      uniqueCandidateCount += 1;
      if (current.length + accepted.length >= MAX_CHAT_ATTACHMENTS) {
        continue;
      }
      seen.add(attachment.sourcePath);
      accepted.push(attachment);
    }
    if (accepted.length < uniqueCandidateCount) {
      params.showErrorToast(`Attach up to ${MAX_CHAT_ATTACHMENTS} files per message.`);
    }
    setPendingChatAttachments(workflowId, nodeId, [...current, ...accepted]);
    return accepted;
  };

  const pickChatAttachments = async (nodeId: NodeId) => {
    const workflowId = params.activeWorkflowId();
    if (!workflowId || params.replayRunId()) {
      return;
    }
    try {
      const sources = await desktop.pickChatAttachmentSources();
      addPendingAttachments(
        workflowId,
        nodeId,
        sources.map((sourcePath) => {
          const fileName = fileNameFromSource(sourcePath);
          return {
            sourcePath,
            fileName,
            kind: attachmentKind(fileName),
          };
        }),
      );
    } catch (error) {
      params.showErrorToast(normalizeError(error));
    }
  };

  const stageChatAttachments = async (nodeId: NodeId, files: readonly File[]) => {
    const workflowId = params.activeWorkflowId();
    if (!workflowId || params.replayRunId()) {
      return;
    }
    const current = pendingChatAttachments(nodeId);
    const slots = Math.max(0, MAX_CHAT_ATTACHMENTS - current.length);
    const candidates = files.slice(0, slots);
    if (candidates.length < files.length) {
      params.showErrorToast(`Attach up to ${MAX_CHAT_ATTACHMENTS} files per message.`);
    }
    const currentTotal = current.reduce(
      (total, attachment) => total + (attachment.sizeBytes ?? 0),
      0,
    );
    let stagedTotal = 0;
    for (const file of candidates) {
      if (
        file.size > MAX_CHAT_ATTACHMENT_BYTES ||
        currentTotal + stagedTotal + file.size > MAX_CHAT_ATTACHMENT_TOTAL_BYTES
      ) {
        params.showErrorToast(`${file.name} exceeds the attachment size limit.`);
        continue;
      }
      try {
        const dataBase64 = bytesToBase64(new Uint8Array(await file.arrayBuffer()));
        const staged = await desktop.stageChatAttachment(file.name, file.type, dataBase64);
        if (params.activeWorkflowId() !== workflowId) {
          await desktop.removeStagedChatAttachment(staged.token);
          continue;
        }
        const attachment: PendingChatAttachment = {
          sourcePath: staged.token,
          fileName: staged.fileName,
          sizeBytes: staged.sizeBytes,
          kind: staged.kind,
          staged: true,
        };
        const accepted = addPendingAttachments(workflowId, nodeId, [attachment]);
        if (accepted.length === 0) {
          await desktop.removeStagedChatAttachment(staged.token);
        } else {
          stagedTotal += staged.sizeBytes;
        }
      } catch (error) {
        params.showErrorToast(normalizeError(error));
      }
    }
  };

  const removePendingChatAttachment = async (nodeId: NodeId, sourcePath: string) => {
    const workflowId = params.activeWorkflowId();
    if (!workflowId) {
      return;
    }
    const current = pendingChatAttachments(nodeId);
    const removed = current.find((attachment) => attachment.sourcePath === sourcePath);
    setPendingChatAttachments(
      workflowId,
      nodeId,
      current.filter((attachment) => attachment.sourcePath !== sourcePath),
    );
    if (removed?.staged) {
      try {
        await desktop.removeStagedChatAttachment(removed.sourcePath);
      } catch (error) {
        params.showErrorToast(normalizeError(error));
      }
    }
  };

  const clearChatSubmission = (nodeId: NodeId) => {
    const workflowId = params.activeWorkflowId();
    setChatDraft(nodeId, "");
    if (workflowId) {
      const staged = pendingChatAttachments(nodeId).filter(
        (attachment) => attachment.staged,
      );
      setPendingChatAttachments(workflowId, nodeId, []);
      for (const attachment of staged) {
        void desktop.removeStagedChatAttachment(attachment.sourcePath).catch((error) => {
          params.showErrorToast(normalizeError(error));
        });
      }
    }
  };

  const skillIdsMemo = createMemo(
    () => new Set(params.availableSkills().map((skill) => skill.id)),
  );
  const chatSubmissionFor = (nodeId: NodeId) =>
    resolveChatSubmission(chatDraft(nodeId), skillIdsMemo());

  const sendableSubmissionText = (nodeId: NodeId) => {
    const submission = chatSubmissionFor(nodeId);
    return submission.submittedText || (submission.invokedSkills.length > 0 ? "/skill" : "");
  };

  const submitUserInput = (
    nodeId: NodeId,
    message: UserMessageInput,
    invokedSkills: readonly string[],
  ) => {
    const runId = params.runState()?.runId;
    if (!runId) {
      return Promise.reject(new Error("Run id missing for chat input."));
    }
    return invokedSkills.length > 0
      ? desktop.submitUserInput(runId, nodeId, message, invokedSkills)
      : desktop.submitUserInput(runId, nodeId, message);
  };

  const resolveChatSubmissionPayload = async (nodeId: NodeId): Promise<UserMessageInput> => {
    const submission = chatSubmissionFor(nodeId);
    const paths = extractReferencedFilePaths(chatDraft(nodeId));
    return {
      text: await formatSubmissionWithFileReferences(submission.submittedText, paths),
      attachmentSourcePaths: pendingChatAttachments(nodeId).map(
        (attachment) => attachment.sourcePath,
      ),
    };
  };

  const canSendChatFor = (nodeId: NodeId) => {
    if (params.replayRunId()) {
      return (
        isGlobalRunEntryNodeId(nodeId) &&
        replayContinuationNodeId(params.activeWorkflow(), params.runState()) !== null &&
        canSendIdleRunKickoff(
          params.runState(),
          params.readiness()?.ready ?? false,
          Boolean(params.activeWorkflow()),
          params.startingRun(),
          sendableSubmissionText(nodeId),
          false,
        )
      );
    }
    if (isGlobalRunEntryNodeId(nodeId)) {
      const activeWorkflow = params.activeWorkflow();
      const activeChat = params.activeChat();
      return canSendIdleRunKickoff(
        params.runState(),
        params.readiness()?.ready ?? false,
        !!activeWorkflow || !!activeChat,
        params.startingRun(),
        sendableSubmissionText(nodeId),
        pendingChatAttachments(nodeId).length > 0,
        !!activeWorkflow && !activeChat,
      );
    }
    return canSendChat(
      params.runState(),
      nodeId,
      params.readiness()?.ready ?? false,
      sendableSubmissionText(nodeId),
      pendingChatAttachments(nodeId).length > 0,
    );
  };

  const composerBusyFor = (nodeId: NodeId) => isChatComposerBusy(params.runState(), nodeId);

  const focusChatNode = (nodeId: NodeId) => {
    chatFocusTick += 1;
    setChatFocusNode({ nodeId, tick: chatFocusTick });
  };

  const isChatNavigatedToNodeFor = (nodeId: NodeId) =>
    isChatNavigatedToNode(
      chatLayout(),
      nodeId,
      chatFilterNodeId(),
      pickedLiveNodeId(),
    );

  const navigateChatToNode = (nodeId: NodeId, options?: ChatNavigateOptions) => {
    const alreadyThere = isChatNavigatedToNodeFor(nodeId);
    const nav = chatNavigationForNode(chatLayout(), nodeId);
    if (nav?.mode === "live") {
      setPickedLiveNodeId(nav.nodeId);
      setChatFilterNodeId(null);
    } else if (nav?.mode === "settled") {
      setChatFilterNodeId(nodeId);
      setPickedLiveNodeId(null);
    }
    const shouldScroll = options?.scroll !== false;
    const forceScroll = options?.forceScroll === true;
    if (shouldScroll && (forceScroll || !alreadyThere)) {
      focusChatNode(nodeId);
    }
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

  const bindResumeChatFromInput = (handler: (nodeId: NodeId) => Promise<void>) => {
    setResumeChatFromInputHandler(() => handler);
  };

  const bindResumeReplayFromInput = (handler: (nodeId: NodeId) => Promise<void>) => {
    setResumeReplayFromInputHandler(() => handler);
  };

  const handleSubmitChat = async (nodeId: NodeId) => {
    if (!canSendChatFor(nodeId)) return;
    if (params.replayRunId()) {
      const handler = resumeReplayFromInputHandler();
      if (handler) {
        await handler(nodeId);
      }
      return;
    }
    if (isGlobalRunEntryNodeId(nodeId)) {
      const handler = startRunFromChatHandler();
      if (handler) {
        await handler(nodeId);
      }
      return;
    }
    if (
      params.activeChat()?.runId &&
      params.runState()?.active !== true
    ) {
      const handler = resumeChatFromInputHandler();
      if (handler) {
        await handler(nodeId);
      }
      return;
    }
    try {
      const submission = chatSubmissionFor(nodeId);
      const message = await resolveChatSubmissionPayload(nodeId);
      const nextRunState = await submitUserInput(nodeId, message, submission.invokedSkills);
      params.publishBackendRunState(nextRunState);
      clearChatSubmission(nodeId);
    } catch (error) {
      params.showErrorToast(normalizeError(error));
    }
  };

  const handleSubmitStructuredInput = async (nodeId: NodeId, text: string) => {
    if (
      params.replayRunId() ||
      !canSendChat(
        params.runState(),
        nodeId,
        params.readiness()?.ready ?? false,
        text,
      )
    ) {
      return;
    }
    try {
      const runId = params.runState()?.runId;
      if (!runId) {
        throw new Error("Run id missing for structured input.");
      }
      const nextRunState = await desktop.submitUserInput(runId, nodeId, {
        text,
        attachmentSourcePaths: [],
      });
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
    pendingChatAttachments,
    pickChatAttachments,
    stageChatAttachments,
    removePendingChatAttachment,
    clearChatSubmission,
    chatSubmissionFor,
    canSendChatFor,
    composerBusyFor,
    resolveChatSubmissionPayload,
    handleSubmitChat,
    handleSubmitStructuredInput,
    bindStartRunFromChat,
    bindResumeChatFromInput,
    bindResumeReplayFromInput,
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
