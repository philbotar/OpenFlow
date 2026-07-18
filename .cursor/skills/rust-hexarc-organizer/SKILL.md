---
name: rust-hexarc-organizer
description: >-
  Place OpenFlow changes in the correct crate and layer. Use when unsure which
  crate owns a change, whether a port is needed, or how to avoid layer
  violations across engine, providers, orchestration, desktop, and ui.
---

# rust-hexarc-organizer

Procedural placement guide. Authoritative rules: `docs/architecture/contract.md`.

## Decide the owner

| If the change is about… | Own it in |
| --- | --- |
| Valid workflow / how a run behaves | `crates/engine` |
| LLM wire format, auth, streaming | `crates/providers` |
| Store, load, wire, host runs, tools I/O | `crates/orchestration` |
| Tauri commands / events | `crates/desktop` |
| Presentation / DTOs / invoke wrappers | `crates/ui` |

## Seams (add a trait only when a consumer is typed on it)

| Seam | Defined in | Implemented in |
| --- | --- | --- |
| `AiPort` | `engine/ports/outbound.rs` | `providers` (`create_provider`) |
| `ToolPort` | `engine/ports/outbound.rs` | `orchestration/run/execution/tool_port.rs` |
| UI → desktop IPC | `ui/src/api.ts` (typed invoke/event wrappers; sole Tauri import site) | same file |

Otherwise call the concrete type.

## Hard bans

- Engine → no orchestration/providers/desktop/ui, no HTTP/fs
- Providers → no orchestration/desktop/ui
- Orchestration → no desktop/ui; no `providers::AiClient`
- UI → no Rust crate imports; Tauri only via `api.ts`
- `InteractiveEngine` constructed only under `orchestration/src/run/execution/`

## After placing

1. Read that crate’s `AGENTS.md`.
2. Load the matching lane skill (`openflow-*-change`).
3. Run `./scripts/check-architecture.sh` if boundaries moved.
4. Finish with `openflow-finish-change`.
