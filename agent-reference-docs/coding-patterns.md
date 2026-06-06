# Coding Patterns

Patterns we follow in this repo.

## Architecture Rules

1. `domain` is pure domain logic.
2. `providers` is an adapter crate that implements `AiPort` for BYOK providers.
3. `orchestration` composes runtime, state, and persistence.
4. `desktop` is a transport adapter only; keep Tauri commands and app bootstrap there.
5. `ui` owns rendering and interaction; it should talk through typed desktop invokes/events.
6. Domain crate must not depend on HTTP clients or async runtimes beyond what's needed for tests.
7. `orchestration` must call domain APIs; do not duplicate domain rules in `orchestration`.
8. Keep section-local seams in `src/ports/inbound.*` and `src/ports/outbound.*`.
9. Keep section-local implementations in `src/adapters/inbound.*` and `src/adapters/outbound.*`.
10. Prefer moving integration logic into `adapters/*` before adding new top-level modules.

## Ownership By Concern

| Concern | Source of Truth |
| --- | --- |
| Workflow types, defaults, schema | `crates/domain/src/model.rs` |
| Graph validity and layer order | `crates/domain/src/validation.rs` |
| Batch run semantics | `crates/domain/src/runner.rs` |
| Interactive pause/resume semantics | `crates/domain/src/interactive.rs` |
| LLM invocation contract | `crates/domain/src/ports/mod.rs` |
| Domain seam declarations | `crates/domain/src/ports/inbound.rs`, `crates/domain/src/ports/outbound.rs` |
| LLM transport mapping | `crates/providers/src/*` |
| Section adapter entry points | `*/src/adapters/inbound.*`, `*/src/adapters/outbound.*` |
| Mutable app state transitions | `crates/orchestration/src/state.rs` |
| File persistence formats | `crates/orchestration/src/storage.rs`, `settings_store.rs` |
| Tauri command/event surface | `crates/desktop/src-tauri/src/lib.rs` |
| Frontend invoke wrappers and UI state wiring | `crates/ui/src/api.ts`, `crates/ui/src/App.tsx` |

## Implementation Conventions

1. Keep constants at top of file and name by intent (`*_WIDTH`, `*_HEIGHT`, `*_GAP`, `*_PADDING`).
2. Keep helper functions near usage and private unless reused.
3. Keep tests in `#[cfg(test)] mod tests` in the same file as behavior.
4. Use typed errors with `thiserror`; include actionable error strings.
5. Preserve deterministic order where it affects behavior by sorting IDs (existing pattern in `validation.rs` and `runner.rs`).
6. For mutating state, prefer dedicated methods on `AppState` rather than direct map/vector edits across modules.

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

## Dependency Boundary

1. Add workspace deps in root `Cargo.toml` first, then consume in crate manifests.
2. Keep crate dependencies minimal and role-specific:
   - `domain`: model/validation/runner only.
   - `providers`: HTTP + provider payload parsing/auth only.
   - `orchestration`: runtime/state/persistence.
   - `desktop`: Tauri adapter only.
   - `ui`: frontend rendering and interaction only.

## Local Run Commands

1. Desktop app: `npm --prefix crates/desktop run start -- dev`
2. Frontend only: `npm --prefix crates/ui run dev`
3. Frontend typecheck: `npm --prefix crates/ui run typecheck`

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