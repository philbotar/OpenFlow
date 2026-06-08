# Context

Domain terms for the Step-through-agentic-workflow architecture.

| Term | Definition |
|---|---|
| **Composition root** | The crate responsible for constructing and wiring all dependencies. Here, orchestration is the composition root — `AppBackend` delegates to `WorkflowCatalog`, `AgentLibrary`, `ProjectRegistry`, `SettingsFacade`, and `RunCoordinator`. Provider construction uses the factory pattern (`create_provider`). |
| **WorkflowCatalog** | Orchestration module: workflow CRUD, app/project merge (project wins on ID collision), assign/unassign. Adapters: `FileWorkflowStore`, `flow_store`. |
| **RunCoordinator** | Orchestration module: active run session, action channel, `start_run` / `submit_*` / event projection entry points. |
| **CallableAgent** | Domain type (`domain::graph::CallableAgent`): saved agent snapshotted at run start for subagent invocation. Persisted as `openflow/agents.json`; orchestration alias `AgentDefinition`. |
| **RunTelemetry** | Domain enum for interactive run events (chat, tools, subagents). Orchestration type alias `ExecutionEvent`; projected into `WorkflowRunState` by `events.rs`. |
| **Factory pattern** | The `providers` crate exposes a single public factory function (`create_provider`) that returns `Box<dyn AiPort>`. Orchestration never names a concrete provider type. This is the contract boundary between orchestration and providers. |
| **Seam** | A typed boundary between layers. Examples: `domain::AiPort`, `UiDesktopOutboundPort` in `ui/src/lib/desktopClient.ts`. Add a seam only when a consumer depends on the interface, not the concrete type. |
| **Dependency graph** | `domain → (none)`, `providers → domain`, `orchestration → domain + providers`, `desktop → orchestration`, `ui → desktop`. |
| **Allowed import scope** | Which submodules a crate may import from its dependencies. `providers → domain`: only `domain::ports`. `orchestration → providers`: only `providers::create_provider`. `orchestration → domain`, `desktop → orchestration`: unrestricted. |
| **Architecture check tier** | The depth of validation enforced in CI. (A) uses Tier 2: Cargo.toml dependency graph checking + banned import patterns within source files. (B) is deferred (planned approach: strict `pub(crate)` visibility on concrete implementations). |
| **Violation class** | Taxonomy of architecture violations. Blocking: banned Cargo dep, banned import. Advisory: empty seam, missing re-export. |
| **Re-export boundary** | Domain types that cross layers (e.g., `Workflow`, `Node`) are re-exported through orchestration via `pub use`. Desktop imports `app_backend::Workflow`, never `workflow_core::Workflow`. This satisfies the "desktop must not depend on domain" rule without a DTO mapping layer. |
