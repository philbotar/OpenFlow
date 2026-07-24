---
name: openflow-engine-change
description: >-
  Procedural playbook for OpenFlow engine crate edits. Use when changing
  crates/engine/** — workflow graph, validation, InteractiveEngine, ports,
  tools policy, telemetry, templates, or completion protocol.
---

# openflow-engine-change

Procedural only. Architecture facts live in the docs below — do not invent a second model.

## Intake

1. Read `crates/engine/AGENTS.md`.
2. For run/pause/IPC context, skim `docs/architecture/end-to-end-runtime.md`.
3. Confirm vocabulary against `docs/glossary.md`.
4. Confirm layer rules in `docs/architecture/contract.md`.

## Placement rules

- Pure domain: no filesystem, HTTP, provider, or UI code.
- Ports only when a consumer is typed on `dyn ThatPort`.
- Provider quirks stay in `crates/providers`. Session/state stay in orchestration.
- Only orchestration `run/execution/` may construct `InteractiveEngine`.

## Where to edit

| Change | Path |
| --- | --- |
| DAG / schema / settings | `graph/` |
| Pause/resume loop | `execution/interactive_engine/` |
| Subagents | `execution/subagent_runtime.rs` |
| Prompt assembly | `execution/node_invocation.rs` |
| Events | `execution/telemetry.rs` |
| Final-output tool contract / schema validation | `execution/completion_protocol.rs` |
| `AiPort` / `ToolPort` | `ports/outbound.rs` |
| Tool tiers | `tools/config.rs` |

## Verify

```bash
./scripts/verify/test-engine.sh
./scripts/verify.sh test-fast clippy arch
```

Execution behavior that crosses the host: also run orchestration acceptance (see `openflow-finish-change`).
