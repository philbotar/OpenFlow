import { For, Show } from "solid-js";
import FileText from "lucide-solid/icons/file-text";
import Folder from "lucide-solid/icons/folder";
import type { ProjectFileReference } from "../../lib/types";

interface FileReferenceComboboxProps {
  open: boolean;
  suggestions: ProjectFileReference[];
  highlightedIndex: number;
  query: string;
  listboxId: string;
  loading: boolean;
  onSelect: (reference: ProjectFileReference) => void;
  onHighlight: (index: number) => void;
}

export function FileReferenceCombobox(props: FileReferenceComboboxProps) {
  return (
    <Show when={props.open}>
      <div class="file-reference-combobox" role="presentation">
        <p class="file-reference-combobox-label eyebrow">
          {props.query === "" ? "Files and folders" : `Files and folders matching @${props.query}`}
        </p>
        <Show
          when={!props.loading && props.suggestions.length > 0}
          fallback={
            <div class="file-reference-empty">
              {props.loading ? "Searching files..." : "No matching files."}
            </div>
          }
        >
          <ul
            id={props.listboxId}
            class="file-reference-combobox-list"
            role="listbox"
            aria-label="File and folder references"
          >
            <For each={props.suggestions}>
              {(reference, index) => (
                <li role="presentation">
                  <button
                    type="button"
                    id={`${props.listboxId}-option-${index()}`}
                    class="file-reference-option"
                    classList={{ "is-highlighted": index() === props.highlightedIndex }}
                    role="option"
                    aria-selected={index() === props.highlightedIndex}
                    onMouseEnter={() => props.onHighlight(index())}
                    onMouseDown={(event) => {
                      event.preventDefault();
                      props.onSelect(reference);
                    }}
                  >
                    <Show
                      when={reference.kind === "directory"}
                      fallback={
                        <FileText class="file-reference-option-icon" width={15} height={15} />
                      }
                    >
                      <Folder class="file-reference-option-icon" width={15} height={15} />
                    </Show>
                    <span class="file-reference-option-path">{reference.displayPath}</span>
                    <span class="file-reference-option-size">
                      {reference.kind === "directory" ? "Folder" : formatBytes(reference.sizeBytes)}
                    </span>
                  </button>
                </li>
              )}
            </For>
          </ul>
        </Show>
      </div>
    </Show>
  );
}

function formatBytes(sizeBytes: number): string {
  if (sizeBytes < 1024) {
    return `${sizeBytes} B`;
  }
  const kib = sizeBytes / 1024;
  if (kib < 1024) {
    return `${kib.toFixed(kib < 10 ? 1 : 0)} KiB`;
  }
  const mib = kib / 1024;
  return `${mib.toFixed(mib < 10 ? 1 : 0)} MiB`;
}
