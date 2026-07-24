# Testing workflows

How to verify workflow behavior without manually clicking through the desktop app.

## Test placement conventions

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

Files predating this convention are brought into conformance opportunistically - when materially editing a file whose tests violate the rules, fix the placement in the same change. No mass migrations.

## Local dev loops

| Goal | Command | What It Proves |
| --- | --- | --- |
| Start full desktop app | `./scripts/start.sh` | Tauri config, frontend dev server, and desktop bootstrap work together |
| Start frontend only | `npm --prefix crates/ui run dev` | Vite dev server and frontend rendering load without desktop runtime |
| Frontend typecheck | `npm --prefix crates/ui run typecheck` | TS/TSX surface still matches current DTOs and component usage |

## Playwright visual regression (chat segments)

Browser-only E2E with mocked IPC in `crates/desktop/e2e/`. Snapshots assert multi-node chat segment dividers and spacing (dark theme).

| Goal | Command |
| --- | --- |
| Install E2E deps | `npm --prefix crates/desktop run e2e:install` |
| Run visual snapshots | `npm --prefix crates/desktop/e2e run test:visual` |
| Update baselines after intentional CSS change | `npm --prefix crates/desktop/e2e run test:visual -- --update-snapshots` |
| Full browser E2E smoke | `npm --prefix crates/desktop run e2e:browser` |
| Providers settings E2E | `npm --prefix crates/desktop/e2e run test:browser -- tests/settings-providers.spec.ts` |

Visual tests use a fixed 1280×900 viewport, `deviceScaleFactor: 1`, and a static three-node settled-run fixture (`fixtures/multiSegmentChat.ts`). Not included in `./scripts/verify.sh` by default (Chromium install + ~30s).

## Runner

