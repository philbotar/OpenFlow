import { createSignal, For, Show } from "solid-js";
import ChevronDown from "lucide-solid/icons/chevron-down";
import type { DiffFile, DiffLine } from "@/lib/diff";

function lineNumber(line: DiffLine): string {
  const value = line.newNo ?? line.oldNo;
  return value === null ? "" : String(value);
}

function FileBlock(props: { file: DiffFile }) {
  const [open, setOpen] = createSignal(true);
  const [viewed, setViewed] = createSignal(false);

  return (
    <div class="diff-file" classList={{ "is-viewed": viewed() }}>
      <div class="diff-file-header">
        <button
          type="button"
          class="diff-file-toggle"
          aria-expanded={open()}
          onClick={() => setOpen((value) => !value)}
        >
          <ChevronDown
            class="diff-chevron"
            classList={{ collapsed: !open() }}
            size={14}
            aria-hidden="true"
          />
          <span class="diff-file-path">{props.file.path}</span>
        </button>
        <span class="diff-file-stat">
          <Show when={props.file.additions > 0}>
            <span class="diff-add-count">+{props.file.additions}</span>
          </Show>
          <Show when={props.file.deletions > 0}>
            <span class="diff-del-count">-{props.file.deletions}</span>
          </Show>
        </span>
        <input
          type="checkbox"
          class="diff-viewed"
          title="Viewed"
          aria-label="Mark file as viewed"
          checked={viewed()}
          onChange={(event) => setViewed(event.currentTarget.checked)}
        />
      </div>
      <Show when={open() && !viewed()}>
        <Show
          when={!props.file.binary}
          fallback={<div class="diff-binary">Binary file not shown</div>}
        >
          <div class="diff-file-body">
            <For each={props.file.hunks}>
              {(hunk) => (
                <>
                  <Show when={hunk.precedingUnmodified > 0}>
                    <div class="diff-context-band">
                      {hunk.precedingUnmodified} unmodified line
                      {hunk.precedingUnmodified === 1 ? "" : "s"}
                    </div>
                  </Show>
                  <For each={hunk.lines}>
                    {(line) => (
                      <div
                        class="diff-line"
                        classList={{
                          "is-add": line.kind === "add",
                          "is-del": line.kind === "del",
                        }}
                      >
                        <span class="diff-ln">{lineNumber(line)}</span>
                        <span class="diff-code">{line.text || " "}</span>
                      </div>
                    )}
                  </For>
                </>
              )}
            </For>
          </div>
        </Show>
      </Show>
    </div>
  );
}

export function GitDiffView(props: { files: DiffFile[] }) {
  return (
    <div class="diff-view">
      <For each={props.files}>{(file) => <FileBlock file={file} />}</For>
    </div>
  );
}
