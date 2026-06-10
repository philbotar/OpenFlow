import { createMemo, createSignal, Show } from "solid-js";
import ArrowUp from "lucide-solid/icons/arrow-up";
import { useAppContext } from "../../context/AppContext";
import {
  applySlashTokenCompletion,
  getActiveSlashToken,
  matchSkillsForSlashQuery,
} from "../../lib/chatCommands";
import type { SkillSummary } from "../../lib/types";
import { SkillCommandCombobox } from "./SkillCommandCombobox";
import { SkillDescriptionPreview } from "./SkillDescriptionPreview";

const COMBOBOX_LISTBOX_ID = "chat-skill-command-listbox";

export function ConversationComposer() {
  const ctx = useAppContext();
  let textareaRef: HTMLTextAreaElement | undefined;
  const [caretPosition, setCaretPosition] = createSignal(0);
  const [highlightedIndex, setHighlightedIndex] = createSignal(0);

  const activeSlashToken = createMemo(() =>
    getActiveSlashToken(ctx.chatInput(), caretPosition()),
  );
  const suggestions = createMemo(() => {
    const token = activeSlashToken();
    if (!token) {
      return [];
    }
    return matchSkillsForSlashQuery(ctx.availableSkills(), token.query);
  });
  const comboboxOpen = createMemo(
    () => !!activeSlashToken() && suggestions().length > 0 && ctx.chatEnabledMemo(),
  );

  const syncCaret = (target: HTMLTextAreaElement) => {
    setCaretPosition(target.selectionStart ?? target.value.length);
  };

  const applySkill = (skill: SkillSummary) => {
    const token = activeSlashToken();
    if (!token) {
      return;
    }

    const { value, caret } = applySlashTokenCompletion(
      ctx.chatInput(),
      token.replaceStart,
      token.replaceEnd,
      skill.id,
    );
    ctx.setChatInput(value);
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

  const handleInput = (event: InputEvent & { currentTarget: HTMLTextAreaElement }) => {
    ctx.setChatInput(event.currentTarget.value);
    syncCaret(event.currentTarget);
    setHighlightedIndex(0);
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
          ctx.setChatInput(
            `${ctx.chatInput().slice(0, token.replaceStart)}${ctx.chatInput().slice(token.replaceEnd)}`,
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

    ctx.handleChatInputKeyDown(event);
  };

  return (
    <div class="chat-composer">
      <SkillDescriptionPreview />
      <div class="chat-composer-input-shell">
        <SkillCommandCombobox
          open={comboboxOpen()}
          suggestions={suggestions()}
          highlightedIndex={highlightedIndex()}
          query={activeSlashToken()?.query ?? ""}
          listboxId={COMBOBOX_LISTBOX_ID}
          onSelect={applySkill}
          onHighlight={setHighlightedIndex}
        />
        <div
          class="chat-composer-pill"
          classList={{ "is-busy": ctx.chatComposerBusyMemo() }}
        >
          <textarea
            ref={textareaRef}
            class="text-area composer-input"
            rows={1}
            value={ctx.chatInput()}
            role="combobox"
            aria-autocomplete="list"
            aria-expanded={comboboxOpen()}
            aria-controls={comboboxOpen() ? COMBOBOX_LISTBOX_ID : undefined}
            aria-activedescendant={
              comboboxOpen()
                ? `${COMBOBOX_LISTBOX_ID}-option-${highlightedIndex()}`
                : undefined
            }
            onInput={handleInput}
            onClick={(event) => syncCaret(event.currentTarget)}
            onKeyUp={(event) => syncCaret(event.currentTarget)}
            onKeyDown={handleKeyDown}
            placeholder={
              ctx.selectedNodePendingApproval()
                ? "Resolve the pending tool approval above."
                : "Continue paused node. Type / for skills."
            }
            disabled={!ctx.chatEnabledMemo() || !!ctx.selectedNodePendingApproval()}
          />
          <Show when={ctx.chatSubmission().invokedSkills.length > 0}>
            <span
              class="composer-skill-pill"
              title={`Sending with skills: ${ctx
                .chatSubmission()
                .invokedSkills.map((skill) => `/${skill}`)
                .join(", ")}`}
            >
              {ctx
                .chatSubmission()
                .invokedSkills.map((skill) => `/${skill}`)
                .join(", ")}
            </span>
          </Show>
          <button
            class="primary-button composer-send-button"
            onClick={() => void ctx.handleSubmitChat()}
            disabled={!ctx.canSendChatMemo()}
            title="Send to paused node"
            aria-label="Send to paused node"
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
