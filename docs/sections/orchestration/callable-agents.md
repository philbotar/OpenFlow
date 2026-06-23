# Callable Agents in Orchestration

Domain concept guide (not crate orientation — see [`../../crates/orchestration/AGENTS.md`](../../crates/orchestration/AGENTS.md)).

## Overview

A **CallableAgent** is a saved agent definition a workflow node may invoke as a subagent. Orchestration snapshots agents at run-start so subagent calls stay deterministic.

**Hexagonal pattern:**
- **Core:** `agent/library.rs` — CRUD, validation
- **Port:** `agent/ports.rs` — `FileAgentStore` trait
- **Adapter:** `adapters/storage/agent_store.rs` — `openflow/agents.json`

## Data Model

### `CallableAgent` (engine)

Canonical type in `engine::graph::CallableAgent`. Immutable snapshot during a run.

### `AgentDefinition` (orchestration)

Orchestration working copy with file-system backing. Converted to `CallableAgent` at run-start.

## Run Lifecycle

1. **Run start** — `RunCoordinator` loads agent via `AgentLibrary`, freezes as `CallableAgent`.
2. **Subagent invocation** — engine uses snapshot; never re-fetches.
3. **Run completion** — snapshot discarded; next run gets fresh snapshot.

## Extending Agents

1. Add field to `engine::CallableAgent`
2. Add to orchestration `AgentDefinition` + store JSON
3. Update `agent/library.rs` validation if needed
4. Update `RunCoordinator::start_run()` if snapshot semantics change

## References

- [`agent/library.rs`](../../crates/orchestration/src/agent/library.rs)
- [`adapters/storage/agent_store.rs`](../../crates/orchestration/src/adapters/storage/agent_store.rs)
- [`run/coordinator.rs`](../../crates/orchestration/src/run/coordinator.rs)
- [`engine::CallableAgent`](../../crates/engine/src/graph/callable_agent.rs)
