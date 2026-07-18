# Slice 2: Preserve Pre-Rig Malformed Arguments

## Goal

- Ensure malformed non-streaming OpenAI Chat Completions and Responses API tool arguments reach OpenFlow as typed repair candidates instead of becoming generic Rig JSON errors with the raw value lost.

## Current Question

- Question: None.
- Recommended answer: Add a local `HttpClientExt` response normalizer, following the existing Anthropic compatibility-client pattern, and carry unrecoverable argument strings through a private marker recognized by provider mapping.
- Reason: Rig 0.39 deserializes stringified tool arguments into `serde_json::Value` before OpenFlow's mapping code; a local response wrapper is the narrowest in-repo seam that retains the raw candidate.

## Codebase Findings

- `crates/providers/src/rig_adapter/model.rs` constructs concrete Rig OpenAI Chat and Responses models over `reqwest::Client`.
- `crates/providers/src/rig_adapter/anthropic_http.rs` demonstrates a local `HttpClientExt` wrapper that normalizes response bytes before Rig deserializes them.
- `crates/providers/src/rig_adapter/error.rs` currently maps Rig JSON failures to generic `AgentError::Failed`.
- `crates/providers/src/rig_adapter/outcome.rs::resolve_outcome_raw_tool_call` proves raw-string deterministic repair, but it has no production caller.
- Custom OpenAI-compatible invocation is already non-streaming, which covers the observed `minimax-m3` failure lane.
- Test command: `cargo test -p providers rig_adapter::openai_http --lib -- --nocapture`

## Ownership

- Create: `crates/providers/src/rig_adapter/openai_http.rs` for non-streaming OpenAI response normalization and private malformed-argument marker creation.
- Modify: `crates/providers/src/rig_adapter/mod.rs` to register the module.
- Modify: `crates/providers/src/rig_adapter/model.rs` to build both OpenAI Chat and Responses Rig models over the wrapper.
- Modify: `crates/providers/src/mapping/mod.rs` to consume the private marker and construct the slice-1 repair candidate.
- Modify: `crates/providers/src/rig_adapter/outcome.rs` only where candidate metadata must survive outcome conversion.
- Test: inline byte-level fixtures in `openai_http.rs`.
- Test: `crates/providers/tests/rig_openai_compat.rs` for real HTTP-boundary Chat and Responses fixtures.
- Test: `crates/providers/tests/rig_anthropic.rs` for structurally invalid but parseable `tool_use.input` classification.

## Marker and Safety Rules

- For each response tool call, parse a stringified `arguments` value normally, then with `jsonrepair-rs`.
- If deterministic repair succeeds, write the repaired JSON back into the response body and let Rig continue normally; no overseer candidate is created.
- If deterministic repair fails, replace only that arguments field with a reserved private marker containing a 64 KiB-capped raw string and a sanitized parse detail.
- Recognize and remove the marker in provider mapping before any call can become executable.
- Never serialize the marker into transcripts, checkpoints, logs, errors, or telemetry.
- Do not reinterpret a general response-body JSON failure as a repairable tool call; unrelated response JSON remains a normal provider error.
- Do not add SSE fragment reconstruction in this slice. The current observed custom-compatible path is non-streaming; streaming recovery remains explicitly deferred.

## Steps

- [x] **Step 1: Write failing raw-boundary tests**
  - Add Chat Completions and Responses API bodies whose outer response JSON is valid but whose stringified function arguments are invalid.
  - Add a deterministic trailing-comma fixture that should repair locally without producing a candidate.
  - Add a malformed general response fixture that must stay a generic provider failure.
- [x] **Step 2: Verify RED**
  - Run: `cargo test -p providers --lib rig_adapter::openai_http -- --nocapture`
  - Expected: FAIL because the OpenAI compatibility wrapper and marker extractor do not exist.
- [x] **Step 3: Implement response normalization**
  - Traverse Chat `choices[].message.tool_calls[].function.arguments` and Responses `output[type=function_call].arguments`.
  - Reuse the same deterministic parsing order as provider mapping.
  - Cap stored raw arguments before creating the private marker.
  - Change the concrete Rig model client types and builders to use the wrapper without exposing it outside `rig_adapter`.
- [x] **Step 4: Verify GREEN at the byte and mapping seams**
  - Run: `cargo test -p providers --lib rig_adapter::openai_http -- --nocapture`
  - Run: `cargo test -p providers --lib mapping::tests:: -- --nocapture`
  - Expected: PASS; deterministic repair stays local, unrecoverable arguments become typed and redacted, unrelated JSON failures remain unrepairable.
- [x] **Step 5: Verify actual provider HTTP boundaries**
  - Run: `cargo test -p providers --test rig_openai_compat --test rig_anthropic -- --nocapture`
  - Expected: PASS for Chat, Responses, and Anthropic structural classification fixtures.
  - Environment note: if Wiremock fails with `Operation not permitted` while binding a local port, rerun the same command with local-port permission; record that as an environment restriction, not a product failure.
- [x] **Step 6: Verify the provider lane**
  - Run: `cargo test -p providers`
  - Run: `cargo clippy -p providers --all-targets -- -D warnings`
  - Expected: PASS, or record only pre-existing unrelated dirty-file clippy failures with exact paths.

## Maintainability Gate

- [x] The wrapper performs byte-shape compatibility only; overseer policy does not enter `providers`.
- [x] Chat and Responses traversal share one argument-normalization helper.
- [x] The reserved marker is private, versioned, and consumed before outcome execution.
- [x] Raw data is capped and redacted on every formatted surface.
- [x] Provider-specific behavior is covered at the real HTTP boundary.

## Self-Review

- [x] Every repairable classification originates from a confirmed tool-argument field.
- [x] No general JSON parse failure is mislabeled as repairable.
- [x] Deterministic repair tests prove the overseer will not be invoked unnecessarily.
- [x] Streaming limitations are explicit and do not silently claim coverage.
- [x] No vague implementation placeholders remain.

## Result

- Status: Done
- Verification:
  - `cargo test -p providers --lib openai_http` — 4 passed
  - `cargo test -p providers --lib mapping::tests::` — 26 passed
  - `cargo test -p providers --test rig_openai_compat --test rig_anthropic` — 22 passed
  - `cargo test -p providers` — 124 passed
  - `cargo clippy -p providers --all-targets -- -D warnings` — clean
- Notes: Concurrent incomplete Plan Mode / `ToolAccessPolicy` dirty tree required minimal compile fixes (default `Execution` policy at request sites, checkpoint/hash glue). Streaming SSE recovery remains deferred.
