import { createEffect, createMemo, createResource, createSignal, For, Show } from "solid-js";
import ArrowUp from "lucide-solid/icons/arrow-up";
import FileText from "lucide-solid/icons/file-text";
import Paperclip from "lucide-solid/icons/paperclip";
import Square from "lucide-solid/icons/square";
import X from "lucide-solid/icons/x";
import { useAppContext } from "../../context/AppContext";
import {
  applySlashTokenCompletion,
  getActiveSlashToken,
  matchSkillsForSlashQuery,
} from "../../lib/chatCommands";
import {
  applyFileReferenceCompletion,
  getActiveFileReferenceToken,
} from "../../lib/fileReferences";
import { createDebounced } from "../../lib/utils";
import type { NodeId, ProjectFileReference, SkillSummary } from "../../lib/types";
import { pendingApprovalForNode, replayContinuationNodeId } from "../../lib/workflow";
import { ComposerInput } from "./ComposerInput";
import { ComposerRuntimeControls } from "./ComposerRuntimeControls";
import { FileReferenceCombobox } from "./FileReferenceCombobox";
import { SkillCommandCombobox } from "./SkillCommandCombobox";
import { SkillDescriptionPreview } from "./SkillDescriptionPreview";
import { Button } from "../Button";
import { Tooltip } from "../Tooltip";
import { ChatRuntimeControls } from "./ChatRuntimeControls";

