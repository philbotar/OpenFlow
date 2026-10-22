# Testing workflows

Use deterministic tests to check workflow contracts, provider fixtures to check wire formats, and an authorized live smoke test to check a real account. Each gives different evidence; a fixture pass does not establish live-provider compatibility.

## Test placement conventions

### Rust unit tests

1. Unit tests live in a `#[cfg(test)] mod tests { ... }` block at the bottom of the source file they test.
2. If the test module exceeds ~150 lines, move it to a sibling file:
   - `foo.rs` → `foo_tests.rs` in the same directory, declared as `#[cfg(test)] mod foo_tests;` (see `adapters/tool_impl/edit/patch_tests.rs`).
   - `mod.rs` → `tests.rs` in the same directory, declared as `#[cfg(test)] mod tests;` (see `run/execution/tests.rs`).
3. Every extracted test module declaration carries `#[cfg(test)]` so test code never compiles into release builds.
4. No other variants: no `*_test.rs`, no `test_*.rs`, no `tests/` directories inside `src/`.

### Rust integration / acceptance tests

- Crate-level `tests/` directory (e.g. `crates/orchestration/tests/workflow_acceptance.rs`).
- Live-network tests are `#[ignore]` and env-gated (`STEP_WORKFLOW_LIVE_AI=1`).

### Frontend (TypeScript) tests

- Vitest files sit next to the source they test: `foo.ts` → `foo.test.ts`, `Foo.tsx` → `Foo.test.tsx`.
- No `__tests__/` directories.

When materially editing a file with older test placement, bring its tests into this layout in the same change. Do not migrate unrelated tests.

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

