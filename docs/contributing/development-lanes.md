# Development lanes

Use this page to classify a change before editing. It translates the architecture contract into day-to-day development lanes, skill selection, and verification commands.

Architecture facts live in [`../architecture/contract.md`](../architecture/contract.md). This page should point to those facts, not duplicate a second architecture model.

## Intake

1. Identify the primary touched path.
2. Read the matching crate `AGENTS.md`.
3. Check whether the change crosses a seam (`AiPort`, `ToolPort`, `AppBackend`, UI `api.ts`, provider factory, storage port).
4. Pick the narrowest verification lane that proves the behavior.
5. Run `./scripts/verify.sh` before handoff, commit, or PR (or load `openflow-finish-change`).

## Lanes

| Lane | Use when | Read first | Normal verification |
| --- | --- | --- | --- |
| Engine semantics | Workflow model, validation, prompts, execution state machine, ports, tool policy, telemetry | `crates/engine/AGENTS.md`, `docs/architecture/contract.md`, `docs/glossary.md`, `docs/architecture/end-to-end-runtime.md` when touching execution | `cargo nextest run -p engine` |
| Run orchestration | Active run lifecycle, execution host, approval/input loop, event projection, checkpoint/replay, execution cwd | `crates/orchestration/AGENTS.md`, `docs/architecture/end-to-end-runtime.md`, `docs/contributing/testing-workflows.md`, `docs/architecture/threading-concurrency.md` | `cargo nextest run -p orchestration --lib` and workflow acceptance when execution behavior changes |
| Application service | Workflow catalog, agent library, project registry, settings facade, tool registry/runner | `crates/orchestration/AGENTS.md`, `docs/contributing/coding-patterns.md` | Focused `cargo nextest run -p orchestration --lib` filters |
| Adapter/I/O | Storage files, tool implementations, git/LSP, filesystem or subprocess behavior | `crates/orchestration/AGENTS.md`, `docs/architecture/contract.md` | Focused adapter tests plus `./scripts/check-architecture.sh` |
| Provider adapter | Rig transport, request/response mapping, auth, streaming, tool argument repair | `crates/providers/AGENTS.md`, `docs/architecture/provider-adapters.md` | `cargo nextest run -p providers` |
| Desktop IPC | Tauri command handlers, event bridge, app bootstrap, macOS integration | `crates/desktop/AGENTS.md` | `cargo nextest run -p desktop` |
| UI/Desktop seam | `api.ts`, frontend DTOs, AppProvider, screens, panels, canvas | `crates/ui/AGENTS.md`, `docs/architecture/end-to-end-runtime.md` for event path | `npm --prefix crates/ui run typecheck` and focused Vitest |
| Cross-crate workflow | A user-visible workflow that crosses engine, orchestration, desktop, and UI | Root `AGENTS.md`, `docs/architecture/end-to-end-runtime.md`, `docs/contributing/testing-workflows.md` | `./scripts/test-fast.sh --execution`; full `./scripts/verify.sh` before handoff |

## Skill recommendations

Project-local skills stay procedural. They tell an agent how to classify a change and which docs to read. They must not become a second source of architecture facts.

| Skill | Path | Use when |
| --- | --- | --- |
| `openflow-engine-change` | `.cursor/skills/openflow-engine-change/SKILL.md` | Any edit under `crates/engine/**` |
| `openflow-orchestration-change` | `.cursor/skills/openflow-orchestration-change/SKILL.md` | Any edit under `crates/orchestration/**` |
| `openflow-provider-change` | `.cursor/skills/openflow-provider-change/SKILL.md` | Any edit under `crates/providers/**` |
| `openflow-ui-change` | `.cursor/skills/openflow-ui-change/SKILL.md` | Any edit under `crates/ui/**` |
| `rust-hexarc-organizer` | `.cursor/skills/rust-hexarc-organizer/SKILL.md` | Cross-crate placement, layer violations, which crate owns a change |
| `openflow-finish-change` | `.cursor/skills/openflow-finish-change/SKILL.md` | Final verification before handoff |

Load the matching lane skill first. Load `rust-hexarc-organizer` when ownership is unclear. Load `openflow-finish-change` before claiming done.

If a skill conflicts with `docs/architecture/contract.md`, fix the skill.

## Verification rules

| Goal | Command |
| --- | --- |
| Compile loop | `./scripts/check-fast.sh` / `./scripts/check-fast.sh --clippy <crate>` |
| Iterate | `./scripts/test-fast.sh` |
| Execution risk | `./scripts/test-fast.sh --execution` |
| Desktop risk | `./scripts/test-fast.sh --desktop` |
| Bedrock / AWS | `./scripts/verify/test-providers-bedrock.sh` |
| Handoff | `./scripts/verify.sh` |
| Full workspace Rust | `./scripts/verify.sh test` |

While editing, prefer `./scripts/check-fast.sh` before tests. Use `./scripts/test-fast.sh` for normal iteration. Add flags by risk:

```bash
./scripts/test-fast.sh --execution
./scripts/test-fast.sh --desktop
./scripts/test-fast.sh --execution --desktop
```

Use `./scripts/verify.sh` before handing work off. Default Rust tests match CI (`test-fast --execution`, no desktop). Parallel agents: `VERIFY_ISOLATE_TARGET=1`. Use `./scripts/verify.sh --deep` (or `./scripts/miri.sh`) when you want [Miri](https://github.com/rust-lang/miri) undefined-behavior coverage on `engine` and `orchestration`.

For execution behavior, always add:

```bash
cargo nextest run -p orchestration --test workflow_acceptance --no-capture
```

For architecture-sensitive work, always add:

```bash
./scripts/check-architecture.sh
```
