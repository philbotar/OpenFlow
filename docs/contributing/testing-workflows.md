# Testing Workflows

How to verify workflow behavior without manually clicking through the desktop app.

## Test Placement Conventions

Mirrored in `.cursor/rules/testing-conventions.mdc` so agents apply them automatically.

### Rust unit tests

1. **Default: inline.** Unit tests live in a `#[cfg(test)] mod tests { ... }` block at the bottom of the source file they test.
2. **Extract when large.** If the test module exceeds ~150 lines, move it to a sibling file:
   - `foo.rs` → `foo_tests.rs` in the same directory, declared as `#[cfg(test)] mod foo_tests;` (see `adapters/tool_impl/edit/patch_tests.rs`).
   - `mod.rs` → `tests.rs` in the same directory, declared as `#[cfg(test)] mod tests;` (see `run/execution/tests.rs`).
3. **Always gate.** Every extracted test module declaration carries `#[cfg(test)]` so test code never compiles into release builds.
4. No other variants: no `*_test.rs`, no `test_*.rs`, no `tests/` directories inside `src/`.

### Rust integration / acceptance tests

- Crate-level `tests/` directory (e.g. `crates/orchestration/tests/workflow_acceptance.rs`).
- Live-network tests are `#[ignore]` and env-gated (`STEP_WORKFLOW_LIVE_AI=1`).

### Frontend (TypeScript) tests

- Vitest files sit next to the source they test: `foo.ts` → `foo.test.ts`, `Foo.tsx` → `Foo.test.tsx`.
- No `__tests__/` directories.

### Migration

Files predating this convention are brought into conformance opportunistically — when materially editing a file whose tests violate the rules, fix the placement in the same change. No mass migrations.

## Local Dev Loops

| Goal | Command | What It Proves |
| --- | --- | --- |
| Start full desktop app | `npm --prefix crates/desktop run start -- dev` | Tauri config, frontend dev server, and desktop bootstrap work together |
| Start frontend only | `npm --prefix crates/ui run dev` | Vite dev server and frontend rendering load without desktop runtime |
| Frontend typecheck | `npm --prefix crates/ui run typecheck` | TS/TSX surface still matches current DTOs and component usage |

## Test Layers

| Layer | Command | What It Proves |
| --- | --- | --- |
| Unit tests | `cargo test --workspace` | Domain rules, tool approval resolution, app/project/agent stores, provider config, shared-context and callable-agent helpers, OpenAI-compatible and Anthropic wire mapping, `jsonrepair-rs` tool-argument recovery |
| Desktop command tests | `cargo test -p desktop` | Tauri command wiring for bootstrap, projects, agents, workflows |
| Deterministic workflow acceptance | `cargo test -p orchestration --test workflow_acceptance -- --nocapture` | A whole workflow can run headlessly with scripted AI outputs, tool calls, and approval pauses |
| Live AI smoke | `STEP_WORKFLOW_LIVE_AI=1 STEP_WORKFLOW_LIVE_API_KEY=... STEP_WORKFLOW_LIVE_MODEL=... cargo test -p orchestration --test live_workflow -- --ignored --nocapture` | A real BYOK provider can complete a small workflow and satisfy schema-level rules |

## Acceptance Rules

The deterministic acceptance tests should prove:

1. Root nodes receive `entrypoint.text`.
2. Downstream nodes receive upstream outputs in deterministic order.
3. Branch/join workflows complete with all expected node outputs.
4. Manual nodes pause before execution, carry a per-node conversation, and pass the final structured node output downstream when the model signals it is ready.
5. Tool-enabled nodes can request one or more tool calls, receive tool results back into the model loop, and still produce the final node output downstream.
6. Tool approval pauses block progress until an approval decision is supplied, and denied tools surface a structured error without corrupting the run.
7. Run trace entries expose queued, running, paused, completed, or failed state transitions.
8. Chat logs capture system, thinking, user, and assistant messages where relevant, including paused-node follow-up turns and approval prompts.

Unit tests in `orchestration/src/execution.rs` should additionally prove:

9. `WorkflowSettings.shared_context` is appended to node and subagent system prompts.
10. `domain::resolve_callable_agent_snapshots` honors `callable_agents` and `allow_all_callable_agents`.
11. `resolve_execution_cwd` falls back to process cwd when unset and rejects invalid directories.

Store and backend tests should prove:

12. `AppBackend::load_all_workflows` merges app-store and project-discovered workflows.
13. Project assign/unassign updates `projects.json` and routes saves to the correct store.
14. App persistence uses `{data_local}/openflow/` only (no legacy data-dir fallback).

