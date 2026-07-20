# Typed Mixed-Turn Error

## Goal
- Preserve the control/work safety boundary while exposing enough structured information for the engine to retry a mixed response.

## Current Question
- Question: Should mixed calls be executed selectively?
- Recommended answer: No. Reject the entire batch and execute none of its calls.
- Reason: A control submission and an executable call have different sequencing and side-effect semantics; selecting one would make provider behavior unsafe and nondeterministic.

## Codebase Findings
- `crates/providers/src/mapping/mod.rs` owns control/work tool classification and currently emits a plain `AgentError::Failed` for mixed calls.
- `crates/engine/src/ports/outbound.rs` owns provider-facing error vocabulary and retry classification.
- Provider mapping tests already cover mixed control/work batches.
- Focused command: `cargo test -p providers mapping`

## Ownership
- Modify: `crates/engine/src/ports/outbound.rs` for the typed error and helper.
- Modify: `crates/providers/src/mapping/mod.rs` to construct it.
- Test: colocated engine/provider unit tests.

## Steps
- [ ] **Step 1: Write the failing test**
  - Assert a mixed control response returns `AgentError::MixedToolTurn` with the provider label, phase, and sorted tool names.
- [ ] **Step 2: Verify RED**
  - Run `cargo test -p providers control_turn_rejects_hallucinated_executable_tool_calls`.
  - Expected: FAIL because the existing result is only `AgentError::Failed`.
- [ ] **Step 3: Implement minimal code**
  - Add the typed error, constructor, predicate, and provider mapping conversion.
- [ ] **Step 4: Verify GREEN**
  - Run `cargo test -p providers control_turn_rejects_hallucinated_executable_tool_calls`.
  - Expected: PASS.
- [ ] **Step 5: Refactor while green**
  - Keep formatting and error text compatible with the existing diagnostic.
- [ ] **Step 6: Verify slice**
  - Run `cargo test -p providers`.

## Maintainability Gate
- [ ] Reuses the existing `AgentError` boundary.
- [ ] Keeps provider-specific classification in `providers`.
- [ ] Does not weaken mixed-call rejection.

## Self-Review
- [ ] Error exposes names without response text or private reasoning.
- [ ] Existing error text remains actionable.
- [ ] No unrelated dirty files are changed.

## Result
- Status: Complete
- Verification: `cargo test -p providers control_turn_rejects_hallucinated_executable_tool_calls` passed; full provider suite remains pending.
- Notes: Mixed control-turn batches remain rejected, but now use a typed `AgentError` carrying the tool-name list for engine recovery.
