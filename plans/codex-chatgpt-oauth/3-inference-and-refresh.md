# Slice 3: Inference and Refresh

## Goal

- Add a refreshable Codex `AiPort` implementation that reuses Rig's ChatGPT Responses transport, persists credential rotation, and retries exactly one unauthorized request.

## Current Question

- Question: None.
- Recommended answer: Build a fresh Rig ChatGPT model for each invocation after credential resolution, while reusing the HTTP client connection pool.
- Reason: OpenFlow's existing model cache assumes immutable authentication; rebuilding the lightweight model avoids stale bearer headers after refresh.

## Codebase Findings

- Rig 0.39's ChatGPT provider already sends the Codex Responses path, required account/session/originator headers, `store: false`, `stream: true`, encrypted reasoning inclusion, and maps Responses SSE/tool events.
- OpenFlow's generic reasoning parameters are flat; Codex requires nested `reasoning: { effort, summary: "auto" }`.
- Rig's OAuth storage/refresh is not usable for this feature, and it does not perform OpenFlow's required post-401 retry.
- Current SSE supports `response.completed`, failed, and incomplete. A `response.done` compatibility shim is justified only if a fixture or live smoke proves the SSE backend requires it.
- Test command: `cargo test -p providers codex -- --nocapture`

## Ownership

- Create: `crates/providers/src/codex.rs` for credential management, proactive refresh, sink persistence, retry policy, and `AiPort` dispatch.
- Modify: `crates/providers/src/rig_adapter/model.rs` to add a Rig ChatGPT model path and Codex-specific nested reasoning.
- Modify: `crates/providers/src/rig_adapter/error.rs` for safe authentication/quota messages when Rig's error is too generic.
- Modify: `crates/providers/src/client.rs` and `crates/providers/src/lib.rs` to dispatch `OpenAiCodex` without changing existing provider paths.
- Test: provider unit tests and Wiremock request/stream integration tests.

## Contract Detail

- Before each invoke or stream, refresh credentials when expiry is within five minutes; persist the complete rotated credential set before making the request.
- On one 401-equivalent provider error, force refresh, persist, rebuild the Rig model, and replay once. Never retry a second unauthorized response.
- A mutex or equivalent single-flight guard prevents concurrent workflow nodes from racing refresh-token rotation.
- The request uses the ChatGPT Codex base/path, access token, account ID, `originator: openflow`, nested reasoning, `store: false`, streaming, and encrypted reasoning inclusion.
- Do not send the old `OpenAI-Beta: responses=experimental` header on the SSE path.
- Preserve tool-call IDs, tool-result IDs, usage, reasoning deltas, and existing OpenFlow turn-outcome behavior.

## Steps

- [ ] **Step 1: Write failing request/stream tests**
  - Prove exact path, auth/account/originator headers, body transforms, nested reasoning, text/reasoning/tool-call mapping, usage, and multi-turn call-ID preservation.
- [ ] **Step 2: Write failing lifecycle tests**
  - Prove proactive refresh, rotated refresh-token persistence, concurrent single-flight refresh, one-shot 401 recovery, and clear auth/quota errors.
- [ ] **Step 3: Verify RED**
  - Run: `cargo test -p providers codex -- --nocapture`
  - Expected: FAIL because Codex dispatch and lifecycle logic do not exist.
- [ ] **Step 4: Implement Rig-backed Codex dispatch**
  - Add the ChatGPT model variant and request construction.
  - Add credential resolution, refresh, sink persistence, and one-shot retry.
  - Add only the narrow error mapping required by contract tests.
- [ ] **Step 5: Verify GREEN**
  - Run: `cargo test -p providers codex -- --nocapture`
  - Run: `cargo test -p providers --lib rig_adapter -- --nocapture`
  - Run: `cargo clippy -p providers --all-targets -- -D warnings`
  - Expected: PASS without regressions to OpenAI, Anthropic, or Bedrock adapters.

## Maintainability Gate

- [ ] Request/SSE mapping is reused rather than forked.
- [ ] Refresh is single-flight and bounded.
- [ ] New credentials are durable before a retry uses them.
- [ ] No stale-token model cache remains on the Codex path.
- [ ] Private-backend assumptions are fixture-tested and documented.

## Result

- Status: Pending
- Verification: Not run.