export function ConversationComposer(props: {
  nodeId: NodeId;
  label: string;
  disabled?: boolean;
  kickoff?: boolean;
  directChat?: boolean;
  onMessageSubmit?: () => void;
}) {
  const ctx = useAppContext();
  let textareaRef: HTMLTextAreaElement | undefined;
  const [caretPosition, setCaretPosition] = createSignal(0);
  const [highlightedIndex, setHighlightedIndex] = createSignal(0);
  const [highlightedFileIndex, setHighlightedFileIndex] = createSignal(0);
  const [dragDepth, setDragDepth] = createSignal(0);
  const [attachmentAnnouncement, setAttachmentAnnouncement] = createSignal("");
  const listboxId = () => `chat-skill-command-listbox-${props.nodeId}`;
  const fileListboxId = () => `chat-file-reference-listbox-${props.nodeId}`;

  const draft = () => ctx.chatDraft(props.nodeId);
  const knownSkillIds = createMemo(
    () => new Set(ctx.availableSkills().map((skill) => skill.id)),
  );
  const pendingApproval = () => pendingApprovalForNode(ctx.runState(), props.nodeId);
  const inputEnabled = () => {
    if (props.disabled) {
      return false;
    }
    if (pendingApproval()) {
      return false;
    }
    if (!(ctx.readiness()?.ready ?? false)) {
      return false;
    }
    if (ctx.replayRunId()) {
      return (
        Boolean(props.kickoff) &&
        !ctx.startingRun() &&
        replayContinuationNodeId(ctx.activeWorkflow(), ctx.runState()) !== null
      );
    }
    if (props.directChat && ctx.runState()?.active === true) {
      return true;
    }
    if (props.kickoff) {
      return !ctx.runState()?.active;
    }
    return ctx.runState()?.active === true;
  };
  const attachmentsEnabled = () => inputEnabled() && !ctx.replayRunId();

  const activeSlashToken = createMemo(() =>
    getActiveSlashToken(draft(), caretPosition()),
  );
  const suggestions = createMemo(() => {
    const token = activeSlashToken();
    if (!token) {
      return [];
    }
    return matchSkillsForSlashQuery(ctx.availableSkills(), token.query);
  });
  const comboboxOpen = createMemo(
    () => !!activeSlashToken() && suggestions().length > 0 && inputEnabled(),
  );

  const activeFileToken = createMemo(() =>
    getActiveFileReferenceToken(draft(), caretPosition()),
  );
  const fileQuery = createMemo(() =>
    inputEnabled() && activeFileToken() ? activeFileToken()!.query : null,
  );
  const debouncedFileQuery = createDebounced(fileQuery, 150);
  const [fileSuggestions] = createResource(debouncedFileQuery, async (query) => {
    if (query === null) {
      return [] as ProjectFileReference[];
    }
    return ctx.searchProjectFileReferences(query);
  });
  const fileComboboxOpen = createMemo(
    () => !!activeFileToken() && inputEnabled() && !comboboxOpen(),
  );

  createEffect(() => {
    activeFileToken();
    setHighlightedFileIndex(0);
  });

  const syncCaret = (target: HTMLTextAreaElement) => {
    setCaretPosition(target.selectionStart ?? target.value.length);
  };

  const applySkill = (skill: SkillSummary) => {
    const token = activeSlashToken();
    if (!token) {
      return;
    }

    const { value, caret } = applySlashTokenCompletion(
      draft(),
      token.replaceStart,
      token.replaceEnd,
      skill.id,
    );
    ctx.setChatDraft(props.nodeId, value);
    setHighlightedIndex(0);
    requestAnimationFrame(() => {
      if (!textareaRef) {
        return;
      }
      textareaRef.focus();
      textareaRef.setSelectionRange(caret, caret);
      setCaretPosition(caret);
    });
  };

  const applyFileReference = (reference: ProjectFileReference) => {
    const token = activeFileToken();
    if (!token) {
      return;
    }

    const { value, caret } = applyFileReferenceCompletion(
      draft(),
      token.replaceStart,
      token.replaceEnd,
      reference.path,
    );
    ctx.setChatDraft(props.nodeId, value);
    setHighlightedFileIndex(0);
    requestAnimationFrame(() => {
      if (!textareaRef) {
        return;
      }
      textareaRef.focus();
      textareaRef.setSelectionRange(caret, caret);
      setCaretPosition(caret);
    });
  };

  const handleInput = (event: InputEvent & { currentTarget: HTMLTextAreaElement }) => {
    ctx.setChatDraft(props.nodeId, event.currentTarget.value);
    syncCaret(event.currentTarget);
    setHighlightedIndex(0);
    setHighlightedFileIndex(0);
  };

  const stageFiles = async (files: readonly File[]) => {
    if (!attachmentsEnabled() || files.length === 0) {
      return;
    }
    const before = ctx.pendingChatAttachments(props.nodeId).length;
    await ctx.handleStageChatAttachments(props.nodeId, files);
    const added = ctx.pendingChatAttachments(props.nodeId).length - before;
    setAttachmentAnnouncement(
      added > 0
        ? `Added ${added} attachment${added === 1 ? "" : "s"}.`
        : "No attachments added.",
    );
  };

  const handlePaste = (event: ClipboardEvent) => {
    const files = Array.from(event.clipboardData?.items ?? [])
      .filter((item) => item.kind === "file" && item.type.startsWith("image/"))
      .map((item) => item.getAsFile())
      .filter((file): file is File => file !== null);
    if (files.length === 0) {
      return;
    }
    event.preventDefault();
    void stageFiles(files);
  };

  const handleDrop = (event: DragEvent) => {
    event.preventDefault();
    setDragDepth(0);
    if (!attachmentsEnabled()) {
      return;
    }
    void stageFiles(Array.from(event.dataTransfer?.files ?? []));
  };

  const handleKeyDown = (event: KeyboardEvent) => {
    if (comboboxOpen()) {
      const items = suggestions();
      if (event.key === "ArrowDown") {
        event.preventDefault();
        setHighlightedIndex((current) => (current + 1) % items.length);
        return;
      }
      if (event.key === "ArrowUp") {
        event.preventDefault();
        setHighlightedIndex((current) => (current - 1 + items.length) % items.length);
        return;
      }
      if (event.key === "Enter" || event.key === "Tab") {
        const selected = items[highlightedIndex()];
        if (selected) {
          event.preventDefault();
          applySkill(selected);
        }
        return;
      }
      if (event.key === "Escape") {
        event.preventDefault();
        const token = activeSlashToken();
        if (token) {
          ctx.setChatDraft(
            props.nodeId,
            `${draft().slice(0, token.replaceStart)}${draft().slice(token.replaceEnd)}`,
          );
          const caret = token.replaceStart;
          requestAnimationFrame(() => {
            textareaRef?.setSelectionRange(caret, caret);
            setCaretPosition(caret);
          });
        }
        setHighlightedIndex(0);
        return;
      }
    }

    if (fileComboboxOpen()) {
      const items = fileSuggestions() ?? [];
      if (event.key === "ArrowDown" && items.length > 0) {
        event.preventDefault();
        setHighlightedFileIndex((current) => (current + 1) % items.length);
        return;
      }
      if (event.key === "ArrowUp" && items.length > 0) {
        event.preventDefault();
        setHighlightedFileIndex((current) => (current - 1 + items.length) % items.length);
        return;
      }
      if ((event.key === "Enter" || event.key === "Tab") && items.length > 0) {
        const selected = items[highlightedFileIndex()];
        if (selected) {
          event.preventDefault();
          applyFileReference(selected);
        }
        return;
      }
      if (event.key === "Escape") {
        event.preventDefault();
        const token = activeFileToken();
        if (token) {
          ctx.setChatDraft(
            props.nodeId,
            `${draft().slice(0, token.replaceStart)}${draft().slice(token.replaceEnd)}`,
          );
          const caret = token.replaceStart;
          requestAnimationFrame(() => {
            textareaRef?.setSelectionRange(caret, caret);
            setCaretPosition(caret);
          });
        }
        setHighlightedFileIndex(0);
        return;
      }
    }

    if (
      event.key === "Enter" &&
      !event.shiftKey &&
      ctx.canSendChatFor(props.nodeId)
    ) {
      props.onMessageSubmit?.();
    }
    ctx.handleChatInputKeyDown(event, props.nodeId);
  };

  const handleSubmit = () => {
    if (!ctx.canSendChatFor(props.nodeId)) {
      return;
    }
    props.onMessageSubmit?.();
    void ctx.handleSubmitChat(props.nodeId);
  };

  return (
    <div class="chat-composer">
      <SkillDescriptionPreview
        skillIds={ctx.chatSubmissionFor(props.nodeId).invokedSkills}
        skillById={ctx.skillById()}
      />
      <div class="chat-composer-input-shell">
        <SkillCommandCombobox
          open={comboboxOpen()}
          suggestions={suggestions()}
          highlightedIndex={highlightedIndex()}
          query={activeSlashToken()?.query ?? ""}
          listboxId={listboxId()}
          onSelect={applySkill}
          onHighlight={setHighlightedIndex}
        />
        <FileReferenceCombobox
          open={fileComboboxOpen()}
          suggestions={fileSuggestions() ?? []}
          highlightedIndex={highlightedFileIndex()}
          query={activeFileToken()?.query ?? ""}
          listboxId={fileListboxId()}
          loading={fileSuggestions.loading}
          onSelect={applyFileReference}
          onHighlight={setHighlightedFileIndex}
        />
        <div
          class="chat-composer-pill"
          classList={{
            "is-busy": ctx.composerBusyFor(props.nodeId),
            "is-dragging-attachments": dragDepth() > 0 && attachmentsEnabled(),
          }}
          onDragEnter={(event) => {
            event.preventDefault();
            if (attachmentsEnabled()) setDragDepth((depth) => depth + 1);
          }}
          onDragLeave={(event) => {
            event.preventDefault();
            setDragDepth((depth) => Math.max(0, depth - 1));
          }}
          onDragOver={(event) => {
            if (attachmentsEnabled()) event.preventDefault();
          }}
          onDrop={handleDrop}
        >
          <Show when={dragDepth() > 0 && attachmentsEnabled()}>
            <div class="composer-drop-indicator" role="status">
              Drop files to attach
            </div>
          </Show>
          <Show when={ctx.pendingChatAttachments(props.nodeId).length > 0}>
            <div class="composer-attachment-list" aria-label="Pending attachments">
              <For each={ctx.pendingChatAttachments(props.nodeId)}>
                {(attachment) => (
                  <div class="composer-attachment-card">
                    <FileText aria-hidden="true" width={16} height={16} />
                    <span class="composer-attachment-name">{attachment.fileName}</span>
                    <Show when={attachment.sizeBytes !== undefined}>
                      <span class="composer-attachment-size">
                        {Math.max(1, Math.ceil((attachment.sizeBytes ?? 0) / 1024))} KB
                      </span>
                    </Show>
                    <button
                      type="button"
                      class="composer-attachment-remove"
                      aria-label={`Remove ${attachment.fileName}`}
                      onClick={async () => {
                        await ctx.handleRemovePendingChatAttachment(
                          props.nodeId,
                          attachment.sourcePath,
                        );
                        textareaRef?.focus();
                      }}
                    >
                      <X aria-hidden="true" width={14} height={14} />
                    </button>
                  </div>
                )}
              </For>
            </div>
          </Show>
          <div class="chat-composer-main">
            <div class="composer-attach-control">
              <Tooltip label="Attach files">
                <Button
                  ghost
                  size="compact"
                  class="composer-attach-button"
                  aria-label="Attach files"
                  disabled={!attachmentsEnabled()}
                  onClick={async () => {
                    const before = ctx.pendingChatAttachments(props.nodeId).length;
                    await ctx.handlePickChatAttachments(props.nodeId);
                    const added = ctx.pendingChatAttachments(props.nodeId).length - before;
                    if (added > 0) {
                      setAttachmentAnnouncement(
                        `Added ${added} attachment${added === 1 ? "" : "s"}.`,
                      );
                    }
                  }}
                >
                  <Paperclip aria-hidden="true" width={16} height={16} />
                </Button>
              </Tooltip>
            </div>
            <ComposerInput
              ref={(el) => {
                textareaRef = el;
              }}
              class="text-area composer-input composer-input-mirror"
              rows={1}
              value={draft()}
              knownSkillIds={knownSkillIds()}
              role="combobox"
              aria-autocomplete="list"
              aria-expanded={comboboxOpen() || fileComboboxOpen()}
              aria-controls={
                comboboxOpen()
                  ? listboxId()
                  : fileComboboxOpen()
                    ? fileListboxId()
                    : undefined
              }
              aria-activedescendant={
                comboboxOpen()
                  ? `${listboxId()}-option-${highlightedIndex()}`
                  : fileComboboxOpen()
                    ? `${fileListboxId()}-option-${highlightedFileIndex()}`
                    : undefined
              }
              onInput={handleInput}
              onClick={(event) => syncCaret(event.currentTarget)}
              onKeyUp={(event) => syncCaret(event.currentTarget)}
              onKeyDown={handleKeyDown}
              onPaste={handlePaste}
              placeholder={
                ctx.replayRunId()
                  ? replayContinuationNodeId(ctx.activeWorkflow(), ctx.runState())
                    ? "Send a message to continue this run."
                    : "This saved run cannot be continued."
                  : props.directChat
                  ? "Message OpenFlow"
                  : props.kickoff
                  ? "Optional message, or use Run in the top bar. / skills · @ files"
                  : props.disabled
                    ? "Run the workflow to chat with agents."
                    : pendingApproval()
                      ? "Approve or deny the tool request above."
                      : `Reply to ${props.label}… / skills · @ files`
              }
              disabled={!inputEnabled()}
            />
            <Tooltip
              label={
                ctx.replayRunId()
                  ? "Continue run"
                  : props.directChat
                  ? "Send message"
                  : props.kickoff
                    ? "Start workflow"
                    : "Send to paused node"
              }
            >
              <Button
                variant="primary"
                class="composer-send-button"
                onClick={handleSubmit}
                disabled={!ctx.canSendChatFor(props.nodeId)}
                aria-label={
                  ctx.replayRunId()
                    ? "Continue saved run with message"
                    : props.directChat
                    ? "Send message"
                    : props.kickoff
                      ? "Start workflow with message"
                      : "Send to paused node"
                }
              >
              <ArrowUp
                class="composer-send-icon"
                aria-hidden="true"
                absoluteStrokeWidth
                strokeWidth={2.3}
              />
            </Button>
            </Tooltip>
            <Show when={!props.directChat && ctx.runState()?.active}>
              <Tooltip label={ctx.stoppingRun() ? "Stopping run" : "Stop run"}>
                <Button
                  variant="danger"
                  class="composer-stop-button"
                  onClick={() => void ctx.handleStopRun()}
                  disabled={ctx.stoppingRun()}
                  aria-label="Stop workflow run"
                >
                  <Square
                    class="composer-stop-icon"
                    aria-hidden="true"
                    fill="currentColor"
                    absoluteStrokeWidth
                    strokeWidth={2}
                  />
                </Button>
              </Tooltip>
            </Show>
          </div>
          <div class="composer-attachment-announcement" aria-live="polite">
            {attachmentAnnouncement()}
          </div>
          <Show when={props.directChat}>
            <ChatRuntimeControls />
          </Show>
          <Show when={!props.kickoff && !props.directChat}>
            <ComposerRuntimeControls nodeId={props.nodeId} disabled={props.disabled} />
          </Show>
        </div>
      </div>
    </div>
  );
}
