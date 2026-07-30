# Verify Provider Compatibility and Document the Feature

## Goal

- Prove the attachment feature across deterministic tests and selected live providers, record unsupported combinations accurately, update user docs, and pass the full repo gate.

## Current Question

- Question: Should deterministic tests claim every configured model supports images/documents?
- Recommended answer: No. Claim only serializer/wire support from tests and runtime acceptance from explicit live smoke.
- Reason: Transport capability does not guarantee the selected model accepts that media, and custom model IDs cannot be inferred safely.

## Codebase Findings

- Provider/model profiles have no input-media capability metadata.
- Existing provider tests use Wiremock; live AI smoke is opt-in through repo testing workflows.
- ChatGPT/Codex uses a Responses-style serializer, but private-backend media acceptance requires a live check.
- `docs/ROADMAP.md` currently marks attach button, drag/drop, structured payload, transcript refs, pills, and images as planned.
- Handoff gate: `./scripts/verify.sh`; Bedrock needs `./scripts/verify/test-providers-bedrock.sh`.

## Ownership

- Modify: `docs/ROADMAP.md` to mark only completed attachment items done.
- Modify: `docs/guides/using-the-app.md` with attach, paste/drop, limits, supported formats, model-error recovery, storage, and deletion behavior.
- Modify: `docs/reference/README.md` with run attachment storage layout and persisted-ref contract.
- Modify: `docs/troubleshooting/README.md` with unsupported media/model, missing/corrupt attachment, size/type rejection, and retry guidance.
- Modify: `docs/contributing/testing-workflows.md` with attachment live-smoke procedure.
- Create: `crates/desktop/e2e/tests/chat-attachments.spec.ts` for picker/import IPC mocks and durable rendering.
- Modify: `crates/desktop/e2e/ipcMocks.ts` with attachment command fixtures.
- Test: all focused suites from slices 1-4 plus full verification.

## Acceptance Matrix

| Path | Deterministic proof | Live proof |
| --- | --- | --- |
| OpenAI Responses image | Wiremock request body | One configured vision model |
| OpenAI Responses PDF/text | Wiremock request body | One configured document-capable model |
| OpenAI Chat Completions image/PDF | Wiremock request body | One compatible configured endpoint when available |
| Anthropic image/PDF/text | Wiremock request body | One configured Claude model |
| Bedrock image/PDF/text | Feature-gated conversion/test | One configured Bedrock Claude model when credentials exist |
| ChatGPT/Codex | Mocked Responses serializer | `gpt-5.6-luna` private-backend PNG/PDF smoke passed; other models remain unverified |
| Custom endpoint/model | Serializer unit test only | User/provider-dependent; actionable runtime error |

## Steps

- [x] **Step 1: Run the focused deterministic matrix**
  - Run: `cargo nextest run -p engine`
  - Run: `cargo nextest run -p providers`
  - Run: `./scripts/verify/test-providers-bedrock.sh`
  - Run: `cargo nextest run -p orchestration --lib`
  - Run: `cargo nextest run -p orchestration --test workflow_acceptance --no-capture`
  - Run: `cargo nextest run -p desktop`
  - Run: `npm --prefix crates/ui run test -- src/api.test.ts src/lib/workflow/workflow.test.ts src/components/conversation/ConversationComposer.test.tsx src/components/conversation/ConversationSegmentMessages.test.tsx src/components/conversation/MessageAttachments.test.tsx src/components/conversation/Message.test.tsx src/app/App.test.tsx`
  - Expected: every command PASS.

- [x] **Step 2: Add one desktop interaction regression**
  - Mock dialog selection plus attachment preview IPC.
  - Start a direct chat with an attachment-only image message.
  - Assert pending card -> accepted user image bubble -> saved chat reopen -> same preview.
  - Assert failed import keeps the pending card and displays the backend error.
  - Run: `npm --prefix crates/desktop/e2e run test:browser -- tests/chat-attachments.spec.ts`
  - Expected: PASS.

- [x] **Step 3: Run live media smoke where credentials exist**
  - Use a generated fixture PNG with unambiguous visual facts and a two-page PDF with unique facts.
  - For each available row in the acceptance matrix, send the fixture, ask a deterministic question, verify the answer uses the media, stop/reopen, then send a follow-up referring to the prior attachment.
  - Record provider, exact model, transport, date, result, and error text in the slice Result section.
  - Skip only rows lacking credentials or a configured compatible model; record the exact skip reason.

- [x] **Step 4: Update docs from proven behavior**
  - Mark completed roadmap rows done; leave line-range refs, assistant images, audio/video, HEIC/SVG, and unproven provider paths planned.
  - Document the supported formats and exact limits.
  - Document that sent files become managed run copies; moving the original does not break replay.
  - Document that chat deletion removes its run-owned attachment data.
  - State that an unsupported selected model returns an error; switching to a media-capable model then retrying is the recovery.
  - Do not claim custom models or Codex media support without a successful live result.
  - Rewrite any roadmap text that still claims attachments are limited to project-jail paths; picked sources become managed run copies.

- [x] **Step 5: Run full handoff verification**
  - Run: `./scripts/verify.sh`
  - Expected: all steps PASS.
  - If the shared dirty worktree produces a failure, reproduce the exact failing step, inspect concurrent changes, and attribute only after causal verification.

- [x] **Step 6: Record final plan state**
  - Mark each verified slice complete in `plans/chat-attachments/todo.md`.
  - Fill every slice `Result` with commands, outcomes, live-smoke matrix rows, and any explicitly deferred provider combinations.
  - Confirm no production placeholder, unchecked acceptance criterion, or orphan plan question remains.

## Maintainability Gate

- [x] Docs distinguish deterministic wire support from live model acceptance.
- [x] Full gate and architecture check pass.
- [x] E2E proves visible attachment state, not internal fn calls.
- [x] Roadmap marks only shipped behavior done.
- [x] Final results capture exact skips/failures.

## Self-Review

- [x] Spec coverage: all plan decisions map to tests/docs.
- [x] Placeholder scan: no vague verification or compatibility claim.
- [x] Type/name consistency: docs match UI and Rust DTO names where technical detail is useful.
- [x] Command quality: focused, feature-gated, E2E, live, and full-gate layers are present.

## Result

- Status: Complete.
- Verification: Focused matrices pass; Bedrock feature tests 179/179; browser E2E 2/2; full repo gate passes after a clippy-only repair rerun; `./scripts/smoke-live-attachments.sh` passes 2/2.
- Live matrix:

  | Date | Provider | Model | Transport | PNG | PDF | Durable replay |
  | --- | --- | --- | --- | --- | --- | --- |
  | 2026-07-29 | ChatGPT (Codex) (`openai-codex`) | `gpt-5.6-luna` | Responses API | PASS: `BLUE-3` | PASS: `EMBER-417`, `AXOLOTL` | PASS after stop/resume: PNG `MIDDLE`, PDF `7319` |

- Notes: The successful Codex row proves the saved account/model/transport combination only. Custom endpoints, Anthropic, Bedrock, and other models remain live-unverified because no corresponding credentials/configured model were available. Deterministic serializer suites cover their documented wire shapes.
