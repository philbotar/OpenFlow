# Callable agents in orchestration

Engine and orchestration concept guide. For crate orientation, see [`crates/orchestration/AGENTS.md`](../../crates/orchestration/AGENTS.md).

## Overview

A **CallableAgent** is a saved agent definition a workflow node may invoke as a subagent. Orchestration snapshots agents at run-start so subagent calls stay deterministic.

**Hexagonal pattern:**

- **Core:** `agent/library.rs` - CRUD and validation.
- **Port:** `agent/ports.rs` - `AgentStore` trait.
- **Adapter:** `adapters/storage/agent_store.rs` - `openflow/agents.json`.

## Data model

### `CallableAgent` (engine)

Canonical type in `engine::graph::CallableAgent`. Immutable snapshot during a run.

### `AgentDefinition` (orchestration)

Orchestration working copy with filesystem backing. Converted to `CallableAgent` at run start.

## Run lifecycle

1. **Run start** - `RunCoordinator` loads agents via `AgentLibrary` and freezes the selected records as `CallableAgent` snapshots.
2. **Subagent invocation** - the engine uses the snapshot and never re-fetches saved agents during the run.
3. **Run completion** - the snapshot is discarded; the next run gets a fresh snapshot.

## Extend agents

1. Add the field to `engine::CallableAgent`.
2. Add the field to orchestration `AgentDefinition` JSON.
3. Update `agent/library.rs` validation if needed.
4. Update `RunCoordinator::start_run()` if snapshot semantics change.

## References

- [`agent/library.rs`](../../crates/orchestration/src/agent/library.rs)
- [`adapters/storage/agent_store.rs`](../../crates/orchestration/src/adapters/storage/agent_store.rs)
- [`run/coordinator/mod.rs`](../../crates/orchestration/src/run/coordinator/mod.rs)
- [`engine::CallableAgent`](../../crates/engine/src/graph/callable_agent.rs)
