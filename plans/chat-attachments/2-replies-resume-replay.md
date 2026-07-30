# Carry Attachments Through Replies, Resume, and Replay

## Goal

- The same structured message supports active replies, saved-chat resume, direct-chat restart, workflow kickoff/manual-root flush, checkpoint restore, and replay without losing attachment refs or duplicating user messages.

## Current Question

- Question: Should attachment messages use a new run command?
- Recommended answer: No. Extend the existing entrypoint and `submit_user_input` paths with one structured message DTO while keeping text-only wrappers for internal tests/headless callers.
- Reason: A second command path would duplicate run validation, skill invocation, projection, and checkpoint behavior.

## Codebase Findings

- `crates/ui/src/context/appProvider/useRunSession.ts` has three direct-chat send paths: start, active input, and durable resume.
- `crates/ui/src/context/appProvider/useChatComposer.ts::flushPendingKickoff` forwards workflow kickoff text to one manual root.
- `crates/orchestration/src/run/coordinator/mod.rs::submit_user_input_with_skill_ids` appends the user projection immediately and sends `ExecutionAction::ProvideInput`.
- `crates/orchestration/src/run/execution/drive/interaction.rs` calls `InteractiveEngine::on_human_input`.
- `InteractiveEngineCheckpoint.transcripts` is the model replay source; `RunCheckpointPayload.projection` is the UI replay source.
- `ConversationSegmentMessages.tsx` currently drops empty, non-streaming messages.
- Focused test commands:
  - `cargo nextest run -p engine interactive_engine`
  - `cargo nextest run -p orchestration --lib`
  - `cargo nextest run -p orchestration --test workflow_acceptance --no-capture`
  - `npm --prefix crates/ui run test -- src/lib/workflow/workflow.test.ts src/components/conversation/ConversationSegmentMessages.test.tsx src/app/App.test.tsx`

## Ownership

- Modify: `crates/engine/src/execution/interactive_engine/{mod.rs,checkpoint.rs,tests.rs}` for structured follow-up input and checkpoint restore.
- Modify: `crates/orchestration/src/run/execution/{mod.rs,drive/interaction.rs,events.rs,tests.rs}` for structured `ExecutionAction::ProvideInput`.
- Modify: `crates/orchestration/src/run/coordinator/{mod.rs,tests.rs}` for async attachment ingestion, validation, action enqueue, and projection.
- Modify: `crates/orchestration/src/backend/runs.rs`, `crates/desktop/src/commands/run.rs`, and `crates/ui/src/api.ts`.
- Modify: `crates/ui/src/context/appProvider/{useChatComposer.ts,useRunSession.ts,useAppProviderState.ts}` and `crates/ui/src/context/AppContext.tsx`.
- Modify: `crates/ui/src/lib/workflow/runState.ts` for attachment-only eligibility.
- Modify: `crates/ui/src/components/conversation/ConversationSegmentMessages.tsx`.
- Test: `crates/orchestration/src/run/coordinator/tests.rs`, `crates/orchestration/src/run/execution/tests.rs`, `crates/ui/src/lib/workflow/workflow.test.ts`, new `crates/ui/src/components/conversation/ConversationSegmentMessages.test.tsx`, and `crates/ui/src/app/App.test.tsx`.

## Contract

- Introduce one orchestration IPC DTO: `UserMessageInput { text, attachmentSourcePaths }`.
- After ingestion, convert it once to an engine user message containing text plus `ChatAttachmentRef` values.
- Preserve `invoked_skill_ids` beside, not inside, the user message.
- Pending manual-root kickoff state contains both the structured message and `invokedSkillIds`.
- Auto-start roots receive the structured entrypoint. Manual roots start with no engine entrypoint and submit the retained kickoff once after the initial pause, preventing duplicate text/media.
- Text-only public wrappers call the structured fn with an empty attachment list.
- `WorkflowRunState.chat_logs` and engine transcript receive the same refs in the same accepted operation.
- A failed submission leaves text and pending attachments in UI draft state.
- A successful accepted submission clears both once.
- Replay preview uses `runId + attachmentId`; the attachment ref never contains an absolute path.

## Steps

- [x] **Step 1: Write failing follow-up and checkpoint tests**
  - Engine: `on_human_message` records text plus attachment refs; `prepare_stop_checkpoint` and `from_checkpoint` retain them.
  - Engine: `conversation_history` projects the same refs into `ChatMessage`.
  - Provider mapping: a historical user turn remains one multipart user message, in transcript order.
  - Expected RED: later input accepts only `&str`.

