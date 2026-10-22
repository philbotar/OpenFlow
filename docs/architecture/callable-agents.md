# Callable agents in orchestration

A `CallableAgent` is a saved agent definition that a workflow node can invoke as a subagent. Each run uses a snapshot taken at run start, so later library edits do not affect it.

`agent.rs` owns CRUD, validation, and the `AgentStore` trait. `adapters/storage/agent_store.rs` persists `openflow/agents.json`. The [orchestration guide](../../crates/orchestration/AGENTS.md) covers the rest of the crate.

## Data model

`engine::graph::CallableAgent` is the canonical type. Orchestration re-exports it as `AgentDefinition` for saved definitions. A run uses an immutable snapshot of the selected records.

## Run lifecycle

1. `RunCoordinator` loads agents through `AgentLibrary` and freezes the selected records as `CallableAgent` snapshots.
2. Installed `/skill-id` tokens anywhere in task prompts resolve through the configured skill catalog. OpenFlow reads the exact `SKILL.md` files, appends their contents to the node or callable-agent system prompt before the snapshot starts executing, and keeps the resolved paths for later chat turns.
3. The engine uses the snapshot and never re-fetches saved agents during the run.
4. The snapshot is discarded; the next run takes a fresh one.

## Extend agents

1. Add the field to `engine::CallableAgent`.
2. Cover it in persisted `AgentDefinition` JSON and compatibility tests.
3. Update `agent.rs` validation if needed.
4. Update `RunCoordinator::start_run()` if snapshot semantics change.

## References

- [`agent.rs`](../../crates/orchestration/src/agent.rs)
- [`adapters/storage/agent_store.rs`](../../crates/orchestration/src/adapters/storage/agent_store.rs)
- [`run/coordinator/mod.rs`](../../crates/orchestration/src/run/coordinator/mod.rs)
- [`engine::CallableAgent`](../../crates/engine/src/graph/callable_agent.rs)
