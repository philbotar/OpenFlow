import { For, Show } from "solid-js";
import type { SkillSummary } from "../../lib/types";

interface SkillCommandComboboxProps {
  open: boolean;
  suggestions: SkillSummary[];
  highlightedIndex: number;
  query: string;
  listboxId: string;
  onSelect: (skill: SkillSummary) => void;
  onHighlight: (index: number) => void;
}

export function SkillCommandCombobox(props: SkillCommandComboboxProps) {
  return (
    <Show when={props.open && props.suggestions.length > 0}>
      <div class="skill-command-combobox" role="presentation">
        <p class="skill-command-combobox-label eyebrow">
          {props.query === "" ? "Skills" : `Skills matching /${props.query}`}
        </p>
        <ul
          id={props.listboxId}
          class="skill-command-combobox-list"
          role="listbox"
          aria-label="Skill commands"
        >
          <For each={props.suggestions}>
            {(skill, index) => (
              <li role="presentation">
                <button
                  type="button"
                  id={`${props.listboxId}-option-${index()}`}
                  class="skill-command-option"
                  classList={{ "is-highlighted": index() === props.highlightedIndex }}
                  role="option"
                  aria-selected={index() === props.highlightedIndex}
                  onMouseEnter={() => props.onHighlight(index())}
                  onMouseDown={(event) => {
                    event.preventDefault();
                    props.onSelect(skill);
                  }}
                >
                  <span class="skill-command-option-id">/{skill.id}</span>
                  <Show when={skill.name !== skill.id}>
                    <span class="skill-command-option-name">{skill.name}</span>
                  </Show>
                  <Show when={skill.description}>
                    <span class="skill-command-option-description">{skill.description}</span>
                  </Show>
                </button>
              </li>
            )}
          </For>
        </ul>
      </div>
    </Show>
  );
}
