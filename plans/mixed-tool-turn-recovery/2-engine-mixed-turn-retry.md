# Engine Mixed-Turn Retry

## Goal
- Convert a rejected mixed tool response into one bounded correction-and-retry instead of an immediate node failure.

## Current Question
- Question: Should the rejected calls be replayed on retry?
- Recommended answer: No; retry from the existing transcript and tell the model that no calls from the rejected response executed.
- Reason: Replaying executable calls could duplicate side effects, while the provider response itself was never accepted by the engine.

## Codebase Findings
- `crates/engine/src/execution/interactive_engine/completion.rs` handles typed protocol retries and transcript feedback.
- `crates/engine/src/execution/interactive_engine/checkpoint.rs` persists per-node retry maps.
- `crates/engine/src/execution/interactive_engine/mod.rs` owns recovery constants and state initialization.
- Existing retry tests use inline `AiPort` stubs.

## Ownership
- Modify: `crates/engine/src/ports/outbound.rs` for retry classification data access.
- Modify: `crates/engine/src/execution/interactive_engine/{mod.rs,completion.rs,checkpoint.rs}` for bounded retry state.
- Test: `crates/engine/src/execution/interactive_engine/tests.rs`.

## Steps
- [x] **Step 1: Write the failing test**
  - Feed a mixed-turn error followed by a valid completion and assert the engine appends correction feedback, re-invokes, and completes without executing the rejected calls.
- [x] **Step 2: Verify RED**
  - Run `cargo test -p engine mixed_tool_turn`.
  - Expected: FAIL because no mixed-turn retry handler exists.
- [x] **Step 3: Implement minimal code**
  - Add a small retry budget, correction text naming the rejected tools, checkpoint persistence, and handler ordering before terminal failure.
- [x] **Step 4: Verify GREEN**
  - Run `cargo test -p engine mixed_tool_turn`.
  - Expected: PASS.
- [x] **Step 5: Refactor while green**
  - Reuse existing transcript and recovery reset patterns; avoid a second generic retry framework.
- [x] **Step 6: Verify slice**
  - Run `cargo test -p engine`.

## Maintainability Gate
- [x] Retry is bounded and persisted.
- [x] Rejected tool calls are never dispatched.
- [x] Correction stays in engine semantics, not provider code.

## Self-Review
- [x] Retry clears its budget after successful completion.
- [x] Retry behavior survives checkpoint serialization.
- [x] Terminal error remains available after the budget is exhausted.

## Result
- Status: Complete
- Verification:
  - `CARGO_NET_OFFLINE=true CARGO_TARGET_DIR=/tmp/openflow-mixed-turn-target cargo test -p engine mixed_tool_turn --lib` — 3 passed.
  - `CARGO_NET_OFFLINE=true CARGO_TARGET_DIR=/tmp/openflow-mixed-turn-target cargo test -p engine --lib` — 142 passed.
- Notes:
  - The provider still rejects mixed batches before dispatch; the engine now retries from the existing transcript with a persisted three-attempt budget.
