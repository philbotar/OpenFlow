# Folder Structure & Architecture Rules

This document defines the folder organization principles across all crates (except `ui` and `desktop`, which have their own constraints).

## Core Principle: Hexagonal Architecture

Every crate applies **hexagonal architecture** with clear separation:
- **Core (Domain Logic):** Business rules, entities, orchestration
- **Ports (Interfaces):** Traits that the core depends on
- **Adapters (Implementations):** Concrete implementations of ports

**Rule:** Core never imports adapters. Adapters implement traits defined by core.

---

## Folder Organization Rules

### 1. Domain Folders (Flat Structure)

**What:** Business-driven vertical slices with application logic.

**Rule:** No nested layers. Files go directly in domain folders.

```
domain_name/
├── ports.rs              ← trait definitions (what domain depends on)
├── logic.rs              ← application logic (CRUD, validation, orchestration)
├── other_logic.rs        ← more logic
├── subfolder/            ← only if logically grouped (e.g., run/execution/)
│   └── detail.rs
└── mod.rs                ← optional, declares submodules
```

**Examples:**
- `agent/ports.rs` — FileAgentStore trait
- `agent/library.rs` — agent CRUD using FileAgentStore
- `workflow/ports.rs` — FileWorkflowStore trait
- `workflow/catalog.rs` — workflow catalog
- `run/coordinator.rs` — run coordination
- `run/execution/` — execution details (grouped)

**✗ Don't do:**
```
agent/application/library.rs    ← unnecessary nesting
workflow/application/catalog.rs ← extra level
```

### 2. Adapters Folder (Centralized by Concern)

**What:** All concrete implementations of ports, grouped by technology concern.

**Rule:** Adapters go in `adapters/`, organized by **what they do**, not **what domain they serve**.

```
adapters/
├── storage/                ← persistence implementations
│   ├── agent_store.rs
│   ├── workflow_store.rs
│   └── ...
├── infrastructure/         ← external systems (LSP, Git, HTTP, DB clients)
│   ├── lsp/
│   ├── git/
│   └── http/
├── ai_provider/            ← (providers crate only) AI service implementations
│   ├── anthropic.rs
│   ├── openai.rs
│   └── ...
├── tool_impl/              ← tool-specific implementations
│   ├── edit/
│   └── ...
└── mod.rs
```

**Rule:** Never have nested adapters or adapters inside domains.

**✗ Don't do:**
```
agent/adapters/store.rs    ← adapters belong in adapters/, not domains
```

### 3. Ports (Interfaces)

**Where:** In domain-specific `ports.rs` file (not in adapter files).

**Rule:** Each domain defines the traits it depends on. Adapters implement those traits, never define them.

**Example:**
```rust
// agent/ports.rs (domain port definitions)
pub trait FileAgentStore {
    fn load(&self, id: &str) -> Result<AgentDefinition>;
    fn save(&mut self, def: AgentDefinition) -> Result<()>;
}
```

```rust
// agent/library.rs (domain logic uses the port)
use crate::agent::ports::FileAgentStore;

pub struct AgentLibrary {
    store: Box<dyn FileAgentStore>,
}
```

```rust
// adapters/storage/agent_store.rs (adapter implements the port)
use crate::agent::ports::FileAgentStore;

pub struct AgentFileStore { ... }

impl FileAgentStore for AgentFileStore { ... }
```

**Rule:** 
- ✅ Port traits defined in `domain/ports.rs`
- ✅ Domain logic imports and uses ports
- ✅ Adapters implement ports
- ✗ Adapters never define ports

---

## Crate-Specific Rules

### `crates/engine` — Core Domain

**What:** Domain model, workflow execution, execution state.

**Structure:**
```
engine/src/
├── conversation/       ← domain concept (chat history)
├── execution/         ← domain concept (run execution)
├── graph/             ← domain concept (workflow structure)
├── ports/             ← inbound/outbound ports for engine
├── template/          ← domain concept
├── tools/             ← domain concept (tool catalog, policies)
├── lib.rs
└── mod declarations
```

**Special case:** Engine defines its own `ports/inbound` and `ports/outbound` (boundaries for external systems). This is the exception—engine is the core and exports ports that others implement.

### `crates/orchestration` — Composition Root

**What:** Orchestrates domain concepts (agents, workflows, projects, tools, runs, settings) + adapters.

**Structure:**
```
orchestration/src/
├── agent/
│   ├── ports.rs                        ← FileAgentStore trait
│   └── library.rs                      ← agent CRUD
├── workflow/
│   ├── ports.rs                        ← FileWorkflowStore trait
│   └── catalog.rs                      ← workflow catalog
├── project/
│   ├── ports.rs                        ← FileProjectStore trait
│   └── registry.rs                     ← project registry
├── run/
│   ├── coordinator.rs                  ← run coordination
│   ├── execution/                      ← execution details
│   └── state/mod.rs                    ← state projection
├── settings/
│   ├── ports.rs                        ← FileSettingsStore trait
│   └── facade.rs                       ← settings aggregation
├── tool/
│   ├── mod.rs                          ← tool layer module
│   ├── registry.rs                     ← tool catalog
│   ├── runner.rs                       ← tool execution
│   └── output.rs                       ← artifact storage
│
├── adapters/
│   ├── storage/                        ← all persistence
│   │   ├── agent_store.rs
│   │   ├── workflow_store.rs
│   │   ├── project_store.rs
│   │   ├── settings_store.rs
│   │   ├── skill_store.rs
│   │   └── template_store.rs
│   ├── tool_impl/                      ← tool implementation (edit, patching)
│   │   ├── edit/
│   │   ├── errors.rs
│   │   └── mod.rs
│   └── infrastructure/                 ← external systems
│       ├── lsp/                        ← LSP protocol
│       └── git/                        ← Git CLI
│
├── backend/mod.rs                      ← composition root (wires all domains + adapters)
├── api.rs                              ← public API entry points
├── lib.rs                              ← module declarations
└── error.rs                            ← top-level errors
```

