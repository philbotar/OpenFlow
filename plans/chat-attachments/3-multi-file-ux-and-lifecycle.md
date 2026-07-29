# Add Multi-File UX and Lifecycle Safety

## Goal

- Users can pick, paste, or drop up to four supported images, reorder-free preserve selection order, remove drafts, recover from errors, and delete a chat without leaving private attachment files behind.

## Current Question

- Question: What limits should the first release enforce?
- Recommended answer: 4 files/message, 10 MiB/file, 25 MiB total.
- Reason: These limits keep local copies, hashing, preview generation, base64 expansion, and provider request memory bounded while covering normal screenshots and photos.

## Codebase Findings

- `@tauri-apps/plugin-dialog` already supports filtered multi-select through `api.ts::openNativeDialog`.
- No existing drag/drop, paste-image, attachment progress, or capability DTO exists.
- `ChatCatalog::delete` currently removes chat metadata only; durable run data remains.
- `start_chat_with_skill_ids` stops a run when attaching the run ID to chat metadata fails but does not remove the created run directory.
- `clear_artifact_root` removes `artifacts/` only; sibling `attachments/` survives stop/resume as required.
- Focused test commands:
  - `cargo nextest run -p orchestration --lib run_attachment_store`
  - `cargo nextest run -p orchestration --lib backend`
  - `npm --prefix crates/ui run test -- src/components/conversation/ConversationComposer.test.tsx src/app/App.test.tsx`

## Ownership

- Modify: `crates/orchestration/src/adapters/storage/run_attachment_store.rs` for batch validation, rollback, corruption checks, and cleanup.
- Modify: `crates/orchestration/src/run/ports.rs` and `crates/orchestration/src/adapters/storage/run_checkpoint_store.rs` for attachment/run deletion.
- Modify: `crates/orchestration/src/backend/chat.rs` and `crates/orchestration/src/chat.rs` for chat deletion orchestration.
- Modify: `crates/orchestration/src/error.rs` for actionable attachment errors.
- Modify: `crates/desktop/src/commands/{attachment.rs,chat.rs}`.
- Modify: `crates/ui/src/api.ts`, `crates/ui/src/context/appProvider/useChatComposer.ts`, `crates/ui/src/components/conversation/ConversationComposer.tsx`, and `crates/ui/src/styles/chat.css`.
- Modify: `crates/ui/src/context/appProvider/useWorkspaceCatalog.ts` for cleanup-pending delete results and warning reload behavior.
- Test: storage inline tests, `crates/orchestration/src/backend/tests.rs`, `crates/ui/src/components/conversation/ConversationComposer.test.tsx`, and `crates/ui/src/app/App.test.tsx`.

## Contract

- Backend owns all count/type/size enforcement; UI mirrors limits for fast feedback.
- Validation is all-or-nothing per message. If any file fails, no file from the batch remains and no message is accepted.
- Source paths remain volatile and disappear after accepted send or explicit removal.
- Selection order controls provider content order.
- Duplicate paths in one draft collapse to one item; files with equal content but different paths remain distinct.
- Preview failures show a generic file card; they do not remove a valid attachment.
- Chat deletion removes its direct-chat run directory, including attachments, checkpoints, and artifacts, after confirming the run is not active.
- Staged paste/drop bytes live under `{data_local}/openflow/attachment-staging/` as UUID plus validated extension. IPC accepts bounded base64, returns an opaque staging token plus display metadata, and orchestration resolves/removes the token.
- First staging use removes staging files older than 24 hours. First app bootstrap retries removal of stale staging and quarantined direct-chat run dirs.

## Steps

- [x] **Step 1: Write failing batch-storage safety tests**
  - Accept four valid files in order.
  - Reject a fifth file.
  - Reject per-file and total-byte overflow while streaming, not from metadata alone.
  - Reject symlink, directory, empty file, unsupported signature, and post-copy hash mismatch.
  - Inject failure on item N and assert items 1..N are rolled back.
  - Expected RED: store supports only the first-slice single image.

- [x] **Step 2: Implement atomic batch ingestion**
  - Validate count and declared aggregate size before copy.
  - Enforce actual byte limits during each copy.
  - Maintain a created-path list and remove it on any error.
  - Verify stored hash on every AI hydration and preview read.
  - Return errors naming the display file and violated policy without exposing the full source path.
  - Run: `cargo nextest run -p orchestration --lib run_attachment_store`
  - Expected: PASS.