All Rust tests use [cargo-nextest](https://nexte.st/) (`cargo nextest run`). Verify scripts, CI, and docs assume nextest: install via `./scripts/setup.sh` or `cargo install cargo-nextest --locked`. Do not use plain `cargo test` for workspace checks.

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
| Contract acceptance (tools, human-input pauses, checkpoints) | `cargo nextest run -p orchestration --test workflow_acceptance --no-capture` |
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
| Manual saved-provider smoke | `./scripts/smoke-live-ai.sh` | The provider selected in saved app settings supports workflow authoring, agent authoring, a fixed two-node run, and post-run review |
| Manual mixed-provider smoke | `OPENFLOW_LIVE_AI_SECONDARY_PROVIDER=<provider-id> ./scripts/smoke-live-multi-provider.sh` | One real workflow routes an override node to a secondary saved provider and an inherited node to the shared saved provider |
| Manual saved-provider attachment smoke | `./scripts/smoke-live-attachments.sh` | The selected provider reads generated PNG/PDF fixtures, then reads a new fact from each attachment after durable stop/resume |

## Workflow acceptance contracts

Deterministic acceptance tests must establish:

1. Root nodes receive `entrypoint.text`.
2. Downstream nodes receive upstream outputs in deterministic order.
3. Branch/join workflows complete with all expected node outputs.
4. Nodes that allow human input start when dependencies are ready, pause when the model requests input, and retain their per-node conversation. They pass structured output downstream only after a valid submit.
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

Use deterministic `output_repair_*` acceptance tests for the overseer repair release gate. A real model cannot reliably be forced to submit malformed output, so a live smoke cannot consistently exercise that recovery path.

## Seam test placement

1. Test `AiPort` contract behavior with inline `impl AiPort` stubs in the owning test module (see `workflow_acceptance.rs`, `runner.rs` tests).
2. Test provider wire mapping in `providers/src/mapping/` and `rig_adapter/` (plus `crates/providers/tests/rig_*.rs`).
3. Test UI desktop seam by mocking `api.ts` wrappers when adding AppProvider behavior tests.
4. End-to-end behavior remains in existing acceptance/live workflows.

## Frontend test placement

| Area | Location | What to test |
| --- | --- | --- |
| DTO helpers | `crates/ui/src/lib/**/*.test.ts` | Project grouping, execution cwd display, workflow utilities |
| Provider vs legacy thinking | `crates/ui/src/components/conversation/providerThinking.test.ts` | Distinguish provider reasoning from legacy tool I/O lines on the thinking role |
| Component behavior | `crates/ui/src/**/*.test.tsx` | Callable agent editor, app shell routing |
| Canvas | `crates/ui/src/canvas/*.test.ts` | Graph interaction contracts |

## Verification gate (`scripts/verify.sh`)

Full gate for code PR readiness, releases, cross-layer changes, or an explicitly requested full verification:

```bash
./scripts/verify.sh
```

Default Rust test step is **`test-fast`** (same lane as CI: engine + providers + orchestration lib + workspace-checks + workflow acceptance). It does **not** build `desktop`/Tauri. Full workspace coverage (including desktop): `./scripts/verify.sh test`.

**CI** runs parallel jobs in [`.github/workflows/ci.yml`](../../.github/workflows/ci.yml): a `build` job warms a shared Rust cache, then `fmt`, `clippy`, `test` (`test-fast.sh --execution`), `ui`, and `lint-extras` (machete, typos, deny, arch, doc, public-api) run in parallel. Skips full workspace `test` (desktop/Tauri) and `--deep` steps (`mutants`). Use the full gate for code PR readiness; docs/instruction-only changes use link, path, syntax, and diff checks.

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
| Deep | `./scripts/verify.sh --deep` adds `cargo mutants --no-shuffle` |
| Env | `VERIFY_FAIL_FAST=1` stop on first failure; `VERIFY_MAX_LINES` (default 150) tail on fail; default reuses `./target`; `VERIFY_ISOLATE_TARGET=1` for parallel agents (`target/verify-<pid>`); `OPENFLOW_MAX_DEBUG_CACHE_GIB` caps `target/debug` (default 64 GiB); `OPENFLOW_MIN_BUILD_SPACE_GIB` sets the local pre-build free-space floor (default 24 GiB, 8 GiB on GitHub); `0` disables the corresponding disk guard |

**Steps (default):** `fmt`, `clippy` (pedantic/nursery/cargo), `doc`, `test-fast`, `public-api`, `machete`, `typos`, `ui-typecheck`, `ui-test`, `deny`, `arch`. **Opt-in:** `test` (full workspace incl. desktop). **`--deep` only:** `mutants`.

Release verification runs execution and desktop tests plus orchestration E2E on Ubuntu before the macOS builds.

**One-time installs:** `cargo install cargo-machete typos-cli cargo-mutants cargo-public-api`.

## Fast local lane

Use this during normal edit/test loops:

```bash
./scripts/test-fast.sh
```

Each leg is also runnable directly under `scripts/verify/test-*.sh` (e.g. `./scripts/verify/test-providers.sh`).

The fast lane omits `desktop` and its Tauri/native dependencies, which `cargo nextest run --workspace` rebuilds. Add `cargo nextest run -p desktop` when changing the desktop boundary. Default `./scripts/verify.sh` includes acceptance through `test-fast --execution`; standalone `./scripts/test-fast.sh` requires `--execution` to include it.

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

Baseline on an Apple Silicon laptop after a warm incremental build (2026-07-22). Measure again on your machine before using these timings. A cold compile after a feature or dependency change can take 10× as long.

| Leg | Script | Rough warm budget |
| --- | --- | --- |
| engine | `test-engine.sh` | ~1 to 5s |
| providers | `test-providers.sh` | ~4 min (~255s; Wiremock; several intentional ~60s timeout tests) |
| orchestration lib | `test-orchestration-lib.sh` | ~5 min (~315s) |
| workspace-checks | `test-workspace-checks.sh` | ~1 to 3 min |
| workflow acceptance | `test-execution.sh` | ~1 to 4 min |
| desktop | `test-desktop.sh` | opt-in; Tauri compile dominates |
| workspace | `test.sh` | opt-in; includes desktop |

**Not pursued after this baseline:** `[profile.test] opt-level` (engine already ~1s warm). **Already applied:** `bedrock` off by default on `providers`/`orchestration` (desktop enables it); ~2× fewer compile units in the fast lane. If provider tests need further optimization, consider shortening the intentional Wiremock timeout tests in `crates/providers/tests/rig_openai_compat.rs`.

## When to run each layer

| Goal | Command |
| --- | --- |
| Iterate on one crate | `./scripts/verify/test-engine.sh` (or providers / orchestration-lib) |
| Normal local loop | `./scripts/test-fast.sh` (+ `--execution` if touching runs) |
| Code PR / release | `./scripts/verify.sh` |
| Desktop / full workspace Rust | `./scripts/verify.sh test` or `./scripts/test-fast.sh --desktop` |

`./scripts/verify.sh` replaces separate `cargo fmt`, `clippy`, and the CI-aligned Rust test lane before commits.

For local changes, select the checks that exercise the affected behavior. Docs/instruction-only edits do not need Rust/UI suites unless executable examples or test/build behavior change. Repeat passing coverage only after relevant changes or failures.

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

For chat attachments, also run:

```bash
cargo nextest run -p engine
cargo nextest run -p providers
cargo nextest run -p orchestration --lib
cargo nextest run -p orchestration --test workflow_acceptance --no-capture
cargo nextest run -p desktop
npm --prefix crates/ui run typecheck
npm --prefix crates/ui run test -- src/api.test.ts src/components/conversation/ConversationComposer.test.tsx src/components/conversation/MessageAttachments.test.tsx
```

These deterministic tests prove persistence and provider wire shape, not that every configured
model accepts every media type. For live validation, use a generated PNG/PDF with a deterministic
question, record provider, exact model, transport, date, result, and exact error. Skip only when
credentials or a compatible configured model are unavailable. Never include credentials or source
paths in fixtures or logs.

To check PNG, PDF, and durable attachment replay with the provider and model selected in OpenFlow
Settings, run:

```bash
./scripts/smoke-live-attachments.sh
```

This command is manual-only. CI and `./scripts/verify.sh` never run it. The smoke creates temporary
PNG and two-page PDF fixtures, verifies an initial media-derived answer, stops and resumes the saved
run, then asks for a different fact that only the earlier attachment contains. It reports the exact
provider, model, and transport without logging credentials. A subscription token refresh may update
saved OAuth credentials, matching normal app behavior.

Run this only when intentionally checking a real provider/model:

```bash
STEP_WORKFLOW_LIVE_AI=1 \
STEP_WORKFLOW_LIVE_API_KEY="$OPENAI_API_KEY" \
STEP_WORKFLOW_LIVE_MODEL="gpt-4o-mini" \
cargo nextest run -p orchestration --test live_workflow --run-ignored ignored-only --no-capture
```

To check every direct app AI surface with the provider and model already selected in OpenFlow
Settings, run:

```bash
./scripts/smoke-live-ai.sh
```

This command is manual-only. CI and `./scripts/verify.sh` never run it. It loads the same saved
API key or subscription OAuth credentials as the app, then reports a separate result for:

1. Workflow **Create with AI** producing a valid small DAG. The generated DAG is not executed.
2. Agent **Create with AI** producing a valid reusable agent in a temporary store.
3. A separate fixed two-node workflow preserving `ORCHID-91` through its handoff.
4. The automatic post-run AI review completing without an error.
5. Two direct chats running against the saved provider concurrently; stopping one leaves the other
   active and able to answer a follow-up.

The smoke does not save generated workflows or agents. A subscription token refresh may update the
saved OAuth credentials, matching normal app behavior. Output repair remains covered by deterministic
acceptance tests because deliberately forcing malformed live model output would make this smoke
non-deterministic.

To validate real per-node provider routing, configure credentials for the active provider and one
secondary saved provider, then run:

```bash
OPENFLOW_LIVE_AI_SECONDARY_PROVIDER=bedrock ./scripts/smoke-live-multi-provider.sh
```

The active provider becomes the workflow's shared provider. The planning node overrides it with the
secondary provider; the implementation node inherits the shared provider. Set
`OPENFLOW_LIVE_AI_PRIMARY_PROVIDER` to override the active-provider default. The smoke writes run
records only under a temporary directory. Normal OAuth refresh may update saved credentials.

DeepInfra-compatible chat completions example:

```bash
STEP_WORKFLOW_LIVE_AI=1 \
STEP_WORKFLOW_LIVE_API_KEY="$OPENAI_COMPATIBLE_API_KEY" \
STEP_WORKFLOW_LIVE_BASE_URL="https://api.deepinfra.com/v1/openai" \
STEP_WORKFLOW_LIVE_CHAT_COMPLETIONS_PATH="chat/completions" \
STEP_WORKFLOW_LIVE_MODEL="deepseek-ai/DeepSeek-V4-Flash" \
cargo nextest run -p orchestration --test live_workflow --run-ignored ignored-only --no-capture
```