**Rules:**
- Domain folders (`agent/`, `workflow/`, etc.) contain logic files, not adapters
- All adapters centralized in `adapters/` by concern (storage, tool_impl, infrastructure)
- No persistence inside domain folders
- `backend/mod.rs` is the only place that directly depends on both domain logic AND adapters

### `crates/providers` — Adapter Crate

**What:** Implements `engine::ports::AiPort` for different AI providers.

**Structure:**
```
providers/src/
├── adapters/
│   ├── anthropic/
│   │   ├── client.rs
│   │   ├── model_list.rs
│   │   └── mod.rs
│   ├── openai/
│   │   └── ...
│   └── mod.rs
│
├── factory.rs                          ← single public factory function
├── lib.rs                              ← exports factory only
└── error.rs
```

**Rules:**
- Single public entry point: `create_provider()` factory function
- All concrete provider implementations in `adapters/`
- Never expose concrete provider types to consumers
- Implement `engine::ports::AiPort` trait

### `crates/ui` — Frontend (EXEMPT)

**What:** React/TypeScript frontend for the desktop app.

**Rules:** N/A — use standard web app conventions (components, pages, hooks, etc.)

### `crates/desktop` — Desktop App (EXEMPT)

**What:** Electron/Tauri desktop shell.

**Rules:** N/A — use desktop app conventions.

---

## Key Design Decisions

### Why Flat Domain Folders?

Avoids unnecessary nesting (`domain/application/logic.rs` → `domain/logic.rs`). Hexagonal boundary is clear through:
1. Files in domain folders = core logic
2. Files in `adapters/` = implementations
3. `lib.rs` declares public API

### Why Centralized Adapters?

Makes it easy to find implementations: "where is agent persistence?" → `adapters/storage/agent_store.rs`. 

Organized by **concern** (storage, infrastructure, tool_impl), not by domain. This prevents duplicated infrastructure code and makes it clear what technologies are being used.

### Why No Nested Adapters?

Adapters are terminal implementations. They don't have sub-adapters. Nesting (`agent/adapters/store.rs`) creates confusion because:
1. Adapters aren't supposed to depend on domains
2. It suggests there might be multiple layers (adapters of adapters)
3. Breaks the one-way dependency rule

### Why Single-Purpose Crates?

`providers` is purely adapters; `orchestration` orchestrates domains + adapters; `engine` is pure domain. This separation means:
- Easy to test each crate independently
- Clear responsibility per crate
- Easy to swap implementations (e.g., replace file storage with DB)

---

## Dependency Rules

```
engine (core domain)
  ↑
  └─ orchestration (domains + adapters)
       ├─ agent/library → adapters/storage/agent_store
       ├─ workflow/catalog → adapters/storage/workflow_store
       └─ tool/runner → adapters/tool_impl/
          
providers (adapters)
  ↑
  └─ orchestration (uses factory)

desktop/ui (frontend)
  ↑
  └─ orchestration (via IPC/API)
```

**Rule:** Each layer imports from layers below, never above. No circular dependencies.

---

## Applying the Rules: Checklist

When adding a new domain or adapter:

**New Domain:**
- [ ] Create folder: `domain_name/`
- [ ] Create `domain_name/ports.rs` — define all traits the domain depends on
- [ ] Add logic files at root: `domain_name/logic.rs` (imports from ports.rs)
- [ ] Update `lib.rs` to re-export domain entry points
- [ ] Add to composition root (`backend/mod.rs`)

**New Adapter:**
- [ ] Create folder in `adapters/concern_name/`
- [ ] Implement traits defined in `domain/ports.rs`
- [ ] Never define ports in adapters
- [ ] Never import the domain logic (only its ports)
- [ ] Update `adapters/mod.rs` if needed

**Refactoring Existing Code:**
- [ ] No nested `application/` folders → move to domain root
- [ ] Adapters out of domain folders → move to `adapters/`
- [ ] Organize adapters by concern, not domain
- [ ] Move trait definitions from adapters to `domain/ports.rs`
- [ ] Verify cargo check passes
- [ ] Update this document if new pattern emerges

## TODO: Port Refactoring

Current state: Trait definitions are in `adapters/` (incorrect).

Needed: Move all traits to `domain/ports.rs` files.

Example: `FileAgentStore` currently in `adapters/storage/agent_store.rs` should move to `agent/ports.rs`.

---

## References

- [CONTEXT.md](./CONTEXT.md) — Orchestration-specific terms and dependencies
- [AGENTS.md](./crates/orchestration/AGENTS.md) — How agents work (hexagonal example)
- Hexagonal Architecture: [Alistair Cockburn's original](https://alistair.cockburn.us/hexagonal-architecture/)
