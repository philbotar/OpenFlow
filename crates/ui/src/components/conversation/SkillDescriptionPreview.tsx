import { createMemo, For, Show } from "solid-js";
import { useAppContext } from "../../context/AppContext";
import type { NodeId } from "../../lib/types";

export function SkillDescriptionPreview(props: { nodeId: NodeId }) {
  const ctx = useAppContext();
  const invokedSkills = createMemo(
    () => ctx.chatSubmissionFor(props.nodeId).invokedSkills,
  );

  return (
    <Show when={invokedSkills().length > 0}>
      <div class="skill-description-preview" aria-live="polite">
        <For each={invokedSkills()}>
          {(skillId) => {
            const skill = () => ctx.skillById().get(skillId);
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
