import { createEffect, createMemo, createResource, createSignal, Show } from "solid-js";
import ArrowUp from "lucide-solid/icons/arrow-up";
import { useAppContext } from "../../context/AppContext";
import {
  applySlashTokenCompletion,
  getActiveSlashToken,
  matchSkillsForSlashQuery,
} from "../../lib/chatCommands";
import {
  applyFileReferenceCompletion,
  extractReferencedFilePaths,
  getActiveFileReferenceToken,
} from "../../lib/fileReferences";
import type { NodeId, ProjectFileReference, SkillSummary } from "../../lib/types";
import { pendingApprovalForNode } from "../../lib/workflow";
import { FileReferenceCombobox } from "./FileReferenceCombobox";
import { SkillCommandCombobox } from "./SkillCommandCombobox";
import { SkillDescriptionPreview } from "./SkillDescriptionPreview";

export function ConversationComposer(props: {
  nodeId: NodeId;
  label: string;
  disabled?: boolean;
  kickoff?: boolean;
}) {
  const ctx = useAppContext();
  let textareaRef: HTMLTextAreaElement | undefined;
  const [caretPosition, setCaretPosition] = createSignal(0);
  const [highlightedIndex, setHighlightedIndex] = createSignal(0);
  const [highlightedFileIndex, setHighlightedFileIndex] = createSignal(0);
  const listboxId = () => `chat-skill-command-listbox-${props.nodeId}`;
  const fileListboxId = () => `chat-file-reference-listbox-${props.nodeId}`;

  const draft = () => ctx.chatDraft(props.nodeId);
  const submission = () => ctx.chatSubmissionFor(props.nodeId);
  const chatEnabled = () => {
    if (props.kickoff) {
      return (
        !props.disabled &&
        ctx.runState()?.active !== true &&
        (ctx.readiness()?.ready ?? false)
      );
    }
    return (
      !props.disabled &&
      ctx.runState()?.active === true &&
      (ctx.runState()?.awaitingNodeIds?.includes(props.nodeId) ||
        ctx.runState()?.awaitingNodeId === props.nodeId) &&
      (ctx.readiness()?.ready ?? false)
    );
  };
  const pendingApproval = () => pendingApprovalForNode(ctx.runState(), props.nodeId);

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
    () => !!activeSlashToken() && suggestions().length > 0 && chatEnabled(),
  );

  const activeFileToken = createMemo(() =>
    getActiveFileReferenceToken(draft(), caretPosition()),
  );
  const fileQuery = createMemo(() =>
    chatEnabled() && activeFileToken() ? activeFileToken()!.query : null,
  );
  const [fileSuggestions] = createResource(fileQuery, async (query) => {
    if (query === null) {
      return [] as ProjectFileReference[];
    }
    return ctx.searchProjectFileReferences(query);
  });
  const fileComboboxOpen = createMemo(
    () => !!activeFileToken() && chatEnabled() && !comboboxOpen(),
  );
  const referencedFilePaths = createMemo(() => extractReferencedFilePaths(draft()));

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

    ctx.handleChatInputKeyDown(event, props.nodeId);
  };

  return (
    <div class="chat-composer">
      <SkillDescriptionPreview nodeId={props.nodeId} />
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
          classList={{ "is-busy": ctx.composerBusyFor(props.nodeId) }}
        >
          <textarea
            ref={textareaRef}
            class="text-area composer-input"
            rows={1}
            value={draft()}
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
            placeholder={
              props.kickoff
                ? "Message to start the workflow... Type / for skills or @ for files."
                : props.disabled
                  ? "Start a run to chat with agents."
                  : pendingApproval()
                    ? "Resolve the pending tool approval above."
                    : `Reply to ${props.label}... Type / for skills or @ for files.`
            }
            disabled={!chatEnabled() || !!pendingApproval()}
          />
          <Show when={submission().invokedSkills.length > 0}>
            <span
              class="composer-skill-pill"
              title={`Sending with skills: ${submission()
                .invokedSkills.map((skill) => `/${skill}`)
                .join(", ")}`}
            >
              {submission()
                .invokedSkills.map((skill) => `/${skill}`)
                .join(", ")}
            </span>
          </Show>
          <Show when={referencedFilePaths().length > 0}>
            <span
              class="composer-file-pill"
              title={`Referenced files: ${referencedFilePaths().join(", ")}`}
            >
              {referencedFilePaths().length === 1
                ? `@ ${referencedFilePaths()[0]}`
                : `@ ${referencedFilePaths().length} files`}
            </span>
          </Show>
          <button
            class="primary-button composer-send-button"
            onClick={() => void ctx.handleSubmitChat(props.nodeId)}
            disabled={!ctx.canSendChatFor(props.nodeId)}
            title={props.kickoff ? "Start workflow" : "Send to paused node"}
            aria-label={
              props.kickoff ? "Start workflow with message" : "Send to paused node"
            }
          >
            <ArrowUp
              class="composer-send-icon"
              aria-hidden="true"
              absoluteStrokeWidth
              strokeWidth={2.3}
            />
          </button>
        </div>
      </div>
    </div>
  );
}
