# Prompt and Acceptance Coverage

## Goal
- Reduce recurrence for large artifact nodes and prove the control/work protocol through existing acceptance lanes.

## Current Question
- Question: Where should the sequencing guidance live?
- Recommended answer: Put the durable rule in the engine runtime preamble and add a focused provider/engine acceptance test.
- Reason: Every node receives the preamble, while provider-specific prompts alone would miss other adapters.

## Codebase Findings
- `crates/engine/src/execution/node_invocation.rs` owns `NODE_RUNTIME_PREAMBLE`.
- `crates/providers/src/mapping/mod.rs` already asserts disjoint control/work tool catalogs.
- `crates/orchestration/tests/workflow_acceptance.rs` verifies real engine turn sequencing.
- Artifact-backed output guidance already exists in the OPE-65 branch and should remain repository-relative.

## Ownership
- Modify: `crates/engine/src/execution/node_invocation.rs` for explicit one-tool-per-turn and compact-submit guidance.
- Test: `crates/engine/src/execution/node_invocation.rs` and `crates/orchestration/tests/workflow_acceptance.rs` as needed.
- Update: this plan with focused and broader verification results.

## Steps
- [x] **Step 1: Write the failing test**
  - Assert the runtime preamble mentions separate control/work turns, no mixed calls, and path-based submission for large artifacts.
- [x] **Step 2: Verify RED**
  - Run `cargo test -p engine runtime_preamble`.
  - Expected: FAIL for the missing wording.
- [x] **Step 3: Implement minimal code**
  - Add concise guidance without duplicating the provider implementation.
- [x] **Step 4: Verify GREEN**
  - Run `cargo test -p engine runtime_preamble`.
  - Expected: PASS.
- [x] **Step 5: Refactor while green**
  - Keep the preamble compact and consistent with existing tool guidance.
- [x] **Step 6: Verify slice**
  - Run `cargo test -p orchestration --test workflow_acceptance -- --nocapture`.

## Maintainability Gate
- [x] Prompt explains behavior rather than naming a provider.
- [x] Large artifacts are referenced by repository-relative path.
- [x] Acceptance coverage exercises the real turn state machine.

## Self-Review
- [x] No full artifact content is encouraged in submit arguments.
- [x] Existing prompt tests remain valid.
- [x] Unrelated dirty changes remain untouched.

## Result
- Status: Complete
- Verification:
  - `CARGO_NET_OFFLINE=true CARGO_TARGET_DIR=/tmp/openflow-mixed-turn-target cargo test -p engine runtime_preamble --lib` — 2 passed.
  - `CARGO_NET_OFFLINE=true CARGO_TARGET_DIR=/tmp/openflow-orch-target cargo test -p orchestration --test workflow_acceptance -- --nocapture` — 8 passed.
  - `CARGO_NET_OFFLINE=true CARGO_TARGET_DIR=/tmp/openflow-provider-target cargo test -p providers --lib` — 95 passed.
- Notes:
  - The provider suite required local-port permission for Wiremock; it passed after that permission was granted.
