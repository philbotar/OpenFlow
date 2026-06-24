---
description: Coding agent orientation for the orchestration crate
globs: crates/orchestration/**
alwaysApply: false
---

# AGENTS.md — Orchestration

**Question this crate answers:** How does the desktop app store, load, wire, and host runs?

Composition root: entity domain logic + centralized adapters + run lifecycle. Depends on `engine` and `providers`; never on `desktop` or `ui`.

## Architecture

```
desktop → AppBackend (backend/mod.rs)
            ├── WorkflowCatalog      workflow CRUD, merge, assign
            ├── AgentLibrary         saved CallableAgent definitions
            ├── ProjectRegistry      folder-scoped projects
            ├── SettingsFacade       settings, keys, skills
            └── RunCoordinator       active run session
                    └── run/execution/   InteractiveEngine host (ONLY place)
```

### Hexagonal layout

```text
orchestration/src/
├── agent/          ports.rs + library.rs
├── workflow/       ports.rs + catalog.rs
├── project/        ports.rs + registry.rs
├── run/            coordinator.rs, execution/, state/
├── settings/       ports.rs + facade.rs
├── tool/           registry.rs, runner.rs, hooks.rs
├── adapters/
│   ├── storage/        File*Store impls
│   ├── tool_impl/      edit, grep, bash, …
│   └── infrastructure/ lsp, git
└── backend/mod.rs      composition root — wires domains + adapters
```

| Layer | Put code here | May import |
| --- | --- | --- |
| `{entity}/` | Use-case logic | `engine`, same-entity `ports.rs` |
| `{entity}/ports.rs` | Traits domain depends on | `engine` types only |
| `adapters/` | Concrete I/O | port traits — **never define ports here** |
| `backend/` | Wire stores into services | entity modules + adapters |
| `run/execution/` | Engine host, `ToolPortImpl` | `engine`, `tool/`, infrastructure |

**Banned in domain folders** (`agent/`, `workflow/`, `project/`, `settings/`, `tool/`):
- `use crate::adapters::`
- `use crate::{agent_store, flow_store, …}` — depend on port traits; wire in `backend/`

### State ownership

| Owned here | Not owned here |
| --- | --- |
| Active run session, approval queues, trace projection | Execution semantics (`engine`) |
| Persistence paths, JSON schemas | LLM wire format (`providers`) |
| Tool I/O, execution cwd, shared-context wiring | UI rendering |

## Dependency rules

**Allowed:** `engine`, `providers` (allowlisted: `create_provider`, config types)

**Forbidden:**
- `desktop`, `ui`, `tauri`
- `use providers::AiClient` — use `create_provider()` → `Box<dyn AiPort>`
- Constructing `InteractiveEngine` / `WorkflowRunner` outside `run/execution/`

## Code standards

1. **Entity folders** — flat logic files; no nested `application/` layers.
2. **Centralized adapters** — persistence in `adapters/storage/`; tools in `adapters/tool_impl/`.
3. **Thin backend** — `AppBackend` delegates; desktop maps 1:1 to backend methods.
4. **Catalog vs run** — workflow/agent CRUD does not share mutex with active run.
5. **Vocabulary** — [`docs/glossary.md`](../../docs/glossary.md); `CallableAgent` not "saved subagent".
6. **Errors** — `BackendError` at IPC boundary; map to actionable strings for UI.
7. **Engine invocation** — `drive.rs` stays thin around `InteractiveEngine::run()`.

## Patterns

### Where to add code

| Change | Location |
| --- | --- |
| New desktop command surface | Delegate in `backend/mod.rs`; logic in entity folder |
| Workflow merge / project assign | `workflow/catalog.rs`, `adapters/storage/*_workflow_store.rs` |
| Saved agents | `agent/library.rs`, `adapters/storage/agent_store.rs` |
| Run start, input, approval | `run/coordinator.rs`, `run/execution/` |
| UI run snapshot fields | `run/state/mod.rs` + engine telemetry if needed |
| New builtin tool | `adapters/tool_impl/` + `tool/registry.rs`; tier in `engine/tools/config.rs`; update `NODE_RUNTIME_PREAMBLE` |
| Tool execution wiring | `run/execution/tool_port.rs` |
| Settings / API keys | `settings/facade.rs`, `settings/provider.rs`, `adapters/storage/settings_store.rs` |
| New persistence | `adapters/storage/` + `{entity}/ports.rs` |
| IPC DTOs | `api.rs` |

See [`docs/architecture/orchestration-layout.md`](../../docs/architecture/orchestration-layout.md) for full directory map.

### Runtime semantics (orchestration wires, engine defines)

1. **Shared context** — trimmed and appended per run in execution layer.
2. **Execution cwd** — resolved at run start from project `default_execution_cwd` or process cwd.
3. **Callable agents** — snapshotted at run start via `resolve_callable_agent_snapshots`.
4. **Provider override** — `WorkflowSettings.provider_id` overrides active provider for the run.
5. **Workflow storage** — app `workflows.json` + project `.flow/workflows/`; merge on load (project wins on ID collision).

### Persistence (quick reference)

| Store | Path |
| --- | --- |
| App workflows | `{data_local}/openflow/workflows.json` |
| Project workflows | `{project}/.flow/workflows/{id}.workflow.json` |
| Agents | `{data_local}/openflow/agents.json` |
| Projects | `{data_local}/openflow/projects.json` |
| Settings | `{data_local}/openflow/settings.json` |
| Templates | `{data_local}/openflow/templates.json` |

API key precedence: transient input → stored `ProviderProfile.api_key` → env var fallback.

### Testing

| Pattern | When |
| --- | --- |
| Inline `#[cfg(test)] mod tests` | Default |
| `run/execution/tests.rs` | Execution subtree integration |
| `tests/workflow_acceptance.rs` | Headless end-to-end runs |

```bash
cargo test -p orchestration
cargo test -p orchestration --test workflow_acceptance -- --nocapture
./scripts/miri.sh   # nightly Miri (orchestration + engine; see docs/contributing/testing-workflows.md)
```

Use inline `impl AiPort` stubs. Live AI: `STEP_WORKFLOW_LIVE_AI=1` (see `docs/contributing/testing-workflows.md`).

## Change checklist

1. Domain folder free of adapter imports?
2. Engine constructed only in `run/execution/`?
3. New I/O behind a port trait in `{entity}/ports.rs`?
4. `./scripts/check-architecture.sh` passes?
5. Run `./scripts/verify.sh` after changes.

## Related docs

- [`docs/architecture/orchestration-layout.md`](../../docs/architecture/orchestration-layout.md)
- [`docs/architecture/callable-agents.md`](../../docs/architecture/callable-agents.md) - CallableAgent snapshot and subagent model
- [`docs/architecture/contract.md`](../../docs/architecture/contract.md)
- [`docs/architecture/threading-concurrency.md`](../../docs/architecture/threading-concurrency.md) — dual runtime, run mutex
- [`../../AGENTS.md`](../../AGENTS.md) — workspace map
