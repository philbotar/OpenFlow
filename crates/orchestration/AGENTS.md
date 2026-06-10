# Agents in Orchestration

## Overview

An **agent** is a stateless workflow invocable that can be called by workflow nodes. Agents are snapshots of orchestration state (tools, settings, models) frozen at run-start to ensure deterministic subagent invocation.

**Hexagonal Architecture:** Agents exemplify the orchestration crate's hexagonal design:
- **Core (domain logic):** `agent/library.rs` — CRUD, validation, business rules
- **Ports (interfaces):** `FileAgentStore` trait — what the core depends on
- **Adapters (implementations):** `adapters/storage/agent_store.rs` — file system persistence
- **Boundaries:** Core never imports adapters; only adapters are concrete

This separation lets agents be tested without file I/O, persisted flexibly (file, DB, cloud), and extended without changing domain logic.

## Data Model

### `CallableAgent` (Domain: `engine::CallableAgent`)

The canonical agent type from the engine. Contains:
- `id`: Unique agent identifier
- `name`: Display name
- `description`: Agent behavior description
- `model_override`: Optional model selection override
- `tool_catalog_selection`: Which tools are available to this agent
- `tool_policy`: Tool approval requirements

**Immutable by design.** Agents don't change during a run—they're snapshots.

### `AgentDefinition` (Orchestration)

Orchestration's working copy: mirrors `CallableAgent` with file-system backing.

```rust
pub struct AgentDefinition {
    pub id: String,
    pub name: String,
    pub description: String,
    pub model_override: Option<String>,
    pub tool_catalog_selection: ToolCatalogSelection,
    pub tool_policy: ToolPolicy,
}
```

Persisted as `openflow/agents.json` (project root).

## Hexagonal Architecture: Ports & Adapters

```
┌─────────────────────────────────────┐
│      agent/library.rs               │
│  (Domain: CRUD & validation logic)  │
├─────────────────────────────────────┤
│  Outbound Port:                     │
│  - FileAgentStore (trait)           │
│    • load(id) → AgentDefinition     │
│    • save(def) → Result             │
│    • list() → Vec<AgentDefinition>  │
│    • delete(id) → Result            │
└─────────────────────────────────────┘
          ↓ (depends on)
┌─────────────────────────────────────┐
│  adapters/storage/agent_store.rs    │
│  (Adapter: file system I/O)         │
├─────────────────────────────────────┤
│  Implements FileAgentStore          │
│  Reads/writes: openflow/agents.json │
└─────────────────────────────────────┘
```

**Key principle:** `agent/library.rs` owns the port definition; `adapters/storage/` owns the implementation. This decouples persistence from business logic.

## Layers

### `agent/library.rs` — Agent Library

**Module:** `crate::agent::library`

Responsibility: CRUD & metadata for agents.

```rust
pub struct AgentLibrary {
    store: FileAgentStore,
}

impl AgentLibrary {
    pub fn list_agents(&self) -> Result<Vec<AgentDefinition>>;
    pub fn get_agent(&self, id: &str) -> Result<AgentDefinition>;
    pub fn create_agent(&mut self, def: AgentDefinition) -> Result<()>;
    pub fn update_agent(&mut self, def: AgentDefinition) -> Result<()>;
    pub fn delete_agent(&self, id: &str) -> Result<()>;
}
```

Used by:
- `backend::AppBackend::agent_library()` — for orchestration setup
- Workflow execution — to snapshot agents at run-start

### `adapters/storage/agent_store.rs` — Persistence

**Module:** `crate::agent_store`

Responsibility: File I/O for agent definitions.

```rust
pub struct FileAgentStore {
    project_root: PathBuf,
}

impl FileAgentStore {
    pub fn list(&self) -> Result<Vec<AgentDefinition>>;
    pub fn load(&self, id: &str) -> Result<AgentDefinition>;
    pub fn save(&mut self, def: AgentDefinition) -> Result<()>;
    pub fn delete(&mut self, id: &str) -> Result<()>;
}
```

**File layout:**
```
project_root/
└── openflow/
    └── agents.json              # JSON array of AgentDefinition
```

## Run Lifecycle

### 1. Run Start
```rust
RunCoordinator::start_run(workflow_id, agent_id) {
    // Snapshot agent at run-start
    let agent = agent_library.get_agent(agent_id)?;  // ← Load from store
    let snapshot = CallableAgent::from(agent);        // ← Freeze it
    // Pass snapshot to engine
}
```

The snapshot ensures that if the user edits the agent definition *during* a run, the running instance is unaffected.

