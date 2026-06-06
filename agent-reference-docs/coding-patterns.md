# Coding Patterns

Patterns we follow in this repo.

## Architecture Rules

1. `workflow-core` is pure domain logic.
2. `ai` is an adapter crate that implements `AiPort` for BYOK providers.
3. `agent-workflow-app` composes UI, local state, and persistence.
4. Domain crate must not depend on `egui`, `eframe`, or HTTP clients.
5. UI crate must call domain APIs; do not duplicate domain rules in UI.

## Ownership By Concern

| Concern | Source of Truth |
| --- | --- |
| Workflow types, defaults, schema | `crates/workflow-core/src/model.rs` |
| Graph validity and layer order | `crates/workflow-core/src/validation.rs` |
| Batch run semantics | `crates/workflow-core/src/runner.rs` |
| Interactive pause/resume semantics | `crates/workflow-core/src/interactive.rs` |
| LLM invocation contract | `crates/workflow-core/src/ports.rs` |
| LLM transport mapping | `crates/ai/src/*` |
| UI rendering + interactions | `crates/agent-workflow-app/src/ui/*` |
| Mutable app state transitions | `crates/agent-workflow-app/src/state.rs` |
| File persistence formats | `crates/agent-workflow-app/src/storage.rs`, `settings_store.rs` |

## Implementation Conventions

1. Keep constants at top of file and name by intent (`*_WIDTH`, `*_HEIGHT`, `*_GAP`, `*_PADDING`).
2. Keep helper functions near usage and private unless reused.
3. Keep tests in `#[cfg(test)] mod tests` in the same file as behavior.
4. Use typed errors with `thiserror`; include actionable error strings.
5. Preserve deterministic order where it affects behavior by sorting IDs (existing pattern in `validation.rs` and `runner.rs`).
6. For mutating state, prefer dedicated methods on `AppState` rather than direct map/vector edits across UI files.
7. For visibility in UI modules, default to `pub(super)` to keep surface area narrow.

## Error Handling Rules

1. Map external/system errors into local domain language at crate boundaries.
2. Return `Result<_, _>` from operations that can fail; avoid panics outside tests.
3. Use `expect(...)` only for invariants that are guaranteed by validated flow.

## Test Strategy

1. Add focused unit tests in the same module for new behavior.
2. Test externally visible behavior, not private implementation detail.
3. For workflow logic changes, cover:
   - validation outcomes,
   - layer/execution ordering,
   - upstream input shape,
   - failure propagation.
4. For UI token/spacing changes, keep value-locking tests where tokens are intentional contracts.

## Dependency Boundary

1. Add workspace deps in root `Cargo.toml` first, then consume in crate manifests.
2. Keep crate dependencies minimal and role-specific:
   - `workflow-core`: model/validation/runner only.
   - `ai`: HTTP + provider payload parsing/auth only.
   - `agent-workflow-app`: UI/runtime/persistence.

## Change Checklist

1. Edit the smallest owning module first.
2. Add or update tests in the same PR.
3. Run:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets
cargo clippy-max
cargo test --workspace
```

4. If behavior contracts changed, update this doc and `AGENTS.md` map when needed.
