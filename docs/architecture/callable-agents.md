# Callable agents in orchestration

A `CallableAgent` is a saved agent definition that a workflow node may invoke as a subagent. Orchestration snapshots the selected definitions at run start, so later library edits do not affect that run.

`agent.rs` owns CRUD, validation, and the `AgentStore` trait; `adapters/storage/agent_store.rs` persists `openflow/agents.json`. The [orchestration guide](../../crates/orchestration/AGENTS.md) describes the surrounding crate.

## Data model

`engine::graph::CallableAgent` is the canonical type. Orchestration re-exports it as `AgentDefinition` for saved definitions; the run uses an immutable snapshot of the selected records.

## Run lifecycle

1. `RunCoordinator` loads agents via `AgentLibrary` and freezes the selected records as `CallableAgent` snapshots.
2. Installed `/skill-id` tokens anywhere in task prompts resolve through the configured skill catalog. OpenFlow reads the exact `SKILL.md` files, appends their contents to the node or callable-agent system prompt before the snapshot starts executing, and keeps resolved paths available for later chat turns.
3. The engine uses the snapshot and never re-fetches saved agents during the run.
4. The snapshot is discarded; the next run gets a fresh snapshot.

## Extend agents

1. Add the field to `engine::CallableAgent`.
2. Cover the field in persisted `AgentDefinition` JSON and compatibility tests.
3. Update `agent.rs` validation if needed.
4. Update `RunCoordinator::start_run()` if snapshot semantics change.

## References

- [`agent.rs`](../../crates/orchestration/src/agent.rs)
- [`adapters/storage/agent_store.rs`](../../crates/orchestration/src/adapters/storage/agent_store.rs)
- [`run/coordinator/mod.rs`](../../crates/orchestration/src/run/coordinator/mod.rs)
- [`engine::CallableAgent`](../../crates/engine/src/graph/callable_agent.rs)
