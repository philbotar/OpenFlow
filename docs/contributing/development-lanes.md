# Development Lanes

Use this page to classify a change before editing. It translates the architecture contract into day-to-day development lanes, skill selection, and verification commands.

Architecture facts live in [`../architecture/contract.md`](../architecture/contract.md). This page should point to those facts, not duplicate a second architecture model.

## Intake

1. Identify the primary touched path.
2. Read the matching crate `AGENTS.md`.
3. Check whether the change crosses a seam (`AiPort`, `ToolPort`, `AppBackend`, `UiDesktopOutboundPort`, provider factory, storage port).
4. Pick the narrowest verification lane that proves the behavior.
5. Run `./scripts/verify.sh` before handoff, commit, or PR.

## Lanes

| Lane | Use when | Read first | Normal verification |
| --- | --- | --- | --- |
| Engine semantics | Workflow model, validation, prompts, execution state machine, ports, tool policy, telemetry | `crates/engine/AGENTS.md`, `docs/architecture/contract.md`, `docs/glossary.md` | `cargo test -p engine` |
| Run orchestration | Active run lifecycle, execution host, approval/input loop, event projection, checkpoint/replay, execution cwd | `crates/orchestration/AGENTS.md`, `docs/contributing/testing-workflows.md`, `docs/architecture/threading-concurrency.md` | `cargo test -p orchestration --lib` and workflow acceptance when execution behavior changes |
| Application service | Workflow catalog, agent library, project registry, settings facade, tool registry/runner | `crates/orchestration/AGENTS.md`, `docs/contributing/coding-patterns.md` | Focused `cargo test -p orchestration --lib` filters |
| Adapter/I/O | Storage files, tool implementations, git/LSP, filesystem or subprocess behavior | `crates/orchestration/AGENTS.md`, `docs/architecture/contract.md` | Focused adapter tests plus `./scripts/check-architecture.sh` |
| Provider adapter | OpenAI-compatible, Anthropic, request/response mapping, auth, SSE, tool argument repair | `crates/providers/AGENTS.md` | `cargo test -p providers` |
| Desktop IPC | Tauri command handlers, event bridge, app bootstrap, macOS integration | `crates/desktop/AGENTS.md` | `cargo test -p desktop` |
| UI/Desktop seam | `UiDesktopOutboundPort`, `api.ts`, frontend DTOs, AppProvider, screens, panels, canvas | `crates/ui/AGENTS.md` | `npm --prefix crates/ui run typecheck` and focused Vitest |
| Cross-crate workflow | A user-visible workflow that crosses engine, orchestration, desktop, and UI | Root `AGENTS.md`, `docs/contributing/testing-workflows.md` | `./scripts/test-fast.sh --execution`; full `./scripts/verify.sh` before handoff |

## Skill Recommendations

Project-local skills should stay procedural. They tell an agent how to classify a change and which docs to read. They must not become a second source of architecture facts.

| Skill | Path | Use when |
| --- | --- | --- |
| `openflow-orchestration-change` | `.cursor/skills/openflow-orchestration-change/SKILL.md` | Any edit under `crates/orchestration/**` — run lifecycle, persistence, tools, AppBackend, adapters |
| `rust-hexarc-organizer` | `.cursor/skills/rust-hexarc-organizer/SKILL.md` | Cross-crate placement, layer violations, which crate owns a change |
| `openflow-engine-change` | `.cursor/skills/openflow-engine-change/SKILL.md` | Any edit under `crates/engine/**` — graph, execution, ports, tools, telemetry |
| `openflow-ui-change` | *(not yet created)* | UI/Desktop seam lane |
| `openflow-provider-change` | *(not yet created)* | Provider adapter lane |
| `openflow-finish-change` | *(not yet created)* | Final verification before handoff |

For orchestration work, load `openflow-orchestration-change` first. For engine work, load `openflow-engine-change` first. Both route to doc paths in the lanes table; architecture rules stay in those docs.

If a skill conflicts with `docs/architecture/contract.md`, fix the skill.

## Verification Rules

Use `./scripts/test-fast.sh` for normal iteration. Add flags by risk:

```bash
./scripts/test-fast.sh --execution
./scripts/test-fast.sh --desktop
./scripts/test-fast.sh --execution --desktop
```

Use `./scripts/verify.sh` before handing work off. It is the canonical full gate. Use `./scripts/verify.sh --deep` (or `./scripts/miri.sh`) when you want [Miri](https://github.com/rust-lang/miri) undefined-behavior coverage on `engine` and `orchestration` — also run on CI via the `miri` workflow job.

For execution behavior, always add:

```bash
cargo test -p orchestration --test workflow_acceptance -- --nocapture
```

For architecture-sensitive work, always add:

```bash
./scripts/check-architecture.sh
```
