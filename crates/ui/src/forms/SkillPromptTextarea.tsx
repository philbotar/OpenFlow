import { createMemo, createSignal, createUniqueId } from "solid-js";
import { SkillCommandCombobox, SkillDescriptionPreview } from "@/components";
import {
  applySlashTokenCompletion,
  getActiveSlashToken,
  matchSkillsForSlashQuery,
} from "@/lib/chatCommands";
import type { SkillSummary } from "@/lib/types";

export function SkillPromptTextarea(props: {
  value: string;
  onInput: (value: string) => void;
  skills: readonly SkillSummary[];
  rows: number;
}) {
  let textareaRef: HTMLTextAreaElement | undefined;
  const [caretPosition, setCaretPosition] = createSignal(0);
  const [highlightedIndex, setHighlightedIndex] = createSignal(0);
  const listboxId = `task-prompt-skill-listbox-${createUniqueId()}`;
  const skillById = createMemo(
    () => new Map(props.skills.map((skill) => [skill.id, skill])),
  );
  const invokedSkillIds = createMemo(() =>
    installedSkillIdsInText(props.value, skillById()),
  );
  const activeSlashToken = createMemo(() =>
    getActiveSlashToken(props.value, caretPosition()),
  );
  const suggestions = createMemo(() => {
    const token = activeSlashToken();
    return token ? matchSkillsForSlashQuery(props.skills, token.query) : [];
  });
  const comboboxOpen = createMemo(
    () => activeSlashToken() !== null && suggestions().length > 0,
  );

  const syncCaret = (target: HTMLTextAreaElement) => {
    setCaretPosition(target.selectionStart ?? target.value.length);
  };

  const applySkill = (skill: SkillSummary) => {
    const token = activeSlashToken();
    if (!token) {
      return;
    }
    const completion = applySlashTokenCompletion(
      props.value,
      token.replaceStart,
      token.replaceEnd,
      skill.id,
    );
    props.onInput(completion.value);
    setHighlightedIndex(0);
    requestAnimationFrame(() => {
      textareaRef?.focus();
      textareaRef?.setSelectionRange(completion.caret, completion.caret);
      setCaretPosition(completion.caret);
    });
  };

  const handleKeyDown = (event: KeyboardEvent) => {
    if (!comboboxOpen()) {
      return;
    }
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
        const value = `${props.value.slice(0, token.replaceStart)}${props.value.slice(token.replaceEnd)}`;
        props.onInput(value);
        requestAnimationFrame(() => {
          textareaRef?.setSelectionRange(token.replaceStart, token.replaceStart);
          setCaretPosition(token.replaceStart);
        });
      }
      setHighlightedIndex(0);
    }
  };

  return (
    <>
      <SkillDescriptionPreview
        skillIds={invokedSkillIds()}
        skillById={skillById()}
      />
      <div class="skill-prompt-input-shell">
        <SkillCommandCombobox
          open={comboboxOpen()}
          suggestions={suggestions()}
          highlightedIndex={highlightedIndex()}
          query={activeSlashToken()?.query ?? ""}
          listboxId={listboxId}
          onSelect={applySkill}
          onHighlight={setHighlightedIndex}
        />
        <textarea
          ref={textareaRef}
          class="text-area"
          rows={props.rows}
          value={props.value}
          aria-label="Task prompt"
          role="combobox"
          aria-autocomplete="list"
          aria-expanded={comboboxOpen()}
          aria-controls={comboboxOpen() ? listboxId : undefined}
          aria-activedescendant={
            comboboxOpen() ? `${listboxId}-option-${highlightedIndex()}` : undefined
          }
          onInput={(event) => {
            props.onInput(event.currentTarget.value);
            syncCaret(event.currentTarget);
            setHighlightedIndex(0);
          }}
          onClick={(event) => syncCaret(event.currentTarget)}
          onKeyUp={(event) => syncCaret(event.currentTarget)}
          onKeyDown={handleKeyDown}
        />
      </div>
    </>
  );
}

function installedSkillIdsInText(
  input: string,
  skillById: ReadonlyMap<string, SkillSummary>,
): string[] {
  const installedSkillIds: string[] = [];
  const seen = new Set<string>();

  for (const token of input.split(/\s+/)) {
    const skillId = token.startsWith("/") ? token.slice(1) : "";
    if (skillById.has(skillId) && !seen.has(skillId)) {
      seen.add(skillId);
      installedSkillIds.push(skillId);
    }
  }

  return installedSkillIds;
}
