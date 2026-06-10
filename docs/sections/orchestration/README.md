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

Source is grouped by **entity** (`workflow/`, `agent/`, `project/`, `run/`, `settings/`, `tool/`) with centralized `adapters/` for I/O. Rust import paths match the folder layout (e.g. `orchestration::run::execution`, `orchestration::workflow::catalog`).

**Full walkthrough:** [`layout.md`](layout.md)

## Module map (import paths)

| Module path | On disk | Owns |
| --- | --- | --- |
| `backend` | `backend/mod.rs` | `AppBackend` — delegates to catalog modules; stable IPC surface |
| `workflow::catalog` | `workflow/catalog.rs` | App + project workflow merge/split, assign/unassign |
| `agent::library` | `agent/library.rs` | Callable agent CRUD, `create_agent_node` |
| `project::registry` | `project/registry.rs` | Project load/save/create |
| `settings::facade` | `settings/facade.rs` | Settings, provider keys, skills, validation summary |
| `run::coordinator` | `run/coordinator.rs` | `start_run`, `submit_*`, session mutex |
| `run::execution` | `run/execution/` | Drive loop, event projection, headless acceptance |
| `run::state` | `run/state/mod.rs` | `WorkflowRunState` — UI run snapshot |
| `tool::{registry,runner,output}` | `tool/` | Tool catalog, execution, artifacts |
| `api` | `api.rs` | IPC DTOs |
| `error` | `error.rs` | `BackendError` |
| `adapters::storage::*` | `adapters/storage/` | JSON persistence (`app_workflow_store`, `project_workflow_store`, …) |
| `tools` (internal) | `adapters/tool_impl/` | Runtime tool I/O (edit, grep, …) |
| `lsp`, `git` | `adapters/infrastructure/` | LSP and Git integration |
| `settings::provider` | `settings/provider.rs` | Provider readiness and API key resolution |

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