- [x] **Step 3: Write failing composer interaction tests**
  - Multi-select adds up to four ordered cards.
  - Remove button is named `Remove {filename}` and returns focus to the composer.
  - Paste an image from `ClipboardEvent.clipboardData.items`.
  - Drop files on the composer and expose a visible/accessible drop state.
  - Duplicate path does not duplicate the card.
  - Disabled/replay composer rejects picker, paste, and drop.
  - A backend validation error leaves the draft intact and uses the existing error toast.
  - Expected RED: only one picker-selected path is supported.

- [x] **Step 4: Implement multi-select, paste, and drop**
  - Enable multi-select in the dialog helper with the accepted image filters.
  - Add one shared `addPendingAttachments` fn for picker, paste, and drop.
  - For browser `File` values without a durable path, send bytes only to a bounded desktop staging command, then treat its returned staged path/token like a pending source; remove staged data on draft removal or expiry.
  - Add drag depth tracking so child enter/leave events do not flicker the drop state.
  - Preserve Enter send and Shift+Enter newline behavior.
  - Announce accepted/rejected counts through `aria-live="polite"`.
  - Run: `npm --prefix crates/ui run test -- src/components/conversation/ConversationComposer.test.tsx src/app/App.test.tsx`
  - Expected: PASS.

- [x] **Step 5: Write failing start rollback and delete tests**
  - Chat start failure after run creation removes the new run directory.
  - `attach_run_with_title` failure removes the run and attachment copies.
  - Deleting an inactive direct chat removes chat metadata and its owned run tree.
  - Deleting the active chat remains blocked and keeps all files.
  - A failure before metadata deletion restores the run directory and leaves the chat intact. A final quarantine-removal failure returns a partial-success result so the UI reloads the deleted chat list and warns that local cleanup is pending.
  - Expected RED: current deletion leaves run data and start rollback leaves directories.

- [x] **Step 6: Implement recoverable lifecycle cleanup**
  - Reuse exact-run deletion added for slice-1 rollback.
  - Before chat metadata deletion, rename the owned run directory to a sibling quarantine name.
  - Delete chat metadata; if it fails, restore the directory name.
  - After metadata succeeds, remove the quarantine directory. If final removal fails, return a path-redacted `ChatDeleteResult::DeletedCleanupPending`, reload the chat list, and show a warning rather than claiming full cleanup.
  - Keep workflow run deletion out of scope; apply this cascade only to direct chat deletion.
  - On app startup or first staging use, remove staged paste/drop files older than 24 hours and retry quarantined direct-chat run cleanup.
  - Run: `cargo nextest run -p orchestration --lib`
  - Expected: PASS.

- [x] **Step 7: Verify the slice**
  - Run: `./scripts/test-fast.sh --execution --desktop`
  - Run: `./scripts/verify.sh ui-typecheck ui-test arch`
  - Expected: every command PASS.
  - Manual smoke: pick two images, remove one, paste another, send, reopen, then delete the chat and confirm its attachment directory is gone.

## Maintainability Gate

- [x] One add fn serves picker, paste, and drop.
- [x] Backend remains authoritative for all security limits.
- [x] Batch failures and stale-run races leave no partial message/files.
- [x] Direct-chat deletion has recoverable ordering.
- [x] Staged clipboard/drop bytes expire automatically.

## Self-Review

- [x] Spec coverage: selection, removal, failure retention, send, reopen, and delete.
- [x] Placeholder scan: limits and rollback semantics are concrete.
- [x] Type/name consistency: pending, staged, stored, and projected attachments stay distinct.
- [x] Command quality: storage fault cases and UI interactions have focused commands.

## Result

- Status: Complete.
- Verification: Attachment-store validation/rollback/staging 6/6; orchestration lib 582/582; focused UI 165/165; browser success/failure lifecycle E2E 2/2.
- Notes: UI supports ordered picker/paste/drop drafts and clears only after acceptance. Backend enforces 4 files, 10 MiB each, 25 MiB total. Direct-chat deletion quarantines the exact run, restores on metadata failure, and retries pending cleanup.