All Rust tests use [cargo-nextest](https://nexte.st/) (`cargo nextest run`). Verify scripts, CI, and docs assume nextest — install via `./scripts/setup.sh` or `cargo install cargo-nextest --locked`. Do not use plain `cargo test` for workspace checks.

## Fast local compile loop

| Goal | Command |
| --- | --- |
| Check touched crates (default: engine + providers + orchestration) | `./scripts/check-fast.sh` |
| One crate | `./scripts/check-fast.sh engine` |
| Clippy one/few crates (not workspace) | `./scripts/check-fast.sh --clippy orchestration` |
| Tests without desktop/Tauri | `./scripts/test-fast.sh` |

**Bedrock / AWS SDK:** `providers` and `orchestration` leave the `bedrock` feature **off** by default so day-to-day checks skip the AWS crate graph. The `desktop` crate enables `bedrock` for the shipped app. Bedrock-only tests: `cargo nextest run -p providers --features bedrock` (CI runs `./scripts/verify/test-providers-bedrock.sh`).

**Linker / cache:** `.cargo/config.toml` sets `linker = "rust-lld"`. Scripts prepend the rustup host `bin` dir to `PATH`. For an interactive shell: `source ./scripts/dev-env.sh`. If `sccache` is installed, scripts set `RUSTC_WRAPPER=sccache`.

## Orchestration headless E2E (`MockAiStack`)

Integration tests under `crates/orchestration/tests/` drive real orchestration execution (no desktop UI, no HTTP providers) via `run_workflow_headless` or `spawn_interactive_workflow_run`.

| Goal | Command |
| --- | --- |
| Stack-mock E2E suite | `cargo nextest run -p orchestration --test workflow_e2e --no-capture` |
| Contract acceptance (tools, manual nodes, checkpoints) | `cargo nextest run -p orchestration --test workflow_acceptance --no-capture` |
| Both integration suites | `cargo nextest run -p orchestration --test workflow_e2e --test workflow_acceptance --no-capture` |

Shared helpers live in `crates/orchestration/tests/support/`:

- **`MockAiStack`** - `impl AiPort` that pops scripted `MockTurn` responses per invoke (`from_invocation_order([...])` consumes the first array entry on the first call).
- **`run_headless_script`** - thin wrapper around `run_workflow_headless`.
- **`spawn_interactive_script`** - wrapper for mid-run interrupt/stop scenarios.

Use inline `impl AiPort` stubs (e.g. node-id-aware `ScriptedAi` in `workflow_acceptance.rs`) when stack order is not deterministic (branch/join parallelism).

## Test layers

| Layer | Command | What It Proves |
| --- | --- | --- |
| Unit tests | `cargo nextest run --workspace` | Engine rules, tool approval resolution, app/project/agent stores, provider config, shared-context and callable-agent helpers, OpenAI-compatible and Anthropic wire mapping, `jsonrepair-rs` tool-argument recovery |
| Desktop command tests | `cargo nextest run -p desktop` | Tauri command wiring for bootstrap, projects, agents, workflows |
| Deterministic workflow acceptance | `cargo nextest run -p orchestration --test workflow_acceptance --no-capture` | A whole workflow can run headlessly with scripted AI outputs, tool calls, and approval pauses |
| Orchestration headless E2E (stack mock) | `cargo nextest run -p orchestration --test workflow_e2e --no-capture` | Full orchestration + engine runs with `MockAiStack` (`tests/support/`) - happy path, retries, missing input/approval, interrupt; no real providers |
| Live AI smoke | `STEP_WORKFLOW_LIVE_AI=1 STEP_WORKFLOW_LIVE_API_KEY=... STEP_WORKFLOW_LIVE_MODEL=... cargo nextest run -p orchestration --test live_workflow --run-ignored ignored-only --no-capture` | A real BYOK provider can complete a small workflow and satisfy schema-level rules |
| Miri (engine + orchestration UB) | `./scripts/miri.sh` or `./scripts/verify.sh --deep miri` | UB interpreter over `engine` + `orchestration` **lib** tests; runs in `release.yml` `release-verify` on tag push (Ubuntu); not on PR CI. |

## Miri

[Miri](https://github.com/rust-lang/miri) interprets Rust MIR to detect undefined behavior. Scope: **`engine`** and **`orchestration`** (`providers` / `desktop` still out - HTTP/Tauri/FFI).

| Goal | Command |
| --- | --- |
| Run Miri (both crates) | `./scripts/miri.sh` |
| Run Miri (one crate) | `./scripts/miri.sh engine` or `./scripts/miri.sh orchestration` |
| Deep verify | `./scripts/verify.sh --deep` |
| Engine cross target (macOS) | `MIRI_ENGINE_VPROC=x86_64-unknown-linux-gnu ./scripts/miri.sh` |

Scope: `cargo miri nextest run -p engine --lib` (isolated; `-Zmiri-ignore-leaks`) and `cargo miri nextest run -p orchestration --lib` with a **UB-relevant allowlist** (`run::execution`, `coordinator`, `tool::runner`, `tool::blocking_ops`, `tool::retry`, `schedule`, `adapters::infrastructure`; see `ORCH_MIRI_FILTER` in `./scripts/miri.sh`) plus `-Zmiri-disable-isolation` for temp files. Pure edit/patch/store logic stays on `test-fast`/clippy — workspace `unsafe_code = "forbid"` means those tests have no UB surface. Tests run one Miri process each via [cargo-nextest](https://nexte.st/docs/integrations/miri/) (`--profile default-miri`) so lib tests can use multiple cores. Trade-offs: Miri recompiles the test crate per test (large crates may see less net speedup); cross-test data races on shared statics are not detected (unlike `cargo miri test`). Integration binaries and tests Miri cannot run (tokio, git/bash/MCP subprocess, live `#[ignore]` suites) carry `#[cfg_attr(miri, ignore)]`.

First run installs nightly `miri` and `cargo-nextest` if missing. Artifacts: `target/miri/`. Optional: `MIRI_JOBS=N` caps nextest parallelism; `MIRI_TOOLCHAIN` selects the rustc nightly (the `release-verify` job pins `nightly-2026-06-20` for cache stability).

Mark new unsupported tests with `#[cfg_attr(miri, ignore)]` and a one-line `ponytail:` comment.


The deterministic acceptance tests should prove:

1. Root nodes receive `entrypoint.text`.
2. Downstream nodes receive upstream outputs in deterministic order.
3. Branch/join workflows complete with all expected node outputs.
4. Manual nodes pause before execution, carry a per-node conversation, and pass the final structured node output downstream when the model signals it is ready.
5. Tool-enabled nodes can request one or more tool calls, receive tool results back into the model loop, and still produce the final node output downstream.
6. Tool approval pauses block progress until an approval decision is supplied, and denied tools surface a structured error without corrupting the run.
7. Run trace entries expose queued, running, paused, completed, or failed state transitions.
8. Chat logs capture system, thinking, user, and assistant messages where relevant, including paused-node follow-up turns and approval prompts.
9. Malformed final-output repair: a typed repairable candidate triggers one isolated overseer request (no worker transcript/tools); a valid repair becomes the node output handed downstream; an invalid overseer response preserves the original retryable path; secret sentinels never appear in trace, chat, or formatted errors; `outputRepairModel` overrides the repair model when set and otherwise inherits the worker model.

Unit tests in `crates/orchestration/src/run/execution/` should additionally prove:

10. `WorkflowSettings.shared_context` is appended to node and subagent system prompts.
11. `engine::resolve_callable_agent_snapshots` honors `callable_agents` and `allow_all_callable_agents`.
12. `resolve_execution_cwd` falls back to process cwd when unset and rejects invalid directories.

Store and backend tests should prove:

13. `AppBackend::load_all_workflows` merges app-store and project-discovered workflows.
14. Project assign/unassign updates `projects.json` and routes saves to the correct store.
15. App persistence uses `{data_local}/openflow/` only (no legacy data-dir fallback).

### Plan → Execute coverage

For a workflow that enables `WorkflowSettings.planMode`, add focused coverage for:

1. Validation rejects a missing review node or one without `requestUserInput`.
2. Planning denies non-`docs/**/*.md` repository writes, MCP, and subagent calls at both engine and host tool boundaries. Only the configured evidence-source node may mutate `run://PLAN.md` or see/call the seal tool; a read-only evidence source may still mutate the virtual draft.
3. `write` plus replace-mode `edit` build `run://PLAN.md`; an argument-free `openflow_write_plan_artifact` call always pauses for explicit human approval. Denial leaves the draft mutable. Approval atomically seals it. A repeated successful seal returns the same artifact. The artifact has a host UUID path, 256 KiB cap, SHA-256, replayable `artifact:<uuid>` lookup, and no Markdown transcript payload.
4. A schema-valid review completion freezes one hash-verified packet; checkpoint/resume preserves it even if saved workflow settings later change.
5. Implementation, verification, and review requests receive the same packet hash and reference; non-docs repository writes stay blocked until the packet is frozen.
6. A workflow without Plan Mode has its existing request and tool behavior.

## Live AI rules

Live AI smoke tests must avoid exact prose assertions. Model output changes naturally, so assert contracts instead:

1. The run completes.
2. Every expected node has output.
3. Output is valid JSON.
4. Output satisfies the node schema.
5. Required fields are non-empty.
6. A sentinel value such as `ORCHID-91` is preserved exactly across nodes.

Optional live smoke for overseer repair (when intentionally exercising a real provider): assert the run completes with schema-valid node outputs after a forced malformed submit path is not practical without a controllable mock; prefer the deterministic `output_repair_*` acceptance tests for release gating.

## Seam test placement

Guidelines:

1. Test `AiPort` contract behavior with inline `impl AiPort` stubs in the owning test module (see `workflow_acceptance.rs`, `runner.rs` tests).
2. Test provider wire mapping in `providers/src/mapping/` and `rig_adapter/` (plus `crates/providers/tests/rig_*.rs`).
3. Test UI desktop seam by mocking `api.ts` wrappers when adding AppProvider behavior tests.
4. End-to-end behavior remains in existing acceptance/live workflows.

## Frontend test placement

| Area | Location | What to test |
| --- | --- | --- |
| DTO helpers | `crates/ui/src/lib/*.test.ts` | Project grouping, execution cwd display, workflow utilities |
| Provider vs legacy thinking | `crates/ui/src/components/conversation/providerThinking.test.ts` | Distinguish provider reasoning from legacy tool I/O lines on the thinking role |
| Component behavior | `crates/ui/src/**/*.test.tsx` | Callable agent editor, app shell routing |
| Canvas | `crates/ui/src/canvas/*.test.ts` | Graph interaction contracts |

## Verification gate (`scripts/verify.sh`)

Primary gate for agents and local handoff - run after every change:

```bash
./scripts/verify.sh
```

Default Rust test step is **`test-fast`** (same lane as CI: engine + providers + orchestration lib + workspace-checks + workflow acceptance). It does **not** build `desktop`/Tauri. Full workspace coverage (including desktop): `./scripts/verify.sh test`.

**CI** runs parallel jobs in [`.github/workflows/ci.yml`](../.github/workflows/ci.yml): a `build` job warms a shared Rust cache, then `fmt`, `clippy`, `test` (`test-fast.sh --execution`), `ui`, and `lint-extras` (machete, typos, deny, arch, doc, public-api) run in parallel. Skips full workspace `test` (desktop/Tauri) and `--deep` steps (`mutants`, `miri`). Miri runs at release (tag push) in `release.yml` `release-verify`, not on PR CI. Run `./scripts/verify.sh` before handoff or PR.

Run a granular script directly for full untruncated output; run `verify.sh` for the truncated summary gate.

| Step | Granular script |
| --- | --- |
| `fmt` | `./scripts/verify/fmt.sh` |
| `clippy` | `./scripts/verify/clippy.sh` |
| `doc` | `./scripts/verify/doc.sh` |
| `test-fast` (default) | `./scripts/test-fast.sh --execution --skip-ui-typecheck` |
| `test` (opt-in) | `./scripts/verify/test.sh` (`cargo nextest run --workspace`, includes desktop) |
| `public-api` | `./scripts/check-engine-public-api.sh` |
| `machete` | `./scripts/verify/machete.sh` |
| `typos` | `./scripts/verify/typos.sh` |
| `ui-typecheck` | `./scripts/verify/ui-typecheck.sh` |
| `ui-test` | `./scripts/verify/ui-test.sh` |
| `deny` | `./scripts/verify/deny.sh` |
| `arch` | `./scripts/check-architecture.sh` |
| `mutants` (`--deep`) | `./scripts/verify/mutants.sh` |
| `miri` (`--deep`) | `./scripts/miri.sh` |

| Test-fast leg | Granular script |
| --- | --- |
| engine | `./scripts/verify/test-engine.sh` |
| providers | `./scripts/verify/test-providers.sh` |
| orchestration lib | `./scripts/verify/test-orchestration-lib.sh` |
| workspace-checks | `./scripts/verify/test-workspace-checks.sh` |
| workflow acceptance | `./scripts/verify/test-execution.sh` |
| desktop | `./scripts/verify/test-desktop.sh` |

| Behavior | Detail |
| --- | --- |
| Default | Runs all 11 steps; continues on failure so one run surfaces every broken step |
| Output | One line per step (`PASS fmt (1s)` / `FAIL clippy (41s)`); truncated logs on fail; summary with exact repro commands |
| Noise | No ANSI/progress escapes (`CARGO_TERM_COLOR=never`, `NO_COLOR=1`, `--quiet` on cargo/npm where supported) |
| Filter | `./scripts/verify.sh fmt clippy ui-test` - unknown step name lists valid steps and exits 1 |
| Deep | `./scripts/verify.sh --deep` adds `cargo mutants --no-shuffle` and `./scripts/miri.sh` (Miri UB on `engine` + `orchestration`; minutes-long) |
| Env | `VERIFY_FAIL_FAST=1` stop on first failure; `VERIFY_MAX_LINES` (default 150) tail on fail; default reuses `./target`; `VERIFY_ISOLATE_TARGET=1` for parallel agents (`target/verify-<pid>`); `OPENFLOW_MAX_DEBUG_CACHE_GIB` caps `target/debug` (default 64 GiB); `OPENFLOW_MIN_BUILD_SPACE_GIB` sets the local pre-build free-space floor (default 24 GiB, 8 GiB on GitHub); `0` disables the corresponding disk guard |

**Steps (default):** `fmt`, `clippy` (pedantic/nursery/cargo), `doc`, `test-fast`, `public-api`, `machete`, `typos`, `ui-typecheck`, `ui-test`, `deny`, `arch`. **Opt-in:** `test` (full workspace incl. desktop). **`--deep` only:** `mutants`, `miri`.

**CI:** parallel jobs (`build` warm cache → `fmt`, `clippy`, `test`, `ui`, `lint-extras`); PR CI no longer runs Miri. Miri runs in the release workflow's `release-verify` job (tag push or `workflow_dispatch`) on Ubuntu: `./scripts/miri.sh` (both crates), pinning `nightly-2026-06-20` (`MIRI_TOOLCHAIN`) and caching `~/.cache/miri` (sysroot) + `target/miri` (via `rust-cache`).

**One-time installs:** `cargo install cargo-machete typos-cli cargo-mutants cargo-public-api`; Miri: `rustup toolchain install nightly --component miri` (see [Miri §](testing-workflows.md#miri)).

## Fast local lane

Use this during normal edit/test loops:

```bash
./scripts/test-fast.sh
```

Each leg is also runnable directly under `scripts/verify/test-*.sh` (e.g. `./scripts/verify/test-providers.sh`).

Why this exists:

- `cargo nextest run --workspace` rebuilds `desktop`, which pulls the Tauri/native stack.
- `cargo nextest run -p desktop` stays opt-in unless you are changing the desktop seam.
- Default `./scripts/verify.sh` already includes acceptance via `test-fast --execution`; standalone `./scripts/test-fast.sh` leaves acceptance opt-in via `--execution`.

Options:

```bash
./scripts/test-fast.sh --execution
./scripts/test-fast.sh --desktop
./scripts/test-fast.sh --execution --desktop
```

### Leg timing

Wall-clock per leg (warm `./target`):

```bash
./scripts/bench-test-legs.sh
# optional: also time desktop + full workspace
./scripts/bench-test-legs.sh --with-opt-in
```

Baseline on an Apple Silicon laptop after a warm incremental build (2026-07-22). Re-run the script on your machine if numbers drift a lot. First run after a feature/dep flip can be 10× these (cold compile).

| Leg | Script | Rough warm budget |
| --- | --- | --- |
| engine | `test-engine.sh` | ~1–5s |
| providers | `test-providers.sh` | ~4 min (~255s; Wiremock; several intentional ~60s timeout tests) |
| orchestration lib | `test-orchestration-lib.sh` | ~5 min (~315s) |
| workspace-checks | `test-workspace-checks.sh` | ~1–3 min |
| workflow acceptance | `test-execution.sh` | ~1–4 min |
| desktop | `test-desktop.sh` | opt-in; Tauri compile dominates |
| workspace | `test.sh` | opt-in; includes desktop |

**Not pursued after this baseline:** `[profile.test] opt-level` (engine already ~1s warm). **Already applied:** `bedrock` off by default on `providers`/`orchestration` (desktop enables it); ~2× fewer compile units in the fast lane. Next runtime cliff if needed: shorten intentional Wiremock timeout tests in `crates/providers/tests/rig_openai_compat.rs`.

## When to run each layer

| Goal | Command |
| --- | --- |
| Iterate on one crate | `./scripts/verify/test-engine.sh` (or providers / orchestration-lib) |
| Normal local loop | `./scripts/test-fast.sh` (+ `--execution` if touching runs) |
| Handoff / PR | `./scripts/verify.sh` |
| Desktop / full workspace Rust | `./scripts/verify.sh test` or `./scripts/test-fast.sh --desktop` |

`./scripts/verify.sh` replaces separate `cargo fmt`, `clippy`, and the CI-aligned Rust test lane before commits.

For local iteration, prefer `./scripts/test-fast.sh`. Use `./scripts/verify.sh` before handing work off or committing.

Run this when changing durable run persistence, replay, or resume behavior:

```bash
cargo nextest run -p orchestration --no-capture run::persistence adapters::storage::run_checkpoint_store run::coordinator_tests
cargo nextest run -p orchestration --test workflow_acceptance --no-capture
npm --prefix crates/ui run typecheck
```

Run this when changing execution behavior, node input shaping, shared context, callable agents, execution cwd, manual pauses, tool approvals, tool result routing, run trace, or chat logs:

```bash
cargo nextest run -p orchestration --test workflow_acceptance --no-capture
cargo nextest run -p orchestration execution::
```

### Schedule

Schedules live on `WorkflowSettings.schedule` and are evaluated only while the desktop app is open. For scheduler changes, run:

```bash
cargo nextest run -p orchestration --no-capture schedule
cargo nextest run -p desktop
npm --prefix crates/ui run test -- src/lib/schedule.test.ts src/api.test.ts
```

For end-to-end run behavior, also run:

```bash
cargo nextest run -p orchestration --test workflow_acceptance --no-capture
```

Run this when changing project/workflow persistence or bootstrap:

```bash
cargo nextest run -p orchestration project_store flow_store backend agent_store
cargo nextest run -p desktop
```

Run this when changing provider wire mapping or tool-argument parsing:

```bash
cargo nextest run -p providers
```

Run this only when intentionally checking a real provider/model:

```bash
STEP_WORKFLOW_LIVE_AI=1 \
STEP_WORKFLOW_LIVE_API_KEY="$OPENAI_API_KEY" \
STEP_WORKFLOW_LIVE_MODEL="gpt-4o-mini" \
cargo nextest run -p orchestration --test live_workflow --run-ignored ignored-only --no-capture
```

DeepInfra-compatible chat completions example:

```bash
STEP_WORKFLOW_LIVE_AI=1 \
STEP_WORKFLOW_LIVE_API_KEY="$OPENAI_COMPATIBLE_API_KEY" \
STEP_WORKFLOW_LIVE_BASE_URL="https://api.deepinfra.com/v1/openai" \
STEP_WORKFLOW_LIVE_CHAT_COMPLETIONS_PATH="chat/completions" \
STEP_WORKFLOW_LIVE_MODEL="deepseek-ai/DeepSeek-V4-Flash" \
cargo nextest run -p orchestration --test live_workflow --run-ignored ignored-only --no-capture
```
