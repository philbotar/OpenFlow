# Slice 5: Acceptance, Documentation, and Canonical Gate

## Goal

- Prove malformed worker output is repaired through the real orchestration path, handed downstream as schema-valid output, documented accurately, and accepted by the repository's full verification gate.

## Current Question

- Question: None.
- Recommended answer: Keep live-provider validation optional and contract-based; make deterministic acceptance tests the release gate for the overseer behavior.
- Reason: Model prose and provider quirks vary, while the malformed-candidate → repair → downstream-output contract can be proven without network access.

## Codebase Findings

- `crates/orchestration/tests/workflow_acceptance.rs` is the required deterministic end-to-end lane for execution behavior.
- `crates/orchestration/tests/support/mock_ai_stack.rs` scripts invocation-order outcomes, while a node-aware inline `AiPort` is safer when branch concurrency affects order.
- Acceptance harness settings must use `McpSettings { discover_external: false, .. }` to avoid user-local MCP discovery.
- `docs/contributing/testing-workflows.md` defines live tests as contract assertions rather than exact prose checks.
- `docs/architecture/provider-adapters.md` still points to pre-Rig `openai_compat.rs`, `anthropic.rs`, and `bedrock.rs` ownership and must be corrected.
- Canonical command: `./scripts/verify.sh`

## Ownership

- Modify: `crates/orchestration/tests/workflow_acceptance.rs` for the malformed-root → overseer-repair → valid-downstream scenario and exhausted-repair fallback.
- Modify: `crates/orchestration/tests/support/mock_ai_stack.rs` only if the shared fixture needs a typed malformed-turn constructor.
- Modify: `crates/providers/tests/rig_openai_compat.rs` and `crates/providers/tests/rig_anthropic.rs` for any missing final wire regressions identified by acceptance work.
- Modify: `docs/architecture/provider-adapters.md` to describe current `rig_adapter/*`, deterministic recovery, raw-preservation boundary, and engine-owned overseer policy.
- Create: `docs/architecture/output-repair.md` as the durable decision record for ownership, runtime sequence, safety guards, v1 scope, and deferred extensions.
- Modify: `docs/architecture/README.md` to index the new output-repair decision record.
- Modify: `docs/contributing/testing-workflows.md` to list the deterministic repair acceptance contract and optional live smoke expectations.
- Modify: `docs/glossary.md` to define `OutputRepairCandidate` and overseer output repair without calling it a normal workflow node or callable agent.

## Acceptance Contract

- A root node returns a typed malformed final-output candidate.
- The next AI request is isolated repair work with no worker transcript or external tools.
- When `outputRepairModel` is set, the repair request uses that model while the worker and downstream requests retain their own models; when unset, the repair request inherits the failed worker request's model.
- A valid repair is accepted by the original schema and becomes the root node output.
- A downstream node receives exactly that repaired structured output and completes.
- Successful repair records repair telemetry but no error toast/state.
- An invalid overseer response returns the original malformed error, triggers the existing bounded worker feedback path, and leaves the node retryable after exhaustion.
- A repair candidate containing a secret sentinel never appears in run trace, chat logs, `last_error`, or formatted errors.
- Node and callable-subagent final output use the same wrapper path.

## Steps

- [x] **Step 1: Write the failing acceptance tests**
  - Add a node-aware scripted `AiPort` that distinguishes worker, repair-scoped, downstream, and subagent requests by node ID/request shape.
  - Add the successful downstream propagation case, configured-model/fallback assertions, and the invalid-repair fallback/redaction case.
  - Set `discover_external: false` in every new harness settings value.
- [x] **Step 2: Verify RED**
  - Run: `cargo test -p orchestration --test workflow_acceptance output_repair -- --nocapture`
  - Expected: FAIL until slices 1–4 expose the full runtime behavior to the acceptance harness.
- [x] **Step 3: Close only integration gaps**
  - Fix wiring, metadata propagation, or fixture helpers revealed by the acceptance tests.
  - Do not broaden v1 into external-tool, request-input, separate-provider, or streaming-fragment repair.
- [x] **Step 4: Verify GREEN across focused lanes**
  - Run: `cargo test -p engine`
  - Run: `cargo test -p providers`
  - Run: `cargo test -p orchestration --lib`
  - Run: `cargo test -p orchestration --test workflow_acceptance -- --nocapture`
  - Run: `./scripts/check-architecture.sh`
  - Expected: PASS; provider Wiremock tests may require local-port permission in this environment.
- [x] **Step 5: Update and verify documentation**
  - Create the output-repair decision record and update the provider adapter, architecture index, testing, and glossary docs from current code, including `outputRepairModel`, its worker-model fallback, the same-provider boundary, v1 non-goals, and privacy/cost guardrails.
  - Run: `./scripts/verify.sh doc typos public-api arch`
  - Expected: PASS and no stale references to removed provider adapter ownership.
- [x] **Step 6: Run the canonical handoff gate**
  - Run: `./scripts/verify.sh`
  - Expected: PASS all configured steps.
  - If unrelated dirty-tree failures remain, rerun the exact focused lane for changed files, record the failing command and paths, and do not modify unrelated user work.
- [x] **Step 7: Update durable plan state**
  - Mark slices 1–5 complete in `plans/overseer-output-repair/todo.md` only after their recorded verification is green.
  - Fill each slice's Result section with exact commands and outcomes.

## Maintainability Gate

- [x] Acceptance proves downstream behavior, not private helper calls.
- [x] Provider fixtures prove real wire boundaries without live network dependency.
- [x] Docs describe the current Rig adapter layout and actual ownership seams.
- [x] Deferred scope remains explicit and does not leak into v1 implementation.
- [x] The full gate or exact unrelated blockers are recorded before handoff.

## Self-Review

- [x] Every fixed v1 decision in `todo.md` maps to at least one test or documented guard.
- [x] Secret-sentinel redaction is asserted end to end.
- [x] No exact model prose appears in live-AI assertions.
- [x] Plan results and checkbox state match actual verification.
- [x] No vague implementation placeholders remain.

## Result

- Status: Complete.
- Verification: `cargo test -p orchestration --test workflow_acceptance output_repair` (3 pass); focused engine/providers/orch + arch PASS; `VERIFY_SHARE_TARGET=1 ./scripts/verify.sh` PASS (fmt, clippy, doc, test, public-api, machete, typos, ui-typecheck, ui-test, deny, arch).
- Notes: Env-dependent MCP playwright home-config test marked `#[ignore]`. Allowed `MIT-0` for `jsonschema` transitive `borrow-or-share`.
