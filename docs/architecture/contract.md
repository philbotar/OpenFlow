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
        │ engine ports       │
        ▼                    ▼
    Engine (hex)        Providers (adapter)
    Execution engine    LLM transport
```

## Crate roles (one line each)

| Crate | Question it answers |
| --- | --- |
| **engine** | What is a valid workflow, and how does a run behave? |
| **orchestration** | How does the desktop app store, load, wire, and host runs? |
| **providers** | How do we talk to OpenAI/Anthropic? |
| **ui** / **desktop** | How does the user interact? |

**engine** holds product rules (graph validation, execution semantics, tool approval policy). **orchestration** holds app rules (persistence, catalog merge, run sessions, tool I/O). **providers** implements `AiPort`. **ui** and **desktop** are inbound adapters only.

## Layer Definitions

### 1. **Engine (crates/engine)**
- **Role:** Hexagon — workflow execution engine
- **Scope:** Workflow model, state machine, ports (`AiPort`, `ToolPort`, `HumanInputPort`, `ToolApprovalPort`); self-driving `InteractiveEngine::run()` calls `AiPort` and `ToolPort` internally; surfaces only interaction pauses (`NeedsInteraction`) to orchestration
- **Public interface:** Traits in `ports/` + model types
- **Dependencies:** None upward; only serialization, async traits, tokio

### 2. **Providers (crates/providers)**
- **Role:** Outbound adapter — implements `engine::AiPort`
- **Scope:** LLM protocol/auth/transport
- **Public interface:** `create_provider()` → `Box<dyn AiPort>`
- **Dependencies:** `engine` types, HTTP client, serde

### 3. **Orchestration (crates/orchestration)**
- **Role:** Inbound adapter for Engine; outbound provider for Desktop
- **Scope:** Run lifecycle, session state, coordination, approval/input loops, event fanout; `ToolPortImpl` executes tools and subagents
- **Sub-roles:**
  - `backend/` — composition root; wires services and adapters
  - `{entity}/application/` — service; coordinates engine + ports
  - `{entity}/adapters/` — repository; persistence and file I/O
  - `adapters/infrastructure/` — drivers; tool/git/LSP execution
- **Public interface:** `AppBackend` — façade that Desktop calls
- **Dependencies:** `engine` + `providers`, no upward

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
- NOT: execution semantics (engine owns that)

### Engine
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
- Orchestration → Engine (use model types, drive `InteractiveEngine::run`)
- Orchestration → Providers (via `Box<dyn AiPort>`, no concrete types)
- Providers → Engine (implements `AiPort` trait)

**Forbidden:**
- UI → Engine or Providers (bypass orchestration)
- UI → Orchestration directly (go through Desktop)
- Desktop → Engine or Providers (go through Orchestration)
- Desktop → Orchestration internals (`{entity}/application/adapters/`); only `AppBackend` public façade
- Orchestration → UI or Desktop (upward)
- Engine → anything outward (no imports of provider, orchestration, UI, desktop)
- Providers → UI or Desktop

**Rationale:** Each layer is an adapter for the layer above it and depends only on the layer below.

## CI enforcement

Checks run in CI via `./scripts/check-architecture.sh`. Rules live in [`arch-check-rules.toml`](arch-check-rules.toml).

### Tier 2 (Phase A) — inter-crate

1. **Workspace dependency graph** — path deps in each crate `Cargo.toml` match the allowed inward graph.
2. **Forbidden cross-crate `use`** — per-crate ban tables (e.g. `desktop` must not `use engine::`).
3. **Engine forbidden deps** — `engine` must not depend on transport/GUI crates (`reqwest`, `tauri`, …).
4. **Legacy crate aliases** — `domain` and `workflow_core` banned in all workspace `use` paths.

### Tier 3 (Phase B) — seams and layout

1. **`orchestration → providers` allowlist** — `orchestration/src` may import only listed config/factory symbols; `AiClient` is banned (use `create_provider`).
2. **Engine invocation locality** — only `orchestration/src/run/execution/` may call `InteractiveEngine::new` / `WorkflowRunner::new`.
3. **Orchestration domain folders** — `agent/`, `workflow/`, `project/`, `settings/`, `tool/` must not `use crate::adapters::`.
4. **UI Tauri seam** — `@tauri-apps/*` imports only in `api.ts`, `port.ts`, and test mocks.

4. **Orchestration domain store ban** — `agent/`, `workflow/`, `project/`, `settings/`, `tool/` must not `use crate::{agent_store, flow_store, …}`; depend on port traits; wire `File*Store` in `backend/`.

Deferred: `tool/` → `lsp` narrowing; `providers → engine` submodule allowlist. See `CONTEXT.md` → **Architecture check rollout**.

## Engine Invocation Rule

- Only `orchestration/run/execution/` may construct `InteractiveEngine` or `WorkflowRunner`.
- Interactive runs call `InteractiveEngine::run()`; orchestration handles only `NeedsInteraction` and terminal outcomes.
- Tool and subagent execution goes through `ToolPortImpl` (`engine::ToolPort`).
- UI, Desktop, and Providers never call the engine directly.

## Port Rule

Engine defines ports (traits) for:
- **Inbound:** What orchestration must implement for the engine (e.g., `HumanInputPort`, `ToolApprovalPort`)
- **Outbound:** What the engine requires from external systems (`AiPort`, `ToolPort`)

Orchestration implements inbound ports, `ToolPortImpl`, and calls `AiPort` via `Box<dyn>`.

Provider-specific branching stays in `providers/`. Engine does not know which LLM is being called.

UI-to-Desktop contract via `UiDesktopOutboundPort` (TypeScript trait). Add a new port only when code is typed on `dyn ThatPort`.

## Testability Rule

- **UI tests:** Mock `UiDesktopOutboundPort` (Tauri invoke mock)
- **Desktop tests:** Mock `AppBackend` methods
- **Orchestration tests:** Inline `impl AiPort` stubs; use acceptance fixtures for critical paths
- **Provider tests:** Verify wire format mapping; test `AiClient` contract compliance
- **Engine tests:** Colocated unit tests for state machine and model invariants

## Adapter Pattern (Nested Hexagons)

Each layer implements the layer above's "inbound port":
- UI implements Desktop's command interface (which commands are available)
- Desktop implements Orchestration's façade interface (which methods Desktop can call)
- Orchestration implements Engine's requirements (which ports orchestration provides)

This is **nested ports-and-adapters**, not pure hex-arc, but follows the same dependency-points-inward principle.

## Change Review Checklist

1. Does this change add a dependency that violates allowed/forbidden rules?
2. Does UI/Desktop bypass layers to call engine/provider code?
3. Did provider-specific logic leak into Orchestration or Engine?
4. Does any new runtime state live outside Orchestration without justification?
5. Are new public interfaces declared at the correct seam (Desktop vs Orchestration vs Engine)?
6. Does Engine remain free of filesystem/tool I/O (delegated to `ToolPort`)?
7. Do Desktop/UI contain only transport logic, not orchestration or engine rules?

## Design Notes

1. **Nested adapters:** UI → Desktop → Orchestration → Engine. Each layer is an adapter for the layer above.
2. **Orchestration coordinates:** Run state, approval/input loops, event fanout; `drive.rs` is thin around `engine.run()`.
3. **Engine is self-driving:** Calls `AiPort` and `ToolPort` internally; no filesystem or provider I/O in the crate.
4. **Providers are swappable:** Orchestration depends on `dyn AiPort`, not concrete implementations.
5. **Desktop is thin:** Only Tauri IPC and DTO mapping. All logic lives in Orchestration or Engine.