### 2. Subagent Invocation
```rust
engine::execute_node(node, snapshot_agent) {
    // Engine uses snapshot_agent.tool_policy, model_override, etc.
    // Never fetches fresh agent definition
}
```

### 3. Run Completion
Snapshot is discarded. If the user re-runs the same workflow with a different agent, a fresh snapshot is taken.

## Import Scope

**Who can import agents?**

| Module | Can Import | Example |
|--------|-----------|---------|
| `workflow::application` | `agent_store::AgentDefinition` | Workflow catalog references agents by ID |
| `run::application::coordinator` | `agent_library::AgentLibrary`, `agent_store::FileAgentStore` | Snapshot agents at run-start |
| `backend::AppBackend` | `agent_library::AgentLibrary` | Expose agents to orchestration API |
| **Domain code** | ❌ | agent/application should not depend on workflow, run, etc. |

## Design Decisions

### Why snapshot agents at run-start?

**Problem:** If an agent definition changes during a run, subagent calls would see inconsistent tool policies, model overrides, etc.

**Solution:** Freeze the agent at run-start as an immutable `CallableAgent`. The engine never re-fetches. If the user edits agents, only *new* runs see the changes.

### Why split `AgentDefinition` (orchestration) from `CallableAgent` (engine)?

**`AgentDefinition`:** Orchestration concern (persistence, CRUD, UI representation).

**`CallableAgent`:** Engine concern (execution, node invocation, snapshot semantics).

They're structurally similar but logically distinct. The orchestration crate converts one → the other at run-start.

### Why is agent storage centralized in `adapters/storage/`?

Consolidates all persistence by *concern* (storage), not by domain. Makes it clear: "where do agents get persisted?" → check `adapters/storage/agent_store.rs`.

## Extending Agents

### Adding a new agent field

1. Add to `engine::CallableAgent` (engine crate)
2. Add to `orchestration::AgentDefinition` (orchestration crate, `adapters/storage/agent_store.rs`)
3. Update `FileAgentStore::load()` / `::save()` to handle JSON
4. Update `agent_library.rs` if new CRUD logic is needed
5. Update `RunCoordinator::start_run()` if the field affects snapshotting

### Adding agent validation

Put it in `agent/library.rs` (the CRUD layer), not in the store. Example:

```rust
impl AgentLibrary {
    pub fn validate_agent(&self, def: &AgentDefinition) -> Result<()> {
        if def.name.is_empty() {
            return Err(AgentError::EmptyName);
        }
        // ... more validation
        Ok(())
    }
}
```

## Agents in the Broader Orchestration Architecture

Agents are one of seven **domain concepts** in orchestration, each following the same hexagonal pattern:

| Domain | Logic | Adapter |
|--------|-------|---------|
| **agent** | `agent/library.rs` | `adapters/storage/agent_store.rs` |
| **workflow** | `workflow/catalog.rs` | `adapters/storage/{app,project}_workflow_store.rs` |
| **project** | `project/registry.rs` | `adapters/storage/project_store.rs` |
| **tool** | `tool/{registry,runner,output}.rs` | `adapters/tool_impl/` |
| **run** | `run/coordinator.rs` + `run/state/` + `run/execution/` | State projection (no persistence) |
| **settings** | `settings/facade.rs` | `adapters/storage/settings_store.rs` |
| **skill/template** | (none) | `adapters/storage/{skill,template}_store.rs` |

Each domain layer is **vertically independent**: agent CRUD doesn't depend on workflow, project, or run logic. They coordinate only through the backend composition root.

This hexagonal isolation means:
- **Testability:** Agents can be unit-tested without touching workflows or the file system
- **Flexibility:** Swap `adapters/storage/agent_store.rs` for a database without changing domain logic
- **Extensibility:** New domains (e.g., "team", "version") can be added without modifying existing code
- **Clarity:** Each domain owns its outbound port contracts; adapters are pure implementations

## References

- [engine::CallableAgent](../../engine/src/graph/callable_agent.rs) — canonical agent type
- [agent/library.rs](./src/agent/library.rs) — agent CRUD
- [adapters/storage/agent_store.rs](./src/adapters/storage/agent_store.rs) — file persistence
- [run/coordinator.rs](./src/run/coordinator.rs) — run start & snapshotting
- [CONTEXT.md](../CONTEXT.md) — orchestration layer overview
- [Orchestration Restructure Plan](.planning/ORCHESTRATION_RESTRUCTURE.md) — architecture rationale
