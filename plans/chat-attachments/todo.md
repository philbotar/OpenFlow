# Chat Attachments Implementation Plan

**Goal:** Let users attach photos and common documents to direct chats and workflow chat messages, send the content to supported models, and reopen the conversation with durable previews.

**Architecture:** Keep selected source paths transient in the UI. On send, orchestration validates and copies files into the durable run directory, while engine transcripts and run projections persist only opaque `ChatAttachmentRef` metadata. A run-scoped AI adapter hydrates bytes immediately before the provider maps each user turn to Rig multipart `UserContent`; the UI fetches bounded previews through the desktop seam.

**Tech Stack:** Rust workspace (`engine`, `providers`, `orchestration`, `desktop`), Tauri 2, SolidJS/TypeScript, Rig 0.39, Nextest, Wiremock, Vitest.

## Decisions

- Use one structured user-message contract for initial entrypoints and later replies.
- Support direct chats and workflow chat composers through the existing shared `ConversationComposer`.
- First media set: JPEG, PNG, GIF, WebP, PDF, TXT, Markdown, CSV, JSON, HTML, CSS, JavaScript, and Python.
- Limits: 4 attachments per message, 10 MiB per file, 25 MiB total per message.
- Reject directories, symlinks, empty files, MIME mismatches, SVG, HEIC/HEIF, audio, video, DOCX, and other binary formats with an actionable error.
- Do not infer vision support from model names. Dispatch valid media through the configured provider transport; map unsupported-model/provider responses to a clear error.
- Keep current `@{path}` project-file inlining unchanged. It remains a separate text-context feature.
- Exclude assistant-generated images and rich MCP tool-result images. They require separate output persistence semantics.
- Sequence implementation after the current uncommitted skill-invocation work is reconciled. Preserve `invokedSkillIds` through every changed request signature.
- Auto-start roots receive the structured entrypoint directly. Manual roots start without an engine entrypoint, then receive the retained structured kickoff exactly once after the pause appears.
- Stored files use `{attachmentId}.{normalizedExtension}` under `<run_dir>/attachments/`; refs never expose that path.
- Preview IPC returns `mediaType` plus bounded base64. Image previews use the first decoded frame, JPEG output, at most 512 px per side and 512 KiB.
- Reject a leaf source path that is a symlink; allow an ancestor directory that resolves through a symlink.
- An image-only direct chat derives its initial title from the first sanitized filename.

## Data Flow

```text
ConversationComposer
  -> api.ts / Tauri command
  -> RunAttachmentStore validates + copies
  -> ChatAttachmentRef in engine transcript + WorkflowRunState
  -> AiInvocationAdapter hydrates bytes
  -> providers/rig_adapter emits multipart UserContent
  -> model

Replay:
run checkpoint refs -> WorkflowRunState -> preview command -> bounded thumbnail
```

---

- [x] 1. Send and render one picked image in a new direct chat end-to-end.
- [x] 2. Carry attachments through replies, saved-chat resume, workflow kickoff, checkpoint, and replay.
- [x] 3. Add multi-file picker, paste/drop UX, strict limits, rollback, and deletion cleanup.
- [x] 4. Send PDF and UTF-8 document attachments through supported provider transports.
- [x] 5. Run provider/live compatibility checks, full verification, and update product docs.
