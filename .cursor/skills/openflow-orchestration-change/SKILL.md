---
name: openflow-orchestration-change
description: >-
  Procedural playbook for OpenFlow orchestration edits. Use when changing
  crates/orchestration/** — run lifecycle, AppBackend, execution host,
  ToolPortImpl, persistence, settings, tools, or workflow catalog.
---

# openflow-orchestration-change

Procedural only. Architecture facts live in the docs below — do not invent a second model.

## Intake

1. Read `crates/orchestration/AGENTS.md`.
2. For module map: `docs/architecture/orchestration-layout.md`.
3. For run path: `docs/architecture/end-to-end-runtime.md`.
4. For dual runtime / mutex: `docs/architecture/threading-concurrency.md`.
5. Confirm `docs/architecture/contract.md` if crossing crates.

## Placement rules

- Domain folders never `use crate::adapters::`.
- Construct `InteractiveEngine` only in `run/execution/`.
- Call `create_provider()` — never `use providers::AiClient`.
- No `desktop` / `ui` / `tauri` imports.

## Where to edit

| Change | Path |
| --- | --- |
| IPC surface | `backend/` → entity folder |
| Run start / input / approval | `run/coordinator/`, `run/execution/` |
| Tool I/O | `run/execution/tool_port.rs`, `adapters/tool_impl/` |
| Persistence | `adapters/storage/` + `{entity}/ports.rs` |
| Settings / keys | `settings/` |

## Verify

```bash
cargo nextest run -p orchestration --lib
./scripts/check-architecture.sh
```

When execution behavior changes:

```bash
cargo nextest run -p orchestration --test workflow_acceptance --no-capture
```

Before handoff: load `openflow-finish-change`.
