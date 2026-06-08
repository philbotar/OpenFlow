# Architecture Contract

This contract defines layer responsibilities and dependency rules for Step-through-agentic-workflow.

## Layer Model

1. UI
- Scope: visual state, interaction, rendering.
- Code: crates/ui/src.

2. Desktop Adapter
- Scope: Tauri transport adapter only (invoke/event wiring, serialization mapping).
- Code: crates/desktop/src-tauri.

3. Orchestration
- Scope: run lifecycle, coordination, retries, approval loops, event fanout.
- Code: crates/orchestration.
- Composition root: `AppBackend` wires catalog modules (`WorkflowCatalog`, `AgentLibrary`, `ProjectRegistry`, `SettingsFacade`, `RunCoordinator`). Backend delegates; it does not embed merge/split or session logic.

4. Domain Engine
- Scope: workflow model, invariants, transitions, execution semantics, ports.
- Code: crates/domain.

5. Provider Adapters
- Scope: concrete model-provider protocol/auth/transport implementations.
- Code: crates/providers.

## State Ownership

1. Orchestration owns runtime execution state.
- Active session/run state.
- Tool approval queues.
- Retry/backoff and lifecycle control.

2. Domain owns state model and invariants.
- Types and legal transitions.
- Validation and execution rules.

3. Persistence/infrastructure owns durable state.
- Workflow/settings/templates files.
- Credential storage integration.

## Dependency Rules (Allowed)

1. UI -> Desktop Adapter.
2. Desktop Adapter -> Orchestration.
3. Orchestration -> Domain Engine.
4. Orchestration -> Provider Adapters via Domain-defined ports.
5. Provider Adapters -> Domain Engine (implements AiPort and related contracts).

## Dependency Rules (Forbidden)

1. UI must not call Domain Engine or Provider Adapters directly.
2. Desktop Adapter must not call Domain Engine directly.
3. Desktop Adapter must not contain orchestration policy.
4. Domain Engine must not import Provider Adapter transport/auth details.
5. Provider Adapters must not depend on UI/Desktop modules.

## Engine Invocation Rule

1. Engine calls are orchestrator-only.
- Only Orchestration may invoke InteractiveEngine/runner entry points.
- UI/Desktop/Providers do not invoke engine entry points.

## Port Rule

1. Orchestration must depend on interfaces, not provider internals.
- Provider-specific branching belongs in the `providers` crate.
- Domain defines provider interaction contracts (`AiPort`).
- UI depends on `UiDesktopOutboundPort`, not raw Tauri invoke details.
- Add a new port only when a consumer is typed on that interface.

## Testability Rule

1. Every layer must be testable at its seam.
- UI tests mock `UiDesktopOutboundPort`.
- Orchestration tests use inline `impl AiPort` stubs or acceptance fixtures.
- Provider tests verify wire mapping and `AiClient` contract compliance.

## Change Review Checklist

1. Does this change add a dependency that violates allowed/forbidden rules?
2. Does UI/Desktop bypass Orchestration to call engine/provider code?
3. Did provider-specific logic leak into Orchestration or Domain?
4. Does any new runtime state live outside Orchestration without justification?
5. Are new public interfaces declared at the correct seam?

## Decision Notes

1. "Orchestration holds all state" means runtime execution state, not all domain data forever.
2. Domain remains provider-agnostic and transport-agnostic.
3. Provider Adapters remain swappable without changing orchestration behavior.
