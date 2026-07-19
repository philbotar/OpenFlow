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
  getActiveFileReferenceToken,
} from "../../lib/fileReferences";
import { createDebounced } from "../../lib/utils";
import type { NodeId, ProjectFileReference, SkillSummary } from "../../lib/types";
import { pendingApprovalForNode } from "../../lib/workflow";
import { ComposerInput } from "./ComposerInput";
import { ComposerRuntimeControls } from "./ComposerRuntimeControls";
import { FileReferenceCombobox } from "./FileReferenceCombobox";
import { SkillCommandCombobox } from "./SkillCommandCombobox";
import { SkillDescriptionPreview } from "./SkillDescriptionPreview";
import { Button } from "../Button";

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
  const knownSkillIds = createMemo(
    () => new Set(ctx.availableSkills().map((skill) => skill.id)),
  );
  const pendingApproval = () => pendingApprovalForNode(ctx.runState(), props.nodeId);
  const inputEnabled = () => {
    if (ctx.replayRunId()) {
      return false;
    }
    if (props.disabled) {
      return false;
    }
    if (pendingApproval()) {
      return false;
    }
    if (!(ctx.readiness()?.ready ?? false)) {
      return false;
    }
    if (props.kickoff) {
      return !ctx.runState()?.active;
    }
    return ctx.runState()?.active === true;
  };

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
          <div class="chat-composer-main">
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
              placeholder={
                props.kickoff
                  ? "Optional message, or use Run in the top bar. / skills · @ files"
                  : props.disabled
                    ? "Run the workflow to chat with agents."
                    : pendingApproval()
                      ? "Approve or deny the tool request above."
                      : `Reply to ${props.label}… / skills · @ files`
              }
              disabled={!inputEnabled()}
            />
            <Button
              variant="primary"
              class="composer-send-button"
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
            </Button>
          </div>
          <Show when={!props.kickoff}>
            <ComposerRuntimeControls nodeId={props.nodeId} disabled={props.disabled} />
          </Show>
        </div>
      </div>
    </div>
  );
}
