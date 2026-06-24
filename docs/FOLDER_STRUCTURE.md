# Folder structure and architecture rules

This document defines the folder organization principles across all crates (except `ui` and `desktop`, which have their own constraints).

## Core principle: hexagonal architecture

Every crate applies **hexagonal architecture** with clear separation:
- **Core logic:** Business rules, entities, orchestration
- **Ports:** Traits that the core depends on
- **Adapters:** Concrete implementations of ports

**Rule:** Core never imports adapters. Adapters implement traits defined by core.

---

## Folder organization rules

### 1. Domain folders (flat structure)

**What:** Business-driven vertical slices with application logic.

**Rule:** No nested layers. Files go directly in domain folders.

```
domain_name/
├── ports.rs  # trait definitions (what domain depends on)
├── logic.rs  # application logic (CRUD, validation, orchestration)
├── other_logic.rs  # more logic
├── subfolder/  # only if logically grouped (e.g., run/execution/)
│   └── detail.rs
└── mod.rs  # optional, declares submodules
```

**Examples:**
- `agent/ports.rs` - AgentStore trait
- `agent/library.rs` - agent CRUD using AgentStore
- `workflow/ports.rs` - WorkflowStore trait
- `workflow/catalog.rs` - workflow catalog
- `run/coordinator/mod.rs` - run coordination
- `run/execution/` - execution details (grouped)

**✗ Don't do:**
```
agent/application/library.rs  # unnecessary nesting
workflow/application/catalog.rs  # extra level
```

### 2. Adapters folder (centralized by concern)

**What:** All concrete implementations of ports, grouped by technology concern.

**Rule:** Adapters go in `adapters/`, organized by **what they do**, not **what domain they serve**.

```
adapters/
├── storage/  # persistence implementations
│   ├── agent_store.rs
│   ├── app_workflow_store.rs
│   ├── project_workflow_store.rs
│   └── ...
├── infrastructure/  # external systems (LSP, Git, HTTP, DB clients)
│   ├── lsp/
│   ├── git/
│   └── http/
├── ai_provider/  # (providers crate only) AI service implementations
│   ├── anthropic.rs
│   ├── openai.rs
│   └── ...
├── tool_impl/  # tool-specific implementations
│   ├── edit/
│   └── ...
└── mod.rs
```

**Rule:** Never have nested adapters or adapters inside domains.

**✗ Don't do:**
```
agent/adapters/store.rs  # adapters belong in adapters/, not domains
```

### 3. Ports

**Where:** In domain-specific `ports.rs` file (not in adapter files).

**Rule:** Each domain defines the traits it depends on. Adapters implement those traits, never define them.

**Example:**
```rust
// agent/ports.rs (domain port definitions)
pub trait AgentStore {
    fn load(&self) -> io::Result<Vec<CallableAgent>>;
    fn save(&self, agents: &[CallableAgent]) -> io::Result<()>;
}
```

```rust
// agent/library.rs (domain logic uses the port)
use crate::agent::ports::AgentStore;

pub struct AgentLibrary {
    store: Box<dyn AgentStore>,
}
```

```rust
// adapters/storage/agent_store.rs (adapter implements the port)
use crate::agent::ports::AgentStore;

pub struct AgentFileStore { ... }

impl AgentStore for AgentFileStore { ... }
```

**Rule:**
- ✅ Port traits defined in `domain/ports.rs`
- ✅ Domain logic imports and uses ports
- ✅ Adapters implement ports
- ✗ Adapters never define ports

---

## Crate-specific rules

### `crates/engine` - Core domain

**What:** Domain model, workflow execution, execution state.

**Structure:**
```
engine/src/
├── conversation/  # domain concept (chat history)
├── execution/  # domain concept (run execution)
├── graph/  # domain concept (workflow structure)
├── ports/  # inbound/outbound ports for engine
├── template/  # domain concept
├── tools/  # domain concept (tool catalog, policies)
├── lib.rs
└── mod declarations
```

**Special case:** Engine defines its own `ports/inbound` and `ports/outbound` (boundaries for external systems). This is the exception: engine is the core and exports ports that others implement.

### `crates/orchestration` - Composition root

**What:** Orchestrates domain concepts (agents, workflows, projects, tools, runs, settings) + adapters.

**Structure:**
```
orchestration/src/
├── agent/
│   ├── ports.rs  # AgentStore trait
│   └── library.rs  # agent CRUD
├── workflow/
│   ├── ports.rs  # WorkflowStore trait
│   └── catalog.rs  # workflow catalog
├── project/
│   ├── ports.rs  # ProjectStore trait
│   └── registry.rs  # project registry
├── run/
│   ├── coordinator/                    # run coordination
│   ├── execution/  # execution details
│   └── state/mod.rs  # state projection
├── settings/
│   ├── ports.rs  # SettingsStore trait
│   └── facade.rs  # settings aggregation
├── tool/
│   ├── mod.rs  # tool layer module
│   ├── registry.rs  # tool catalog
│   ├── runner.rs  # tool execution
│   └── output.rs  # artifact storage
│
├── adapters/
│   ├── storage/  # all persistence
│   │   ├── agent_store.rs
│   │   ├── app_workflow_store.rs
│   │   ├── project_workflow_store.rs
│   │   ├── project_store.rs
│   │   ├── settings_store.rs
│   │   ├── skill_store.rs
│   │   └── template_store.rs
│   ├── tool_impl/  # tool implementation (edit, patching)
│   │   ├── edit/
│   │   ├── errors.rs
│   │   └── mod.rs
│   └── infrastructure/  # external systems
│       ├── lsp/  # LSP protocol
│       └── git/  # Git CLI
│
├── backend/mod.rs  # composition root (wires all domains + adapters)
├── api.rs  # public API entry points
├── lib.rs  # module declarations
└── error.rs  # top-level errors
```