## Live AI Rules

Live AI smoke tests must avoid exact prose assertions. Model output changes naturally, so assert contracts instead:

1. The run completes.
2. Every expected node has output.
3. Output is valid JSON.
4. Output satisfies the node schema.
5. Required fields are non-empty.
6. A sentinel value such as `ORCHID-91` is preserved exactly across nodes.

## Seam Test Placement

Guidelines:

1. Test `AiPort` contract behavior with inline `impl AiPort` stubs in the owning test module (see `workflow_acceptance.rs`, `runner.rs` tests).
2. Test provider wire mapping in `providers/src/mapping.rs`, `openai_compat.rs`, and `anthropic.rs`.
3. Test UI desktop seam by mocking `UiDesktopOutboundPort` when adding AppProvider behavior tests.
4. End-to-end behavior remains in existing acceptance/live workflows.

## Frontend Test Placement

| Area | Location | What to test |
| --- | --- | --- |
| DTO helpers | `crates/ui/src/lib/*.test.ts` | Project grouping, execution cwd display, workflow utilities |
| Legacy tool message parsing | `crates/ui/src/lib/parseLegacyToolMessages.test.ts` | Grouping old plain-text tool lines into tool bubbles |
| Component behavior | `crates/ui/src/**/*.test.tsx` | Callable agent editor, app shell routing |
| Canvas | `crates/ui/src/canvas/*.test.ts` | Graph interaction contracts |

## Verification Gate (`scripts/verify.sh`)

Primary gate for agents and CI — run after every change:

```bash
./scripts/verify.sh
```

| Behavior | Detail |
| --- | --- |
| Default | Runs all 11 steps; continues on failure so one run surfaces every broken step |
| Output | One line per step (`PASS fmt (1s)` / `FAIL clippy (41s)`); truncated logs on fail; summary with exact repro commands |
| Noise | No ANSI/progress escapes (`CARGO_TERM_COLOR=never`, `NO_COLOR=1`, `--quiet` on cargo/npm where supported) |
| Filter | `./scripts/verify.sh fmt clippy ui-test` — unknown step name lists valid steps and exits 1 |
| Deep | `./scripts/verify.sh --deep` adds `cargo mutants --no-shuffle` (minutes-long; missed mutants = untested behavior backlog) |
| Env | `VERIFY_FAIL_FAST=1` stop on first failure; `VERIFY_MAX_LINES` (default 150) tail on fail |

**Steps:** `fmt`, `clippy` (pedantic/nursery/cargo), `doc`, `test`, `public-api`, `machete`, `typos`, `ui-typecheck`, `ui-test`, `deny`, `arch`.

**One-time installs:** `cargo install cargo-machete typos-cli cargo-mutants cargo-public-api` (nightly toolchain for public-api).

## When To Run Each Layer

`./scripts/verify.sh` replaces separate `cargo fmt`, `clippy`, and `cargo test --workspace` before commits.

Run this when changing execution behavior, node input shaping, shared context, callable agents, execution cwd, manual pauses, tool approvals, tool result routing, run trace, or chat logs:

```bash
cargo test -p orchestration --test workflow_acceptance -- --nocapture
cargo test -p orchestration execution::
```

Run this when changing project/workflow persistence or bootstrap:

```bash
cargo test -p orchestration project_store flow_store backend agent_store
cargo test -p desktop
```

Run this when changing provider wire mapping or tool-argument parsing:

```bash
cargo test -p providers
```

Run this only when intentionally checking a real provider/model:

```bash
STEP_WORKFLOW_LIVE_AI=1 \
STEP_WORKFLOW_LIVE_API_KEY="$OPENAI_API_KEY" \
STEP_WORKFLOW_LIVE_MODEL="gpt-4o-mini" \
cargo test -p orchestration --test live_workflow -- --ignored --nocapture
```

DeepInfra-compatible chat completions example:

```bash
STEP_WORKFLOW_LIVE_AI=1 \
STEP_WORKFLOW_LIVE_API_KEY="$OPENAI_COMPATIBLE_API_KEY" \
STEP_WORKFLOW_LIVE_BASE_URL="https://api.deepinfra.com/v1/openai" \
STEP_WORKFLOW_LIVE_CHAT_COMPLETIONS_PATH="chat/completions" \
STEP_WORKFLOW_LIVE_MODEL="deepseek-ai/DeepSeek-V4-Flash" \
cargo test -p orchestration --test live_workflow -- --ignored --nocapture
```
