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
            ├── ChatCatalog          saved direct-chat metadata
            ├── AgentLibrary         saved CallableAgent definitions
            ├── ProjectRegistry      folder-scoped projects
            ├── SettingsFacade       settings, keys, skills
            └── RunCoordinator       active run session
                    └── run/execution/   InteractiveEngine host (ONLY place)
```

### Hexagonal layout

```text
orchestration/src/
├── agent.rs        saved CallableAgent definitions and AgentStore port
├── workflow/       ports.rs + catalog.rs
├── project/        ports.rs + registry.rs
├── run/            coordinator/, execution/, handoff.rs, state.rs
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
- Constructing `InteractiveEngine` outside `run/execution/`

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
| Saved agents | `agent.rs`, `adapters/storage/agent_store.rs` |
| Direct chats | `chat.rs`, `adapters/storage/chat_store.rs` |
| Run start, input, approval | `run/coordinator.rs`, `run/execution/` |
| UI run snapshot fields | `run/state.rs` + engine telemetry if needed |
| New builtin tool | `adapters/tool_impl/` + `tool/registry.rs`; tier in `engine/tools/config.rs`; update `NODE_RUNTIME_PREAMBLE` |
| Tool execution wiring | `run/execution/tool_port.rs` |
| Node handoff storage / `run://` resolution | `run/handoff.rs`, `tool/dispatch.rs` |
| Settings / API keys | `settings/facade.rs`, `settings/provider.rs`, `adapters/storage/settings_store.rs` |
| New persistence | `adapters/storage/` + `{entity}/ports.rs` |
| IPC DTOs | `api.rs` |

See [`docs/architecture/orchestration-layout.md`](../../docs/architecture/orchestration-layout.md) for full directory map.

### Runtime semantics (orchestration wires, engine defines)

1. **Shared context** — trimmed and appended per run in execution layer.
2. **Execution cwd** — resolved at run start from project `default_execution_cwd` or process cwd.
3. **Callable agents** — snapshotted at run start via `resolve_callable_agent_snapshots`.
4. **Provider routing** — `AgentNodeConfig.provider_id` overrides `WorkflowSettings.provider_id`, which overrides the active settings provider. Orchestration builds every referenced client at run prep and routes each `AgentRequest` to its effective provider.
5. **Model default** — an empty node model inherits its effective provider's `default_model` at run prep; an explicit node model wins.
6. **Skill invocation** — installed `/skill-id` task-prompt tokens resolve through `SkillCatalog` at run start and augment node/callable-agent system prompts.
7. **Workflow storage** — app `workflows.json` + project `.flow/workflows/`; merge on load (project wins on ID collision).
8. **Direct chats** — `ChatCatalog` persists metadata plus project/model/approval/reasoning config. `backend/chat.rs` privately builds the execution `Workflow` at run start without returning it in the Chat DTO or adding it to `WorkflowCatalog`.
9. **Node handoffs** — `AiInvocationAdapter` validates and materializes `HANDOFF.md` or `HANDOFF.json` before emitting completion; run state and checkpoints retain the immutable manifest.

### Persistence (quick reference)

| Store | Path |
| --- | --- |
| App workflows | `{data_local}/openflow/workflows.json` |
| Chats | `{data_local}/openflow/chats.json` |
| Project workflows | `{project}/.flow/workflows/{id}.workflow.json` |
| Agents | `{data_local}/openflow/agents.json` |
| Projects | `{data_local}/openflow/projects.json` |
| Settings | `{data_local}/openflow/settings.json` |
| Node handoffs | `{run_root}/{run_id}/handoffs/{node_id}/HANDOFF.md` or `HANDOFF.json` |
API key precedence: transient input → stored `ProviderProfile.api_key` → env var fallback.

### Testing

| Pattern | When |
| --- | --- |
| Inline `#[cfg(test)] mod tests` | Default |
| `run/execution/tests.rs` | Execution subtree integration |
| `tests/workflow_acceptance.rs` | Headless end-to-end runs |

```bash
./scripts/check-fast.sh orchestration
cargo nextest run -p orchestration
cargo nextest run -p orchestration --test workflow_acceptance --no-capture
./scripts/miri.sh   # nightly Miri (orchestration + engine; see docs/contributing/testing-workflows.md)
```

`bedrock` is off by default here (no AWS SDK). Desktop enables it for the app.

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
- [`docs/architecture/end-to-end-runtime.md`](../../docs/architecture/end-to-end-runtime.md) — run host, events, pause/resume
- [`docs/architecture/contract.md`](../../docs/architecture/contract.md)
- [`docs/architecture/threading-concurrency.md`](../../docs/architecture/threading-concurrency.md) — dual runtime, run mutex, layer concurrency
- [`../../AGENTS.md`](../../AGENTS.md) — workspace map
