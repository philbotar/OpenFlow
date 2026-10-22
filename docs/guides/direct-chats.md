# Direct chats

A direct chat is a saved conversation with a model. You can create and browse chats before [connecting a provider](provider-setup.md), but sending a message requires a ready provider.

## Start or restore a chat

Select **New chat**. Choose the [project](projects-and-persistence.md) that should own the chat before sending the first message, or leave the project control empty for an isolated app-managed workspace.

Send a request such as “Prepare a small release checklist for a desktop app.” The assistant’s reply appears in the conversation. Sending the first message creates a saved run and locks project selection for that chat. To use a different project, start a new chat.

The model, speed, approval mode, reasoning effort, and skills remain available when the selected provider supports them.

<figure class="product-shot product-shot--detail" style="--shot-width: 625px">

[![Unsent release-checklist request above Project: None, Approval: Read only, and model settings, with paperclip and send arrow on the right.](../public/media/screenshots/direct-chat-composer-detail.png)](../public/media/screenshots/direct-chat-prepared.png)

<figcaption>Prepared, not sent. Choose project and runtime settings below the message; attach files with the paperclip and send with the upward arrow. Select the image for the complete window.</figcaption>
</figure>

OpenFlow names the chat from its first message. Select it under **Chats** to reopen the transcript, then send another message to continue the same run.

A project chat uses that project's configured execution folder and `.flow/runs/` storage. A chat without a project uses an isolated app-managed workspace.

To remove a chat, stop its active run, then choose **Delete chat** from the chat row menu.

## Attach files

Use the paperclip, paste an image, or drop files onto the composer. A message accepts at most four files; each file can be up to 10 MiB and the message total can be up to 25 MiB.

Supported formats are JPEG, PNG, GIF, WebP, PDF, TXT, Markdown, CSV, JSON, HTML, CSS, JavaScript, and Python.

OpenFlow validates the file, copies it into the run-owned attachment directory, and sends the managed copy to the provider. The original path is not part of the checkpoint.

Image previews are local bounded derivatives. Documents appear as metadata cards and are not rendered as active HTML or scripts. A provider or model may still reject an accepted media type.

If a file is rejected, remove it or export it to a supported format and retry. If the model rejects accepted media, choose a model that supports that media type.

For missing files, corrupt copies, or preview errors, follow [Chat attachments](../troubleshooting/README.md#chat-attachments).

## Continue a conversation

Each assistant turn ends after a normal reply. If the assistant asks for a choice, answer the card or type the answer in the same composer. The same [run controls](runs.md) handle questions, approvals, and recovery.

Chats appear under **Chats** in the sidebar, separately from the workflow catalog and canvas.

## Recover a failed turn

For an active failed turn, use **Retry**, or send a correction in the composer. Change the model first if the error is model-specific. Retry retains the chat's run, transcript, and attachments.

Reopen an existing chat to continue its saved conversation. Opening an active chat preserves its pending question or approval; answer it in the conversation. [Runs and recovery](runs.md#retry-a-failed-or-interrupted-node) describes which settings can change before retry.

## Data and cleanup

[Security and data](../security-and-data.md) describes the data boundaries. The [persistence reference](../reference/README.md#runtime-and-persistence-paths) lists attachment and chat storage locations.

Deleting a chat removes its history and run-owned copies. If filesystem cleanup cannot finish immediately, OpenFlow records pending cleanup and retries later.