- [x] **Step 2: Implement structured engine follow-up input**
  - Add `on_human_message(node_id, text, attachments)`.
  - Keep `on_human_input(node_id, text)` as the text-only wrapper.
  - Update checkpoint serde and exhaustive transcript matches.
  - Run: `cargo nextest run -p engine`
  - Expected: PASS.

- [x] **Step 3: Write failing coordinator/action tests**
  - Extend `submit_user_input_appends_chat_and_sends_action`: the action and projection contain identical refs.
  - Add attachment-only input acceptance.
  - Add stale-session test: run/pause changes during file copy -> copied files roll back, no action or chat row is added.
  - Add channel-closed rollback test.
  - Expected RED: coordinator accepts only text and performs no ingestion.

- [x] **Step 4: Implement follow-up ingestion without blocking the run mutex**
  - Lock only to snapshot run ID, root, awaiting node, and a session generation token.
  - Run validated copy/hashing in `spawn_blocking`.
  - Reacquire the mutex; revalidate the same run, node, and generation.
  - On success, enqueue one structured `ExecutionAction::ProvideInput` containing the accepted message and optional resolved skill prompt, then append the matching projection.
  - On stale state or send failure, remove only files created by this attempted message.
  - Update drive interaction to call `on_human_message`.
  - Run: `cargo nextest run -p orchestration --lib`
  - Expected: PASS.

- [x] **Step 5: Write failing UI send-path tests**
  - Active chat reply sends text, attachment sources, and skill IDs once.
  - Saved chat resumes, applies current runtime config, then submits the attachment message.
  - New workflow kickoff carries attachment sources into the entrypoint.
  - Manual-root flush sends the same structured payload once, without duplicating its user bubble.
  - Failed send retains text and attachments; accepted send clears both.
  - Replay renders attachment-only messages.
  - Expected RED: pending kickoff and send helpers carry strings only.

- [x] **Step 6: Centralize UI submission state**
  - Replace `resolveChatSubmittedText` with `resolveChatSubmissionPayload`.
  - Store pending attachments beside text drafts, keyed by active workflow/chat ID and node ID.
  - Replace `pendingKickoffText` with the structured payload.
  - Add `clearChatSubmission(nodeId)` and call it only after the backend accepts the message.
  - Preserve async selection results only when workflow/chat and node still match the initiating draft.
  - Update `canSendChat` and `canSendIdleRunKickoff` to accept `hasAttachments`.
  - Update `PlainMessage.shouldRender` to include attachment-only rows.
  - Run: `npm --prefix crates/ui run test -- src/lib/workflow/workflow.test.ts src/components/conversation/ConversationSegmentMessages.test.tsx src/app/App.test.tsx`
  - Expected: PASS.

- [x] **Step 7: Prove durable replay**
  - Backend test: start chat with image, pause, stop, load latest checkpoint, resume, submit another message, and assert both attachment refs remain.
  - UI test: reopen a chat from its durable run projection and request the preview by run/attachment ID.
  - Run: `cargo nextest run -p orchestration --test workflow_acceptance --no-capture`
  - Expected: PASS.

- [x] **Step 8: Verify the slice**
  - Run: `./scripts/test-fast.sh --execution --desktop`
  - Run: `./scripts/verify.sh ui-typecheck ui-test public-api arch`
  - Expected: every command PASS.

## Maintainability Gate

- [x] One structured message path covers entrypoint, reply, resume, and manual-root flush.
- [x] File I/O never runs while holding the coordinator Tokio mutex.
- [x] Action and projection update from one accepted message value.
- [x] Text-only callers stay source-compatible through wrappers.
- [x] UI success/failure behavior is tested through public handlers.

## Self-Review

- [x] Spec coverage: every current send path has a regression.
- [x] Placeholder scan: no unresolved path or state-ownership choice.
- [x] Type/name consistency: structured input, engine refs, and UI DTO mirror align.
- [x] Command quality: checkpoint/replay behavior has focused and acceptance coverage.

## Result

- Status: Complete.
- Verification: Coordinator attachment kickoff/reply regressions 39/39; workflow acceptance 11/11; browser reopen/preview E2E 2/2; full UI attachment suite 165/165.
- Notes: Replies submit one atomic message/ref/skill action after out-of-lock ingestion plus session-generation revalidation. Saved-chat resume hydrates the same durable refs; preview lookup covers active state and persisted replay.
