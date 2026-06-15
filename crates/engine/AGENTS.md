---
description: Coding agent orientation for the engine crate
globs: crates/engine/**
alwaysApply: false
---

# AGENTS.md — Engine

**Question this crate answers:** What is a valid workflow, and how does a run behave?

Pure domain hexagon. No filesystem, HTTP, provider, or UI code.

## Architecture

```
┌─────────────────────────────────────────┐
│  graph/          Workflow model, DAG    │
│  execution/      Run semantics          │
│  ports/          Inbound + outbound     │
│  conversation/   Transcript types       │
│  template/       Node templates         │
│  tools/          Tool policy & catalog    │
└─────────────────────────────────────────┘
         ▲                    ▲
         │ implements         │ implements
  orchestration          providers
  (ToolPort, inbound)    (AiPort)
```

### Module map

| Path | Owns | Glossary terms |
| --- | --- | --- |
| `graph/` | `Workflow`, validation, layers, `CallableAgent` | Workflow, Node, Edge, execution layers |
| `execution/` | `InteractiveEngine`, `WorkflowRunner`, subagent runtime, telemetry | RunTelemetry, subagent builtins |
| `ports/outbound.rs` | `AiPort`, `ToolPort`, `AgentRequest` | LLM + tool seams |
| `ports/inbound.rs` | `HumanInputPort`, `ToolApprovalPort` | Pause/resume contracts |
| `conversation/` | `ChatMessage`, `AgentTranscriptItem` | Transcript DTOs |
| `template/` | `Template`, `TemplateStore` trait | Node presets |
| `tools/` | `NodeToolConfig`, approval tiers, truncation | ToolPolicy, ApprovalMode |

### Execution modes

| Type | Use | Behavior |
| --- | --- | --- |
| `WorkflowRunner` | Batch / headless | One AI turn per node; no tool loop or pauses |
| `InteractiveEngine` | Desktop app | Self-driving `run()`; calls `AiPort` + `ToolPort`; surfaces `NeedsInteraction` only |

Only **orchestration** `run/execution/` may construct engines. Engine never imports upward crates.

## Dependency rules

**Allowed:** `serde`, `async-trait`, `tokio` (minimal), `thiserror`

**Forbidden (CI-enforced):**
- `orchestration`, `providers`, `desktop`, `ui`
- `reqwest`, `tauri`, filesystem I/O
- Legacy names `domain`, `workflow_core`

## Ports

Add a port **only** when a consumer is typed on `dyn ThatPort`.

| Port | Direction | Implemented by |
| --- | --- | --- |
| `AiPort` | Outbound | `providers::AiClient` |
| `ToolPort` | Outbound | `orchestration::run::execution::tool_port` |
| `HumanInputPort` | Inbound | orchestration execution host |
| `ToolApprovalPort` | Inbound | orchestration execution host |

Provider-specific branching stays in `providers/`. Engine does not know which LLM is active.

## Code standards

1. **Pure logic** — delegate I/O to ports; no `std::fs`, no HTTP.
2. **Vocabulary** — use [`docs/glossary.md`](../../docs/glossary.md) terms (`CallableAgent`, not "saved subagent"; `RunTelemetry`, not "execution event").
3. **Determinism** — sort IDs where order affects behavior (see `validation.rs`, `workflow_runner.rs`).
4. **Errors** — `thiserror` enums; actionable messages; no panics outside tests.
5. **Constants** — name by intent at top of file (`NODE_RUNTIME_PREAMBLE`, tier labels).
6. **State ownership** — engine owns execution *semantics*; orchestration owns session *state*.

## Patterns

### Where to add code

| Change | Location |
| --- | --- |
| Workflow rule or DAG validation | `graph/validation.rs` |
| Workflow schema / settings | `graph/workflow.rs` |
| Batch run behavior | `execution/workflow_runner.rs` |
| Interactive pause/resume | `execution/interactive_engine/` |
| Subagent declare/call | `execution/subagent_runtime.rs` |
| Shared prompt assembly | `execution/node_invocation.rs` |
| Run event vocabulary | `execution/telemetry.rs` |
| New port contract | `ports/` |
| Tool tier / approval policy | `tools/config.rs` |
| New builtin tool guidance in prompts | Update `NODE_RUNTIME_PREAMBLE` in `node_invocation.rs` |

### Runtime semantics (engine side)

These rules live here; orchestration wires them — do not duplicate in UI:

1. **Shared context** — `WorkflowSettings.shared_context` merged into node/subagent system prompts.
2. **Callable agents** — `resolve_callable_agent_snapshots` in `graph/`; snapshotted IDs frozen at run start.
3. **Subagent builtins** — `openflow_declare_subagents`, `openflow_call_subagent` in subagent runtime.
4. **Validation** — `validate_workflow` is the single gate for graph legality.

### Testing

| Pattern | When |
| --- | --- |
| Inline `#[cfg(test)] mod tests` | Default — bottom of source file |
| Sibling `foo_tests.rs` | Test module exceeds ~150 lines |
| `tests.rs` in module dir | Integration tests for a subtree |

```bash
cargo test -p engine
cargo clippy -p engine -- -D warnings
```

Mock ports with inline `impl AiPort` / `impl ToolPort` stubs. Test behavior, not private helpers.

## Change checklist

1. Does this stay free of I/O and upward imports?
2. Is vocabulary aligned with `glossary.md`?
3. Did provider/orchestration-specific logic stay out of engine?
4. Are tests colocated and behavior-focused?
5. Run `./scripts/verify.sh test clippy arch` after changes.

## Related docs

- [`docs/sections/domain/README.md`](../../docs/sections/domain/README.md)
- [`docs/architecture/contract.md`](../../docs/architecture/contract.md)
- [`docs/FOLDER_STRUCTURE.md`](../../docs/FOLDER_STRUCTURE.md) — engine layout
- [`../../AGENTS.md`](../../AGENTS.md) — workspace map
