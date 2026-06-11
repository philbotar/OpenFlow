import { createMemo, createSignal, Show } from "solid-js";
import ArrowUp from "lucide-solid/icons/arrow-up";
import { useAppContext } from "../../context/AppContext";
import {
  applySlashTokenCompletion,
  getActiveSlashToken,
  matchSkillsForSlashQuery,
} from "../../lib/chatCommands";
import type { NodeId } from "../../lib/types";
import type { SkillSummary } from "../../lib/types";
import { pendingApprovalForNode } from "../../lib/workflow";
import { SkillCommandCombobox } from "./SkillCommandCombobox";
import { SkillDescriptionPreview } from "./SkillDescriptionPreview";

export function ConversationComposer(props: {
  nodeId: NodeId;
  label: string;
  disabled?: boolean;
}) {
  const ctx = useAppContext();
  let textareaRef: HTMLTextAreaElement | undefined;
  const [caretPosition, setCaretPosition] = createSignal(0);
  const [highlightedIndex, setHighlightedIndex] = createSignal(0);
  const listboxId = () => `chat-skill-command-listbox-${props.nodeId}`;

  const draft = () => ctx.chatDraft(props.nodeId);
  const submission = () => ctx.chatSubmissionFor(props.nodeId);
  const chatEnabled = () =>
    !props.disabled &&
    ctx.runState()?.active === true &&
    (ctx.runState()?.awaitingNodeIds?.includes(props.nodeId) ||
      ctx.runState()?.awaitingNodeId === props.nodeId) &&
    (ctx.readiness()?.ready ?? false);
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

  const handleInput = (event: InputEvent & { currentTarget: HTMLTextAreaElement }) => {
    ctx.setChatDraft(props.nodeId, event.currentTarget.value);
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
            aria-expanded={comboboxOpen()}
            aria-controls={comboboxOpen() ? listboxId() : undefined}
            aria-activedescendant={
              comboboxOpen()
                ? `${listboxId()}-option-${highlightedIndex()}`
                : undefined
            }
            onInput={handleInput}
            onClick={(event) => syncCaret(event.currentTarget)}
            onKeyUp={(event) => syncCaret(event.currentTarget)}
            onKeyDown={handleKeyDown}
            placeholder={
              props.disabled
                ? "Start a run to chat with agents."
                : pendingApproval()
                  ? "Resolve the pending tool approval above."
                  : `Reply to ${props.label}… Type / for skills.`
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
          <button
            class="primary-button composer-send-button"
            onClick={() => void ctx.handleSubmitChat(props.nodeId)}
            disabled={!ctx.canSendChatFor(props.nodeId)}
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
