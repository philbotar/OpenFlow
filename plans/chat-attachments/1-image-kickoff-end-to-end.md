# Send One Image in a New Direct Chat

## Goal

- A user can pick one JPEG, PNG, GIF, or WebP image in an unsent direct chat, send it with optional text, let the configured model receive the image, and see the durable image attachment in the user bubble.

## Current Question

- Question: Where should sent attachment bytes live?
- Recommended answer: Store them under `<run_dir>/attachments/`, beside `artifacts/`.
- Reason: The direct chat already points at a durable run. Run-owned files survive checkpoint/replay, follow app/project run placement, and can be deleted with that run. `chats.json` stays small.

## Codebase Findings

- `crates/ui/src/components/conversation/ConversationComposer.tsx` is shared by direct and workflow chat.
- `crates/ui/src/context/appProvider/useRunSession.ts` sends a new direct chat through `startChat`.
- `crates/orchestration/src/backend/chat.rs::start_chat_with_skill_ids` converts the first message into a run entrypoint.
- `crates/engine/src/execution/node_invocation.rs::build_node_input` currently embeds the entrypoint as JSON text.
- `crates/providers/src/rig_adapter/convert.rs::to_completion_request` currently emits text-only `Message::user` values.
- Rig 0.39 supports `Message::User { content: OneOrMany<UserContent> }`, including image and document blocks.
- `crates/orchestration/src/adapters/storage/run_checkpoint_store.rs` owns durable run directories and already creates `artifacts/`.
- Focused test commands:
  - `cargo nextest run -p engine`
  - `cargo nextest run -p providers`
  - `cargo nextest run -p orchestration --lib`
  - `cargo nextest run -p desktop`
  - `npm --prefix crates/ui run test -- src/api.test.ts src/components/conversation/ConversationComposer.test.tsx src/components/conversation/Message.test.tsx src/app/App.test.tsx`

## Ownership

- Create: `crates/orchestration/src/adapters/storage/run_attachment_store.rs` for validated run-owned copies, reads, and bounded image previews.
- Modify: `Cargo.toml` and `crates/orchestration/Cargo.toml` to add an image decoder with only JPEG, PNG, GIF, and WebP features for bounded previews.
- Modify: `crates/orchestration/src/run/ports.rs` to add the `RunAttachmentStore` port consumed by run coordination and invocation.
- Modify: `crates/orchestration/src/adapters/storage/mod.rs`, `crates/orchestration/src/backend/mod.rs`, and `crates/orchestration/src/run/coordinator/mod.rs` to wire the port.
- Modify: `crates/engine/src/conversation/mod.rs` to add `ChatAttachmentKind`, `ChatAttachmentRef`, attachment metadata on `ChatMessage`, and attachment refs on `AgentTranscriptItem::UserMessage`.
- Modify: `crates/engine/src/ports/outbound.rs` to add transient resolved attachment payloads to `AgentRequest`.
- Modify: `crates/engine/src/execution/interactive_engine/{mod.rs,checkpoint.rs}` and `crates/engine/src/execution/node_invocation.rs` to preserve initial entrypoint attachment refs and expose them on the first `AgentRequest`.
- Modify: `crates/orchestration/src/run/persistence.rs`, `crates/orchestration/src/run/coordinator/session.rs`, `crates/orchestration/src/run/execution/mod.rs`, and `crates/orchestration/src/run/execution/drive/setup.rs` to carry the run attachment root and store.
- Modify: `crates/orchestration/src/run/execution/ai_adapter.rs` to hydrate referenced files before delegating to the provider.
- Modify: `crates/providers/src/rig_adapter/{convert.rs,model.rs}` to emit text plus image `UserContent` in one user message.
- Modify: `crates/providers/tests/rig_openai_compat.rs` and `crates/providers/tests/rig_anthropic.rs` to prove actual wire payloads.
- Modify: `crates/orchestration/src/backend/chat.rs` and `crates/desktop/src/commands/chat.rs` to accept a structured entrypoint while preserving `invoked_skill_ids`.
- Create: `crates/desktop/src/commands/attachment.rs` for bounded preview reads; register it in `crates/desktop/src/commands/mod.rs` and `crates/desktop/src/lib.rs`.
- Modify: `crates/ui/src/lib/types/index.ts` and `crates/ui/src/api.ts` for attachment draft, ref, structured message, and preview DTOs.
- Create: `crates/ui/src/components/conversation/MessageAttachments.tsx` and `crates/ui/src/components/conversation/MessageAttachments.test.tsx`.
- Create: `crates/ui/src/components/conversation/ConversationComposer.test.tsx`.
- Modify: `crates/ui/src/components/conversation/{ConversationComposer.tsx,Message.tsx,ConversationSegmentMessages.tsx,index.ts}` and `crates/ui/src/styles/chat.css`.
- Modify: `crates/ui/src/context/appProvider/{useChatComposer.ts,useRunSession.ts,useAppProviderState.ts}` and `crates/ui/src/context/AppContext.tsx`.
- Test: `crates/engine/src/conversation/mod.rs`, `crates/engine/src/execution/interactive_engine/tests.rs`, `crates/orchestration/src/backend/tests.rs`, `crates/ui/src/api.test.ts`, and `crates/ui/src/app/App.test.tsx`.
- Update: `crates/engine/tests/snapshots/public_api.txt` through the repo public-API snapshot workflow.

