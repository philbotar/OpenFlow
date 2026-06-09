# Orchestration

`crates/orchestration`

## What it does

Orchestration is the **composition root**: it wires domain engines, provider adapters, persistence, and runtime state. Domain stays stateless; orchestration owns I/O, run sessions, and UI-facing projections.

```text
desktop → AppBackend (backend/)
            ├── WorkflowCatalog      workflow CRUD, merge, project assign
            ├── AgentLibrary         saved agent definitions
            ├── ProjectRegistry      folder-scoped projects
            ├── SettingsFacade       settings, keys, skills, validation DTOs
            └── RunCoordinator       active run session, channels, lifecycle
```

## Source layout

Source is grouped by **entity** (`workflow/`, `agent/`, `project/`, `run/`, `settings/`, `template/`, `skill/`) and **hexarc role** (`application/` = service, `adapters/` = repository).

Rust module paths are still flat (`orchestration::workflow_catalog`, `orchestration::execution`) via `#[path]` in `lib.rs` — disk layout and import paths are intentionally different during migration.

**Full walkthrough:** [`layout.md`](layout.md)

## Module map (import paths)

| Module path | On disk | Owns |
| --- | --- | --- |
| `backend` | `backend/mod.rs` | `AppBackend` — delegates to catalog modules; stable IPC surface |
| `workflow_catalog` | `workflow/application/catalog.rs` | App + project workflow merge/split, assign/unassign |
| `agent_library` | `agent/application/library.rs` | Callable agent CRUD, `create_agent_node` |
| `project_registry` | `project/application/registry.rs` | Project load/save/create |
| `settings_facade` | `settings/application/facade.rs` | Settings, provider keys, skills, validation summary |
| `run_coordinator` | `run/application/coordinator.rs` | `start_run`, `submit_*`, session mutex |
| `execution/` | `run/application/execution/` | Drive loop, event projection, headless acceptance |
| `state` | `run/state/mod.rs` | `WorkflowRunState` — UI run snapshot |
| `api` | `api.rs` | IPC DTOs |
| `error` | `error.rs` | `BackendError` |
| `storage`, `flow_store` | `workflow/adapters/` | Workflow file adapters |
| `agent_store`, `project_store`, `settings_store` | `{entity}/adapters/` | JSON persistence |
| `template_store`, `skill_store` | `template/store.rs`, `skill/store.rs` | Templates and skill discovery |
| `tools/`, `lsp/`, `git` | `adapters/infrastructure/` | Runtime tool/LSP/git I/O |
| `provider_config` | `settings/adapters/provider_config.rs` | Provider readiness and API key resolution |

## Why it is structured this way

- **Entity folders** — find workflow code under `workflow/`, run code under `run/`, etc.
- **Service vs repository** — merge/assign logic in `application/`; JSON paths in `adapters/`.
- **Thin `AppBackend`** — desktop maps 1:1 to backend methods; depth lives in entity services.
- **Catalog vs run** — workflow/agent CRUD does not share a mutex with the active run session.
- **Engine invocation rule** — only `RunCoordinator` / `execution` spawn the interactive drive loop.

## Change paths

| Goal | Primary files |
| --- | --- |
| Workflow merge or project assign | `workflow/application/catalog.rs`, `workflow/adapters/`, `project/application/registry.rs` |
| Saved agents / canvas node from agent | `agent/application/library.rs`, `agent/adapters/store.rs` |
| Settings or provider keys | `settings/application/facade.rs`, `settings/adapters/` |
| Run start, input, approval | `run/application/coordinator.rs`, `run/application/execution/` |
| New desktop command | `backend/mod.rs` (delegate), `crates/desktop/src/lib.rs` |
