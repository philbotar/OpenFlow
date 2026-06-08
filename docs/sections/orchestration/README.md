# Orchestration

`crates/orchestration`

## What it does

Orchestration is the **composition root**: it wires domain engines, provider adapters, persistence, and runtime state. Domain stays stateless; orchestration owns I/O, run sessions, and UI-facing projections.

```text
desktop → AppBackend (thin orchestrator)
            ├── WorkflowCatalog      workflow CRUD, merge, project assign
            ├── AgentLibrary         saved agent definitions
            ├── ProjectRegistry      folder-scoped projects
            ├── SettingsFacade       settings, keys, skills, validation DTOs
            └── RunCoordinator       active run session, channels, lifecycle
```

## Module map

| Module | Owns |
| --- | --- |
| `backend.rs` | `AppBackend` — delegates to catalog modules; stable IPC surface for desktop |
| `workflow_catalog.rs` | App + project workflow merge/split, rename, assign/unassign |
| `agent_library.rs` | Callable agent CRUD, `create_agent_node` |
| `project_registry.rs` | `projects.json` load/save/create |
| `settings_facade.rs` | Settings, provider keys, skill discovery, `validate_workflow` summary |
| `run_coordinator.rs` | `start_run`, `submit_*`, `apply_execution_event`, session mutex |
| `api.rs` | IPC DTOs (`WorkflowListItem`, `ProviderReadiness`, …) |
| `error.rs` | `BackendError` |
| `execution/` | Drive host loop (AI/tool I/O), projects `RunTelemetry` → UI state, headless acceptance |
| `state.rs` | `WorkflowRunState` — UI run snapshot |
| `storage.rs`, `flow_store.rs` | Workflow file adapters |
| `agent_store.rs`, `project_store.rs`, `settings_store.rs` | JSON persistence |
| `template_store.rs` | `FileTemplateStore` (`TemplateStore` adapter) |
| `tools/` | Builtin tool registry and filesystem runner |
| `provider_config.rs`, `settings_store.rs` | Provider readiness and API key resolution |

## Why it is structured this way

- **Thin `AppBackend`:** desktop maps 1:1 to backend methods; implementation depth lives in named modules so each concern has locality.
- **Catalog vs run:** workflow/agent/project CRUD does not share a mutex with the active run session.
- **Stores stay adapters:** merge rules and assign logic sit in catalog modules, not in `FileWorkflowStore` / `FileProjectStore`.
- **Engine invocation rule:** only `RunCoordinator` spawns `drive_interactive_workflow`; UI never calls `InteractiveEngine` directly.

## Change paths

| Goal | Primary files |
| --- | --- |
| Workflow merge or project assign | `workflow_catalog.rs`, `flow_store.rs`, `project_registry.rs` |
| Saved agents / canvas node from agent | `agent_library.rs`, `agent_store.rs` |
| Settings or provider keys | `settings_facade.rs`, `settings_store.rs` |
| Run start, input, approval | `run_coordinator.rs`, `execution/drive.rs` |
| New desktop command | `backend.rs` (delegate), `crates/desktop/src/lib.rs` |
