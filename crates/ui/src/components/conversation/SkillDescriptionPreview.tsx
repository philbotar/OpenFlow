import { For, Show } from "solid-js";
import type { SkillSummary } from "../../lib/types";

export function SkillDescriptionPreview(props: {
  skillIds: readonly string[];
  skillById: ReadonlyMap<string, SkillSummary>;
}) {
  return (
    <Show when={props.skillIds.length > 0}>
      <div class="skill-description-preview" aria-live="polite">
        <For each={props.skillIds}>
          {(skillId) => {
            const skill = () => props.skillById.get(skillId);
            return (
              <article class="skill-description-entry">
                <p class="eyebrow">/{skillId}</p>
                <Show when={skill()?.name && skill()?.name !== skillId}>
                  <h4 class="skill-description-title">{skill()?.name}</h4>
                </Show>
                <Show
                  when={skill()?.description}
                  fallback={<p class="skill-description-missing">Description unavailable</p>}
                >
                  <p class="skill-description-body">{skill()?.description}</p>
                </Show>
              </article>
            );
          }}
        </For>
      </div>
    </Show>
  );
}
