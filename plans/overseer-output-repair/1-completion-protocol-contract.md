# Slice 1: Completion Protocol and Repairable Failure Contract

## Goal

- Produce one engine-owned completion protocol that normalizes and validates final-output calls, and returns a redacted in-memory repair candidate when deterministic recovery cannot satisfy the node output schema.

## Current Question

- Question: None.
- Recommended answer: Keep JSON wire decoding in `providers`, but move the semantic `openflow_submit_node_output` envelope and output-schema rules into `engine`.
- Reason: The provider adapter knows how bytes become JSON; the engine owns what constitutes a valid completed node.

## Codebase Findings

- `crates/providers/src/mapping/mod.rs` currently owns JSON repair, wrapper normalization, internal-tool parsing, and outcome construction.
- `crates/engine/src/ports/outbound.rs` already exposes `AgentError::MalformedSubmitOutput`, but it keeps only a display detail and loses the malformed arguments needed by an overseer.
- `crates/engine/src/execution/interactive_engine/completion.rs` already applies three bounded same-model correction nudges; this slice must preserve that fallback.
- `AgentTurnSuccess.output` is not authoritatively checked against `AgentRequest.output_schema` after normalization.
- Focused baseline commands already pass: `cargo test -p providers --lib mapping::tests::` (24 tests) and `cargo test -p providers --lib rig_adapter::outcome::tests::` (10 tests).
- Test command: `cargo test -p engine completion_protocol -- --nocapture`

## Ownership

- Create: `crates/engine/src/execution/completion_protocol.rs` for submit-output normalization, JSON Schema validation, and conversion to `AgentTurnOutcome`.
- Modify: `Cargo.toml`, `crates/engine/Cargo.toml`, and `Cargo.lock` to add the JSON Schema validator used by the engine protocol.
- Modify: `crates/engine/AGENTS.md` and `docs/architecture/contract.md` to record the pure validation dependency while preserving the engine's no-I/O boundary.
- Modify: `crates/engine/src/execution/mod.rs` and `crates/engine/src/lib.rs` to expose only the protocol types/functions required by providers and the repair decorator.
- Modify: `crates/engine/src/ports/outbound.rs` to add `OutputRepairCandidate`, `OutputRepairFailureKind`, and a redacted malformed-submit error payload.
- Modify: `crates/providers/src/mapping/mod.rs` to retain JSON decoding/`jsonrepair-rs`, delegate semantic validation to engine, and remove duplicated completion-envelope decisions.
- Test: inline tests in `crates/engine/src/execution/completion_protocol.rs`, `crates/engine/src/ports/outbound.rs`, and `crates/providers/src/mapping/mod.rs`.
- Test: `crates/engine/tests/snapshots/public_api.txt` for the intentional public contract change.

## Contract Detail

- `OutputRepairCandidate` contains the tool-call ID when available, fixed tool name, size-capped raw arguments, sanitized validation detail, original output schema, failure kind, and safe usage/finish metadata when available.
- `OutputRepairFailureKind` distinguishes invalid JSON, wrong envelope, schema violation, and truncated response.
- Implement custom `Debug` for the candidate so raw arguments render as a redacted byte count. `Display` for `AgentError` must never include raw arguments or private reasoning.
- The completion protocol accepts a decoded JSON value, applies the existing information-preserving wrapper normalization, validates `output` against the effective node schema, and constructs `AgentTurnOutcome::Completed` only on success.
- Keep the existing large-string file-path guidance and valid legacy normalization behavior. Do not map arbitrary prose to an unrelated first schema field when it cannot pass schema validation.
- A truncated candidate is typed for diagnostics but is not eligible for AI repair because missing content cannot be reconstructed safely.

## Steps

- [x] **Step 1: Write failing engine contract tests**
  - Prove a valid wrapped result completes, a valid flat legacy result normalizes, a schema-invalid result returns `SchemaViolation`, and candidate `Debug`/error `Display` omit a secret sentinel from raw arguments.
  - Prove a `finish_reason=length` candidate is classified as truncated and not repairable.
- [x] **Step 2: Verify RED**
  - Run: `cargo test -p engine completion_protocol -- --nocapture`
  - Expected: FAIL because the completion-protocol module, schema validation, and typed repair candidate do not exist.
- [x] **Step 3: Implement the minimal engine protocol**
  - Add `jsonschema = { version = "0.48.0", default-features = false }` under workspace dependencies and consume it through `jsonschema.workspace = true` in `crates/engine/Cargo.toml`; default features stay off so HTTP/file reference resolution cannot leak I/O into engine.
  - Update the engine dependency documentation in `crates/engine/AGENTS.md` and `docs/architecture/contract.md` in the same change.
  - Preserve unrelated lockfile entries while recording the new dependency graph.
  - Move semantic normalization and final-output conversion into `completion_protocol.rs`.
  - Extend `AgentError` with a repair-candidate accessor while preserving the existing human-facing malformed-submit message.
  - Update provider mapping to call the new engine function only after `serde_json` and `jsonrepair-rs` decoding.
- [x] **Step 4: Verify GREEN**
  - Run: `cargo test -p engine completion_protocol -- --nocapture`
  - Run: `cargo test -p engine ports::outbound -- --nocapture`
  - Expected: PASS; malformed candidates are typed and redacted, and valid outputs satisfy their schema.
- [x] **Step 5: Prove provider behavior did not regress**
  - Run: `cargo test -p providers --lib mapping::tests:: -- --nocapture`
  - Run: `cargo test -p providers --lib rig_adapter::outcome::tests:: -- --nocapture`
  - Expected: PASS; deterministic JSON repair and existing wrapper normalization still complete without an overseer.
- [x] **Step 6: Refresh and verify the public seam**
  - Run: `./scripts/check-engine-public-api.sh`
  - Expected on the first run: FAIL with only the intentional repair-vocabulary diff; apply that diff to `crates/engine/tests/snapshots/public_api.txt` with `apply_patch`.
  - Run: `./scripts/check-engine-public-api.sh`
  - Run: `./scripts/check-architecture.sh`
  - Expected: the snapshot changes only for the intentional repair vocabulary, and architecture passes.

## Maintainability Gate

- [x] One completion-envelope implementation is shared by provider parsing and overseer acceptance.
- [x] Deterministic repair stays ahead of AI repair.
- [x] Raw arguments cannot appear through `Debug`, `Display`, telemetry, or public error formatting.
- [x] Engine remains free of HTTP, filesystem, provider, orchestration, and UI imports.
- [x] Schema validation is behavior-tested through the public protocol function.

## Self-Review

- [x] Every new public symbol is necessary at the engine/provider seam.
- [x] Tests use a secret sentinel and prove all formatted surfaces redact it.
- [x] Existing normalization cases have explicit regression tests.
- [x] Commands and symbol names match the current working tree.
- [x] No vague implementation placeholders remain.

## Result

- Status: Done
- Verification:
  - `cargo test -p engine completion_protocol` — 6 passed
  - `cargo test -p engine ports::outbound` — 1 passed
  - `cargo test -p providers --lib mapping::tests::` — 26 passed
  - `cargo test -p providers --lib rig_adapter::outcome::tests::` — 11 passed
  - `cargo clippy -p engine -p providers -- -D warnings` — clean
  - `./scripts/check-engine-public-api.sh` — pass (snapshot refreshed)
  - `./scripts/check-architecture.sh` — pass
- Notes: Snapshot also gained transitive `equivalent::Equivalent` blanket noise from `jsonschema`; kept because the dep is required. Provider jsonrepair test now passes an open object schema so schema validation matches sibling outcome tests.
