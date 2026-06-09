# Architecture Contract

Step-through uses **Hexagonal Architecture with Layers** — nested ports-and-adapters where each layer is both an adapter (for the layer above) and a provider of services (to the layer below).

```
┌─────────────────────────────────────────────────┐
│  UI (crates/ui)                                 │
│  Presentation — visual state, interaction       │
└──────────────────┬──────────────────────────────┘
                   │
                   │ implements Desktop inbound port
                   │ (invokes Tauri commands)
                   ▼
┌──────────────────────────────────────────────────┐
│  Desktop (crates/desktop)                        │
│  Tauri Adapter — IPC transport, DTO mapping      │
└──────────────────┬───────────────────────────────┘
                   │
                   │ implements Orchestration inbound port
                   │ (calls AppBackend)
                   ▼
┌──────────────────────────────────────────────────┐
│  Orchestration (crates/orchestration)            │
│  Application — run lifecycle, coordination       │
└──────────────────┬───────────────────────────────┘
          ╱        │        ╲
    entity/    backend/   adapters/
    services   composition   drivers
         │        │            │
         └────────┼────────────┘
                  │
        ┌─────────┴──────────┐
        │ implements         │
        │ domain ports       │
        ▼                    ▼
    Domain (hex)        Providers (adapter)
    Business logic      LLM transport
```

## Layer Definitions

### 1. **Domain (crates/domain)**
- **Role:** Hexagon — pure business logic
- **Scope:** Workflow model, invariants, transitions, execution semantics, ports (inbound + outbound)
- **Public interface:** Traits in `ports/` + model types
- **Dependencies:** None upward; only serialization, async traits

### 2. **Providers (crates/providers)**
- **Role:** Outbound adapter — implements `domain::AiPort`
- **Scope:** LLM protocol/auth/transport
- **Public interface:** `create_provider()` → `Box<dyn AiPort>`
- **Dependencies:** `domain` types, HTTP client, serde

### 3. **Orchestration (crates/orchestration)**
- **Role:** Inbound adapter for Domain; outbound provider for Desktop
- **Scope:** Run lifecycle, session state, coordination, approval loops, event fanout
- **Sub-roles:**
  - `backend/` — composition root; wires services and adapters
  - `{entity}/application/` — service; coordinates domain + ports
  - `{entity}/adapters/` — repository; persistence and file I/O
  - `adapters/infrastructure/` — drivers; tool/git/LSP execution
- **Public interface:** `AppBackend` — façade that Desktop calls
- **Dependencies:** `domain` + `providers`, no upward

### 4. **Desktop (crates/desktop)**
- **Role:** Inbound adapter for UI; calls Orchestration
- **Scope:** Tauri transport only — IPC wiring, command serialization, DTOs
- **Public interface:** Tauri command handlers that map to Orchestration calls
- **Dependencies:** `orchestration::AppBackend`, no upward

### 5. **UI (crates/ui)**
- **Role:** Presentation layer; implements Desktop's command interface
- **Scope:** Visual state, interaction, rendering
- **Public interface:** React components, TypeScript types
- **Dependencies:** Tauri invoke client (to Desktop), no upward

## State Ownership

### Orchestration
- Active session/run state (approval queues, retry counters, execution trace)
- Runtime coordination (what step to run next, when to pause for approval)
- NOT: business logic (domain owns that)

### Domain
- Model types and invariants (Workflow, Node, ToolCall, ToolResult)
- Legal transitions and validation rules
- Execution semantics (when does an engine advance, what are error states)

### Infrastructure / Persistence
- Durable state (workflows.json, settings.json, agent definitions)
- Credentials, cache
- NOT: runtime session state (that's orchestration)

## Dependency Rules

**Allowed (dependencies point inward):**
- UI → Desktop (invoke Tauri commands)
- Desktop → Orchestration (call `AppBackend` methods)
- Orchestration → Domain (use model types, call engine)
- Orchestration → Providers (via `Box<dyn AiPort>`, no concrete types)
- Providers → Domain (implements `AiPort` trait)

**Forbidden:**
- UI → Domain or Providers (bypass orchestration)
- UI → Orchestration directly (go through Desktop)
- Desktop → Domain or Providers (go through Orchestration)
- Desktop → Orchestration internals (`{entity}/application/adapters/`); only `AppBackend` public façade
- Orchestration → UI or Desktop (upward)
- Domain → anything outward (no imports of provider, orchestration, UI, desktop)
- Providers → UI or Desktop

**Rationale:** Each layer is an adapter for the layer above it and depends only on the layer below.

## Engine Invocation Rule

- Only `orchestration/run/application/coordinator.rs` may invoke `InteractiveEngine` or `WorkflowRunner`.
- UI never calls engine directly.
- Desktop never calls engine directly.
- Providers never call engine.
- This rule prevents accidental state machine violations and ensures orchestration owns the run lifecycle.

## Port Rule

Domain defines ports (traits) for:
- **Inbound:** What orchestration must implement for domain (e.g., `HumanInputPort`, `ToolApprovalPort`)
- **Outbound:** What domain requires from external systems (e.g., `AiPort`)

Orchestration implements both inbound ports and calls outbound ports via `Box<dyn>`.

Provider-specific branching stays in `providers/`. Domain does not know which LLM is being called.

UI-to-Desktop contract via `UiDesktopOutboundPort` (TypeScript trait). Add a new port only when code is typed on `dyn ThatPort`.

## Testability Rule

- **UI tests:** Mock `UiDesktopOutboundPort` (Tauri invoke mock)
- **Desktop tests:** Mock `AppBackend` methods
- **Orchestration tests:** Inline `impl AiPort` stubs; use acceptance fixtures for critical paths
- **Provider tests:** Verify wire format mapping; test `AiClient` contract compliance
- **Domain tests:** Colocated unit tests for engine and model invariants

## Adapter Pattern (Nested Hexagons)

Each layer implements the layer above's "inbound port":
- UI implements Desktop's command interface (which commands are available)
- Desktop implements Orchestration's façade interface (which methods Desktop can call)
- Orchestration implements Domain's requirements (which ports orchestration provides for engine)

This is **nested ports-and-adapters**, not pure hex-arc, but follows the same dependency-points-inward principle.

## Change Review Checklist

1. Does this change add a dependency that violates allowed/forbidden rules?
2. Does UI/Desktop bypass layers to call domain/provider code?
3. Did provider-specific logic leak into Orchestration or Domain?
4. Does any new runtime state live outside Orchestration without justification?
5. Are new public interfaces declared at the correct seam (Desktop vs Orchestration vs Domain)?
6. Does Domain remain free of I/O or external crate imports?
7. Do Desktop/UI contain only transport logic, not orchestration or domain rules?

## Design Notes

1. **Nested adapters:** UI → Desktop → Orchestration → Domain. Each layer is an adapter for the layer above.
2. **Orchestration is thick:** It owns run state, approval/retry loops, event fanout. Not a thin layer.
3. **Domain is pure:** No I/O, no provider knowledge, no runtime state. Only model, rules, and ports.
4. **Providers are swappable:** Orchestration depends on `dyn AiPort`, not concrete implementations.
5. **Desktop is thin:** Only Tauri IPC and DTO mapping. All logic lives in Orchestration or Domain.
