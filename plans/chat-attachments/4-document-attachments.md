# Add PDF and UTF-8 Document Attachments

## Goal

- Users can attach PDF, TXT, Markdown, CSV, JSON, HTML, CSS, JavaScript, and Python files; supported provider transports receive them as document/text content, and the chat shows compact durable file cards.

## Current Question

- Question: Should OpenFlow accept arbitrary files and let the provider decide?
- Recommended answer: No. Allow only provider-mappable documents in this slice.
- Reason: Arbitrary binaries create privacy, memory, and inconsistent-provider failures. A narrow MIME allowlist gives deterministic validation and actionable errors.

## Codebase Findings

- Rig 0.39 has `UserContent::Document` and `DocumentMediaType` for PDF, TXT, RTF, HTML, CSS, Markdown, CSV, XML, JavaScript, and Python.
- Current Rig serializers differ:
  - OpenAI Responses and Chat Completions accept PDF data; text-like documents can be flattened as text.
  - Anthropic accepts PDF plus TXT-compatible content.
  - Bedrock accepts image/document blocks but applies stricter media rules.
- `CompletionRequest.documents` is retrieval/static context and is not the user-attachment path.
- Current `@{path}` support reads project files and inlines text; it must remain unchanged.
- Focused test commands:
  - `cargo nextest run -p providers`
  - `./scripts/verify/test-providers-bedrock.sh`
  - `cargo nextest run -p orchestration --lib run_attachment_store`
  - `npm --prefix crates/ui run test -- src/components/conversation/ConversationComposer.test.tsx src/components/conversation/MessageAttachments.test.tsx`

## Ownership

- Modify: `crates/orchestration/src/adapters/storage/run_attachment_store.rs` for PDF magic-byte and UTF-8 document validation.
- Modify: `crates/engine/src/conversation/mod.rs` only if the existing `ChatAttachmentKind::Document` needs no additional field.
- Modify: `crates/providers/src/rig_adapter/{convert.rs,model.rs}` for document mapping and preflight errors.
- Modify: `crates/providers/tests/{rig_openai_compat.rs,rig_anthropic.rs,rig_codex.rs}` and feature-gated Bedrock tests.
- Modify: `crates/ui/src/api.ts`, `crates/ui/src/components/conversation/{ConversationComposer.tsx,MessageAttachments.tsx}`, and `crates/ui/src/styles/chat.css`.
- Test: storage inline tests, provider unit/wire tests, composer tests, and attachment renderer tests.

## Contract

- Accept MIME/signature combinations:
  - `application/pdf` from `%PDF-` bytes.
  - UTF-8 text for `.txt`, `.md`, `.markdown`, `.csv`, `.json`, `.html`, `.htm`, `.css`, `.js`, `.mjs`, `.cjs`, and `.py`.
- Map listed formats lacking a native provider document type, including JSON, to a named text block. Reject unlisted extensions even when bytes are UTF-8.
- Keep the original sanitized filename in a short text prefix so the model can distinguish multiple documents.
- Do not render PDF/HTML/SVG inline in the webview. Show filename, type, and size card only.
- On an unsupported provider/model response, retain the message and ref, stop with a clear error, and let the user switch model/retry. Do not guess capability from the model string.

## Steps

- [x] **Step 1: Write failing document validation tests**
  - Accept valid PDF header and UTF-8 text extensions.
  - Reject fake `.pdf`, invalid UTF-8 text, embedded NUL-heavy binary, DOCX, RTF, SVG, HEIC, audio, and video.
  - Enforce the same count and byte limits as images.
  - Expected RED: storage accepts image signatures only.

- [x] **Step 2: Implement the document allowlist**
  - Sniff PDF bytes and validate UTF-8 text content.
  - Normalize media type and `ChatAttachmentKind::Document`.
  - Reuse atomic copy, hash, rollback, and safe filename behavior.
  - Do not generate a thumbnail for documents.
  - Run: `cargo nextest run -p orchestration --lib run_attachment_store`
  - Expected: PASS.

- [x] **Step 3: Write failing provider multipart tests**
  - OpenAI Responses: PDF becomes `input_file` data; UTF-8 document becomes named input text.
  - OpenAI Chat Completions: PDF and text-like content use the transport-supported shape.
  - Anthropic: PDF uses a base64 document source; text-like content uses a named text block.
  - Codex: mocked Responses serializer emits the expected attachment request shape; mark actual private-backend acceptance for live smoke.
  - Bedrock: PDF/TXT conversion succeeds under the feature-gated suite and includes required accompanying text.
  - Expected RED: only image payloads map.

- [x] **Step 4: Implement provider-aware document mapping**
  - Encode PDF bytes at the provider boundary.
  - Decode validated UTF-8 files once and emit named text/document content.
  - Return `AgentError::Permanent` before HTTP for a media/transport combination the adapter cannot represent.
  - Map provider unsupported-media responses to an error that names provider, model, and file type without including file content.
  - Run: `cargo nextest run -p providers`
  - Run: `./scripts/verify/test-providers-bedrock.sh`
  - Expected: PASS.

- [x] **Step 5: Write failing document UI tests**
  - Picker filter includes the exact document extensions.
  - Selected PDF/text files render compact cards with filename and formatted size.
  - Remove control remains keyboard accessible.
  - Sent/replayed documents never create `<iframe>`, raw HTML, or inline SVG.
  - Expected RED: UI recognizes images only.

- [x] **Step 6: Implement document cards**
  - Expand picker filters and pending-card icon/labels.
  - Render sent docs in `MessageAttachments` as metadata cards.
  - Keep image preview loading isolated to `kind === "image"`.
  - Run: `npm --prefix crates/ui run test -- src/components/conversation/ConversationComposer.test.tsx src/components/conversation/MessageAttachments.test.tsx`
  - Expected: PASS.

- [x] **Step 7: Verify the slice**
  - Run: `./scripts/test-fast.sh --execution --desktop`
  - Run: `./scripts/verify.sh ui-typecheck ui-test arch`
  - Expected: every command PASS.
  - Manual smoke: send one PDF plus one Markdown file, ask for a comparison, reopen the chat, and confirm both file cards remain.

## Maintainability Gate

- [x] Images and documents reuse one ref/storage/submission pipeline.
- [x] Provider-specific representation stays in providers.
- [x] No active-content document format renders inline.
- [x] `@{path}` behavior remains intact and separately tested.
- [x] Unsupported media fails before request when the transport cannot represent it.

## Self-Review

- [x] Spec coverage: validation, wire mapping, UI cards, replay, and failure.
- [x] Placeholder scan: exact accepted/rejected formats are listed.
- [x] Type/name consistency: one `Document` kind crosses the stack.
- [x] Command quality: every supported provider family has mapping coverage.

## Result

- Status: Complete.
- Verification: Provider base matrix 163/163; Bedrock-feature matrix 179/179; storage document tests pass; focused document-card UI tests pass.
- Notes: PDF and allowlisted UTF-8 docs share the image ref/storage pipeline. OpenAI Responses and Anthropic wire bodies have deterministic coverage. UI renders inactive metadata cards only.