## Contract

- `ChatAttachmentRef` contains `id`, sanitized `file_name`, `media_type`, `size_bytes`, `sha256`, and `kind`; it never contains a source or storage path.
- A UI `PendingChatAttachment` may contain `sourcePath`, but only in volatile composer state and the outbound command.
- Sent messages carry `text`, `attachments`, and existing `invokedSkillIds`.
- `ChatMessage.attachments` and `AgentTranscriptItem::UserMessage.attachments` use `#[serde(default, skip_serializing_if = "Vec::is_empty")]` so old checkpoints deserialize unchanged.
- `AgentRequest` carries a transient `resolved_attachments` map keyed by attachment ID. That map is not checkpointed.
- Define `UserMessageInput { text, attachmentSourcePaths }` in this slice and reuse it for later replies.
- Store each accepted file as `{attachmentId}.{normalizedExtension}`. Hydration never scans or guesses a path.
- Preview output is `{ mediaType: "image/jpeg", dataBase64 }`; decode the first image frame and reduce dimensions/quality until it is at most 512 px per side and 512 KiB.
- Provider conversion preserves order: text first when non-empty, then attachment blocks in selection order.
- Image-only messages are valid. The UI renders an attachment-only user bubble instead of dropping it as empty.
- An image-only first message uses its first sanitized filename as the direct-chat title seed.

## Steps

- [x] **Step 1: Write failing engine contract tests**
  - Add legacy JSON tests proving `ChatMessage` and `AgentTranscriptItem::UserMessage` without `attachments` deserialize to an empty vector.
  - Add a rich serde round-trip with one image ref.
  - Add an initial-entrypoint test proving one image ref appears on the first `AgentRequest`.
  - Expected RED: attachment types/fields and entrypoint attachment state do not exist.

- [x] **Step 2: Add the pure engine attachment contract**
  - Add the metadata types and defaulted fields.
  - Keep attachment bytes and filesystem paths out of engine state.
  - Extend the initial entrypoint/checkpoint path without removing the existing text-only constructor; map text-only callers to an empty attachment list.
  - Update exhaustive `AgentTranscriptItem::UserMessage` matches across the workspace.
  - Run: `cargo nextest run -p engine`
  - Expected: PASS.

- [x] **Step 3: Write failing storage and start-chat tests**
  - Add `run_attachment_store.rs` tests for one valid JPEG copy, safe generated filename, SHA-256, read-back, and bounded preview output.
  - Extend the backend direct-chat start test: source path enters the backend; run state and initial checkpoint contain only the safe ref; copied bytes exist under `<run_dir>/attachments/`.
  - Expected RED: no attachment store or structured entrypoint exists.

- [x] **Step 4: Implement validated run-owned image ingestion**
  - Add constants for 10 MiB/file and 25 MiB/message.
  - Reject directories, symlinks, empty files, unsupported magic bytes, MIME/extension mismatch, and reads exceeding the byte limit.
  - Copy through a uniquely named temp file, hash while copying, flush/sync, then rename atomically.
  - Store as UUID plus allowlisted extension. Keep the sanitized display filename only in metadata.
  - Generate a bounded preview no larger than 512 px on either axis and 512 KiB encoded.
  - Add exact-run removal now so any error after run creation removes the incomplete run tree.
  - Add the store to `AppBackendDeps` so tests can use a temp-backed implementation.