**Rules:**
- Domain folders (`agent/`, `workflow/`, etc.) contain logic files, not adapters
- All adapters centralized in `adapters/` by concern (storage, tool_impl, infrastructure)
- No persistence inside domain folders
- `backend/mod.rs` is the only place that directly depends on both domain logic AND adapters

### `crates/providers` - Adapter crate

**What:** Implements `engine::ports::AiPort` for different AI providers.

**Structure:**
```
providers/src/
├── anthropic.rs  # Anthropic transport
├── openai_compat.rs  # OpenAI-compatible transport
├── client.rs  # AiClient implementing AiPort
├── mapping.rs  # transcript/tool-arg mapping
├── sse.rs  # SSE stream parsing
├── lib.rs  # create_provider() factory
└── ...
```

**Rules:**
- Single public entry point: `create_provider()` factory function in `lib.rs`
- New provider -> add `providers/src/{name}.rs` and wire in `create_provider()`
- Never expose concrete provider types to consumers
- Implement `engine::ports::AiPort` trait

### `crates/ui` - Frontend (EXEMPT)

**What:** React/TypeScript frontend for the desktop app.

**Rules:** N/A - use standard web app conventions (components, pages, hooks, etc.)

### `crates/desktop` - Desktop app (EXEMPT)

**What:** Tauri desktop shell.

**Rules:** N/A - use desktop app conventions.

---

## Key design decisions

### Why flat domain folders?

Avoids unnecessary nesting (`domain/application/logic.rs` -> `domain/logic.rs`). Hexagonal boundary is clear through:
1. Files in domain folders = core logic
2. Files in `adapters/` = implementations
3. `lib.rs` declares public API

### Why centralized adapters?

Makes it easy to find implementations: "where is agent persistence?" -> `adapters/storage/agent_store.rs`.

Organized by **concern** (storage, infrastructure, tool_impl), not by domain. This prevents duplicated infrastructure code and makes it clear what technologies are being used.

### Why no nested adapters?

Adapters are terminal implementations. They don't have sub-adapters. Nesting (`agent/adapters/store.rs`) creates confusion because:
1. Adapters aren't supposed to depend on domains
2. It suggests there might be multiple layers (adapters of adapters)
3. Breaks the one-way dependency rule

### Why single-purpose crates?

`providers` is purely adapters; `orchestration` orchestrates domains + adapters; `engine` is pure domain. This separation means:
- Easy to test each crate independently
- Clear responsibility per crate
- Easy to swap implementations (e.g., replace file storage with DB)

---

## Dependency rules

```
engine (core domain)
  ↑
  └─ orchestration (domains + adapters)
       ├─ agent/library -> adapters/storage/agent_store
       ├─ workflow/catalog -> adapters/storage/{app,project}_workflow_store
       └─ tool/runner -> adapters/tool_impl/

providers (adapters)
  ↑
  └─ orchestration (uses factory)

desktop/ui (frontend)
  ↑
  └─ orchestration (via IPC/API)
```

**Rule:** Each layer imports from layers below, never above. No circular dependencies.

---

## Applying the rules: checklist

When adding a new domain or adapter:

**New domain:**
- [ ] Create folder: `domain_name/`
- [ ] Create `domain_name/ports.rs` - define all traits the domain depends on
- [ ] Add logic files at root: `domain_name/logic.rs` (imports from ports.rs)
- [ ] Update `lib.rs` to re-export domain entry points
- [ ] Add to composition root (`backend/mod.rs`)

**New adapter:**
- [ ] Create folder in `adapters/concern_name/`
- [ ] Implement traits defined in `domain/ports.rs`
- [ ] Never define ports in adapters
- [ ] Never import the domain logic (only its ports)
- [ ] Update `adapters/mod.rs` if needed

**Refactoring existing code:**
- [ ] No nested `application/` folders -> move to domain root
- [ ] Adapters out of domain folders -> move to `adapters/`
- [ ] Organize adapters by concern, not domain
- [ ] Move trait definitions from adapters to `domain/ports.rs`
- [ ] Verify cargo check passes
- [ ] Update this document if new pattern emerges

## Port refactoring status

Current state: entity store traits live in `{entity}/ports.rs`, and concrete file stores live in `adapters/storage/`.

---

## References

- [CONTEXT.md](CONTEXT.md) - Orchestration-specific terms and dependencies
- [crates/orchestration/AGENTS.md](../crates/orchestration/AGENTS.md) - Orchestration crate orientation
- [docs/architecture/callable-agents.md](./architecture/callable-agents.md) - CallableAgent snapshot and subagent model
- Hexagonal Architecture: [Alistair Cockburn's original](https://alistair.cockburn.us/hexagonal-architecture/)
