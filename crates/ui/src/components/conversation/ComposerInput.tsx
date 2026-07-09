import FileText from "lucide-solid/icons/file-text";
import Folder from "lucide-solid/icons/folder";
import Sparkles from "lucide-solid/icons/sparkles";
import {
  createEffect,
  createMemo,
  For,
  Show,
  splitProps,
  type ComponentProps,
  type JSX,
} from "solid-js";
import { parseComposerDisplaySegments } from "../../lib/fileReferences";
import { resizeComposerTextarea } from "../../lib/utils";

type ComposerInputProps = {
  value: string;
  knownSkillIds?: ReadonlySet<string>;
} & Omit<ComponentProps<"textarea">, "value" | "children">;

function ComposerFileChipContent(props: { path: string }) {
  return (
    <>
      {props.path.endsWith("/") ? (
        <Folder class="composer-file-chip-icon" width={14} height={14} />
      ) : (
        <FileText class="composer-file-chip-icon" width={14} height={14} />
      )}
      <span class="composer-file-chip-label">{props.path}</span>
    </>
  );
}

function ComposerSkillChipContent(props: { skillId: string }) {
  return (
    <>
      <Sparkles class="composer-file-chip-icon" width={14} height={14} />
      <span class="composer-file-chip-label">{props.skillId}</span>
    </>
  );
}

function ComposerReferenceChip(props: {
  token: string;
  title: string;
  children: JSX.Element;
}) {
  return (
    <span class="composer-token-wrap">
      <span class="composer-token-spacer" aria-hidden="true">
        {props.token}
      </span>
      <span class="composer-file-chip composer-token-chip" title={props.title}>
        {props.children}
      </span>
    </span>
  );
}

export function ComposerInput(props: ComposerInputProps) {
  const [local, rest] = splitProps(props, [
    "value",
    "class",
    "classList",
    "ref",
    "knownSkillIds",
    "onScroll",
    "onInput",
    "placeholder",
  ]);
  let highlightRef: HTMLDivElement | undefined;
  let textareaEl: HTMLTextAreaElement | undefined;
  const segments = createMemo(() =>
    parseComposerDisplaySegments(local.value, local.knownSkillIds),
  );

  const syncHighlightScroll = (scrollTop: number, scrollLeft: number) => {
    if (!highlightRef) {
      return;
    }
    highlightRef.scrollTop = scrollTop;
    highlightRef.scrollLeft = scrollLeft;
  };

  const bindTextareaRef = (el: HTMLTextAreaElement) => {
    textareaEl = el;
    resizeComposerTextarea(el);
    const ref = local.ref;
    if (typeof ref === "function") {
      ref(el);
      return;
    }
    local.ref = el;
  };

  createEffect(() => {
    local.value;
    if (textareaEl) {
      resizeComposerTextarea(textareaEl);
    }
  });

  return (
    <div class="composer-input-stack">
      <div ref={highlightRef} class="composer-input-highlight" aria-hidden="true">
        <Show
          when={local.value.length === 0 && local.placeholder}
          fallback={
            <For each={segments()}>
              {(segment) =>
                segment.kind === "text" ? (
                  <span class="composer-input-text">{segment.value}</span>
                ) : segment.kind === "skillRef" ? (
                  <ComposerReferenceChip token={segment.token} title={segment.token}>
                    <ComposerSkillChipContent skillId={segment.skillId} />
                  </ComposerReferenceChip>
                ) : (
                  <ComposerReferenceChip token={segment.token} title={segment.path}>
                    <ComposerFileChipContent path={segment.path} />
                  </ComposerReferenceChip>
                )
              }
            </For>
          }
        >
          <span class="composer-input-placeholder">{local.placeholder}</span>
        </Show>
      </div>
      <textarea
        {...rest}
        ref={bindTextareaRef}
        class={local.class}
        classList={local.classList}
        value={local.value}
        placeholder=""
        aria-label={
          typeof local.placeholder === "string" ? local.placeholder : undefined
        }
        onInput={(event) => {
          resizeComposerTextarea(event.currentTarget);
          if (typeof local.onInput === "function") {
            local.onInput(event);
          }
        }}
        onScroll={(event) => {
          syncHighlightScroll(event.currentTarget.scrollTop, event.currentTarget.scrollLeft);
          if (typeof local.onScroll === "function") {
            local.onScroll(event);
          }
        }}
      />
    </div>
  );
}