- [x] **Step 5: Write failing provider mapping and wire tests**
  - Unit-test `to_completion_request`: the initial user message contains `UserContent::Text` plus one typed `UserContent::Image`.
  - Add OpenAI Responses and Anthropic Wiremock cases that assert base64 image content and MIME.
  - Expected RED: conversion emits only text.

- [x] **Step 6: Hydrate and map the image**
  - Pass the attachment root/store into `AiInvocationAdapter`.
  - Before `inner.invoke_stream`, collect every referenced attachment in the request, read it, verify size/hash, and fill `resolved_attachments`.
  - Map JPEG/PNG/GIF/WebP bytes to Rig image content. Change provider conversion to return an error for a missing payload or unsupported MIME before HTTP.
  - Do not use `CompletionRequest.documents`; attachments belong to the user message.
  - Run: `cargo nextest run -p providers`
  - Expected: PASS, including both wire tests.

- [x] **Step 7: Write failing UI and IPC tests**
  - `api.test.ts`: native picker uses image filters; `startChat` sends the structured entrypoint plus existing skill IDs.
  - `ConversationComposer.test.tsx`: paperclip button has `aria-label="Attach files"`, selection shows a removable filename card, and image-only draft enables Send.
  - `App.test.tsx`: new direct-chat send returns a user `ChatMessage` with an attachment and clears the volatile draft only after success.
  - `MessageAttachments.test.tsx`: one image ref loads its preview and renders filename alt text.
  - Expected RED: the picker, structured DTO, draft state, and attachment renderer do not exist.

- [x] **Step 8: Implement the basic UI path**
  - Wrap the existing `openNativeDialog` in an attachment-specific `api.ts` fn with multi-select off for this slice.
  - Store one pending source path per workflow/chat + node draft.
  - Preserve it on picker, import, or start failure; clear it with text only after successful send.
  - Add the paperclip next to Send using existing `Button` and `Tooltip`.
  - Render the selected filename card with an accessible remove button.
  - Add `attachments.length > 0` to send eligibility.
  - Render sent image previews through the bounded preview command; never construct `file://` URLs or expose absolute paths.
  - Run: `npm --prefix crates/ui run test -- src/api.test.ts src/components/conversation/ConversationComposer.test.tsx src/components/conversation/MessageAttachments.test.tsx src/components/conversation/Message.test.tsx src/app/App.test.tsx`
  - Expected: PASS.

- [x] **Step 9: Verify the slice**
  - Run: `./scripts/check-fast.sh engine providers orchestration`
  - Run: `./scripts/test-fast.sh --execution --desktop`
  - Run: `./scripts/verify.sh ui-typecheck ui-test public-api arch`
  - Expected: every command PASS.
  - Manual smoke with a mocked or configured vision model: create direct chat, attach PNG, send no text, confirm the model receives image content and the user bubble renders after the run starts.

## Maintainability Gate

- [x] Reuses the existing `ConversationComposer`, `api.ts`, run directory, and `AiInvocationAdapter` seams.
- [x] Keeps filesystem I/O in orchestration and provider wire mapping in providers.
- [x] Persists refs only; no base64 or absolute path in chat metadata/checkpoints.
- [x] Keeps one structured message contract for first turn and later replies.
- [x] Preserves current skill IDs and `@{path}` behavior.
- [x] Tests observable UI, checkpoint, and HTTP wire behavior.

## Self-Review

- [x] Spec coverage: picker -> storage -> checkpoint -> provider -> rendered user bubble.
- [x] Placeholder scan: no unresolved implementation choices.
- [x] Type/name consistency: `ChatAttachmentRef`, pending source, and structured message names match across crates.
- [x] Command quality: focused and broader commands include expected PASS/RED outcomes.

## Result

- Status: Complete.
- Verification: Engine 195/195; providers 163/163; orchestration lib 582/582; coordinator 39/39; desktop 6/6; focused UI 165/165; attachment browser E2E 2/2.
- Notes: Structured kickoff refs persist without paths or bytes. Orchestration owns validation/copy/rollback; provider hydration occurs immediately before HTTP. `invokedSkillIds` remain part of the same accepted kickoff.
