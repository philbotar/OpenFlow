# Development commands

Run commands from the repository root. [Build from source](../getting-started/build-from-source.md) covers the toolchain and first launch. [Testing workflows](testing-workflows.md) defines which checks each change needs.

| Goal | Command |
| --- | --- |
| Full desktop app | `./scripts/start.sh` |
| Install desktop app (macOS) | `./scripts/install.sh` |
| Frontend only | `npm --prefix crates/ui run dev` |
| Frontend typecheck | `npm --prefix crates/ui run typecheck` |
| Fast Rust lane | `./scripts/test-fast.sh` |
| Fast lane with execution tests | `./scripts/test-fast.sh --execution` |
| Full verification gate | `./scripts/verify.sh` (default Rust tests = `test-fast`; desktop via `./scripts/verify.sh test`) |
| Leg timing | `./scripts/bench-test-legs.sh` |
| Architecture checks only | `./scripts/check-architecture.sh` |
| Workflow acceptance | `cargo nextest run -p orchestration --test workflow_acceptance --no-capture` |
| Live AI smoke | `STEP_WORKFLOW_LIVE_AI=1 STEP_WORKFLOW_LIVE_API_KEY=... STEP_WORKFLOW_LIVE_MODEL=... cargo nextest run -p orchestration --test live_workflow --run-ignored ignored-only --no-capture` |

## Architecture check config

Run architecture checks with:

```bash
./scripts/check-architecture.sh
```

The machine-readable rule file is [`../../crates/workspace-checks/arch-check-rules.toml`](../../crates/workspace-checks/arch-check-rules.toml). The workspace-check tooling owns this file.

Human-readable architecture rules live in [`../architecture/contract.md`](../architecture/contract.md).

## Verification quick path

| Goal | Command |
| --- | --- |
| Compile loop | `./scripts/check-fast.sh` |
| Iterate | `./scripts/test-fast.sh` (+ `--execution` if touching runs) |
| Handoff / PR | `./scripts/verify.sh` |
| Full workspace Rust (incl. desktop) | `./scripts/verify.sh test` |
| Parallel agents on one machine | `VERIFY_ISOLATE_TARGET=1 ./scripts/verify.sh` |

```bash
./scripts/check-fast.sh
./scripts/test-fast.sh
./scripts/verify.sh
```

Default verify reuses `./target` and runs the CI-aligned `test-fast` lane (no desktop; Bedrock/AWS off unless desktop/featured). Opt-in full workspace: `./scripts/verify.sh test`.

Intentional changes to engine's public surface require updating `crates/engine/tests/snapshots/public_api.txt` (`cargo +nightly public-api` from `crates/engine/`).

[Testing workflows](testing-workflows.md) lists checks by behavior. Run `./scripts/verify.sh --deep` to add mutation testing.
