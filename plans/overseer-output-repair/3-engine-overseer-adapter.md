# Slice 3: Engine-Owned Overseer AI Adapter

## Goal

- Add a bounded `RepairingAiPort` that invokes the run's provider with the configured workflow overseer model, or the originating request model when unset, in a fresh repair-only context and returns only candidates accepted by the slice-1 completion protocol.

## Current Question

- Question: None.
- Recommended answer: Reuse the same inner `AiPort` for worker and overseer roles, with an optional per-workflow model override and an originating-model fallback, while isolating the overseer request from the worker transcript and tools.
- Reason: This gives users a reliability lever without adding another credential or data-sharing boundary and keeps a future separate overseer provider as a wiring-only extension.

## Codebase Findings

- `crates/engine/src/ports/outbound.rs::AiPort` already supports both regular and streaming invocation and can be decorated without adding another port trait.
- `crates/engine/src/graph/workflow.rs::WorkflowSettings` already owns persisted run-level provider and reasoning defaults, so it is the correct owner for an optional repair-model choice.
- `InteractiveEngine` and `ToolPortImpl` both consume the same `AiPort` path once orchestration wires the decorator before `AiInvocationAdapter`.
- `AiStreamEvent` currently carries assistant and thinking deltas; it is the existing route from an AI adapter to orchestration run telemetry.
- `crates/engine/src/execution/subagent_runtime.rs` fails immediately on final `AgentError`, so a port decorator is the narrow seam that gives nodes and subagents parity.
- Test command: `cargo test -p engine output_repair -- --nocapture`

## Ownership

- Create: `crates/engine/src/execution/output_repair.rs` for `RepairingAiPort`, repair request construction, eligibility guards, and repaired-candidate acceptance.
- Modify: `crates/engine/src/graph/workflow.rs` to add `output_repair_model: Option<String>`, serialized as `outputRepairModel` and accepting `output_repair_model` as an alias, with `None` as the backward-compatible default.
- Modify: `crates/engine/src/execution/mod.rs` and `crates/engine/src/lib.rs` to export the decorator.
- Modify: `crates/engine/src/ports/outbound.rs` to add non-content repair lifecycle variants to `AiStreamEvent`.
- Test: inline scripted `AiPort` tests in `output_repair.rs`.
- Test: `crates/engine/tests/snapshots/public_api.txt` for the intentional decorator and stream-event surface.

## Runtime Contract

- Call the primary inner port normally, preserving streaming behavior.
- Intercept only a slice-1 malformed-submit error whose candidate is repairable and at most 64 KiB.
- Emit `OutputRepairStarted` through the stream sink without exposing raw arguments.
- Construct `OutputRepairPolicy` once per run. Normalize a blank `output_repair_model` to `None` at this boundary.
- Build a synthetic `AgentRequest` with:
  - the same workflow ID, a repair-scoped node ID, and model `policy.model.unwrap_or(originating_request.model)`;
  - one fixed system instruction that treats candidate text as untrusted data and forbids inventing facts;
  - input containing only malformed arguments, sanitized validation detail, fixed tool name, and expected schema;
  - no external tools, no transcript, no user-input permission, and no repository context;
  - an output schema requiring exactly one `repaired_arguments` object.
- Invoke the raw inner port directly with non-streaming `invoke`; never call the decorator recursively.
- Accept only a completed repair turn with `repaired_arguments` that passes the shared completion protocol.
- Preserve the original call ID and name. Clear overseer assistant prose and private reasoning.
- On invalid repair, cancellation, timeout, truncation, or oversize input, emit a sanitized failure event and return the original primary error so existing engine retry behavior remains unchanged.
- Set `MAX_OUTPUT_REPAIR_ATTEMPTS_PER_INVOCATION` to one.

## Steps

- [x] **Step 1: Write failing decorator tests**
  - Cover successful repair, configured-model precedence, originating-model fallback, blank-model normalization, schema-invalid overseer output, an overseer error, a second malformed overseer response, truncation bypass, 64 KiB guard, cancellation, one-attempt bound, and absence of the originating transcript/tools in the synthetic request.
  - Add `WorkflowSettings` serde tests proving `outputRepairModel` round-trips and missing or snake-case values deserialize compatibly.
  - Prove failure returns the original primary error rather than the overseer error.
- [x] **Step 2: Verify RED**
  - Run: `cargo test -p engine output_repair -- --nocapture`
  - Expected: FAIL because `RepairingAiPort` and repair lifecycle stream events do not exist.
- [x] **Step 3: Implement minimal decorator behavior**
  - Add `RepairingAiPort<A>` over a shared inner `AiPort` and an immutable `OutputRepairPolicy`.
  - Implement `invoke` and `invoke_stream` through one shared repair algorithm.
  - Resolve the synthetic request model from the policy with the originating request model as fallback, build the isolated request, and revalidate through `completion_protocol`.
  - Keep all constants and prompt text local to `output_repair.rs`.
- [x] **Step 4: Verify GREEN**
  - Run: `cargo test -p engine output_repair -- --nocapture`
  - Expected: PASS; exactly one repair call occurs and no invalid candidate advances.
- [x] **Step 5: Prove existing retry behavior remains intact**
  - Run: `cargo test -p engine interactive_engine -- --nocapture`
  - Expected: PASS; when the decorator returns the original error, current malformed-submit feedback and retry caps behave unchanged.
- [x] **Step 6: Refresh and verify the engine seam**
  - Run: `./scripts/check-engine-public-api.sh`
  - Expected on the first run: FAIL with only the intentional decorator/event diff; apply that diff to `crates/engine/tests/snapshots/public_api.txt` with `apply_patch`.
  - Run: `./scripts/check-engine-public-api.sh`
  - Run: `cargo clippy -p engine --all-targets -- -D warnings`
  - Expected: PASS with only the intentional public surface changes.

## Maintainability Gate

- [x] No new port trait exists; the stable `AiPort` boundary is decorated.
- [x] The repair prompt, guards, and acceptance logic live in one focused module.
- [x] The same inner provider is invoked directly, preventing recursive repair.
- [x] Model selection is data in `OutputRepairPolicy`; it does not branch by provider or leak UI concerns into execution.
- [x] The shared completion protocol is the only authority that accepts repaired output.
- [x] Repair failure cannot weaken current engine retry or tool-approval behavior.

## Self-Review

- [x] Repair input excludes transcript, tool results, repository content, and private reasoning.
- [x] Candidate length is measured before building the overseer request.
- [x] Truncation never routes to generative reconstruction.
- [x] Cancellation wins over both primary and overseer work.
- [x] No vague implementation placeholders remain.

## Result

- Status: Complete.
- Verification: `cargo test -p engine output_repair` (14 pass); `cargo test -p engine interactive_engine` (46 pass); `./scripts/check-engine-public-api.sh` PASS; `cargo clippy -p engine --all-targets -- -D warnings` PASS.
- Notes: Decorator not wired into runs yet — that is slice 4.
