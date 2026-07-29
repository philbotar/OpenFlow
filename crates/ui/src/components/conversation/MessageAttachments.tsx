import { createResource, For, Show } from "solid-js";
import ImageIcon from "lucide-solid/icons/image";
import { loadChatAttachmentPreview } from "../../api";
import type { ChatAttachmentRef } from "../../lib/types";

function formatBytes(sizeBytes: number): string {
  if (sizeBytes < 1024) {
    return `${sizeBytes} B`;
  }
  if (sizeBytes < 1024 * 1024) {
    return `${Math.ceil(sizeBytes / 1024)} KB`;
  }
  return `${(sizeBytes / (1024 * 1024)).toFixed(1)} MB`;
}

function displayExtension(fileName: string): string {
  const extension = fileName.split(".").pop()?.toUpperCase();
  return extension && extension.length <= 8 ? extension : "FILE";
}

function AttachmentItem(props: {
  attachment: ChatAttachmentRef;
  runId: string | null;
}) {
  const [preview] = createResource(
    () =>
      props.attachment.kind === "image" && props.runId
        ? [props.runId, props.attachment.id] as const
        : null,
    ([runId, attachmentId]) => loadChatAttachmentPreview(runId, attachmentId),
  );

  return (
    <div class="message-attachment-card">
      <Show
        when={props.attachment.kind === "image" && preview()}
        fallback={
          <div class="message-attachment-placeholder" aria-hidden="true">
            {props.attachment.kind === "image" ? (
              <ImageIcon width={20} height={20} />
            ) : (
              <span class="message-attachment-extension">
                {displayExtension(props.attachment.fileName)}
              </span>
            )}
          </div>
        }
      >
        {(resolved) => (
          <img
            class="message-attachment-preview"
            src={`data:${resolved().mediaType};base64,${resolved().dataBase64}`}
            alt={props.attachment.fileName}
          />
        )}
      </Show>
      <div class="message-attachment-meta">
        <span class="message-attachment-name">{props.attachment.fileName}</span>
        <span class="message-attachment-detail">
          {props.attachment.mediaType} · {formatBytes(props.attachment.sizeBytes)}
        </span>
      </div>
    </div>
  );
}

export function MessageAttachments(props: {
  attachments: readonly ChatAttachmentRef[];
  runId: string | null;
}) {
  return (
    <Show when={props.attachments.length > 0}>
      <div class="message-attachments" aria-label="Message attachments">
        <For each={props.attachments}>
          {(attachment) => (
            <AttachmentItem attachment={attachment} runId={props.runId} />
          )}
        </For>
      </div>
    </Show>
  );
}
