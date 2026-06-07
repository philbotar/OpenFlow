---
description: 
alwaysApply: true
---

# AGENTS.md

Single-file orientation for contributors and coding agents.

## 30-Second Intake

1. This is a Rust workspace with five sections: `domain`, `providers`, `orchestration`, `desktop`, `ui`.
2. Core rule: keep domain logic in `domain`; keep API transport/auth quirks in `providers`; keep runtime/state/storage in `orchestration`; keep Tauri adapter code in `desktop`; keep frontend code in `ui`.
3. Start reference docs at `/Users/philipbotar/Developer/Step-through-agentic-workflow/agent-reference-docs/README.md`.
4. Coding patterns and implementation rules are in `/Users/philipbotar/Developer/Step-through-agentic-workflow/agent-reference-docs/coding-patterns.md`.
5. Workflow acceptance and live-AI verification rules are in `/Users/philipbotar/Developer/Step-through-agentic-workflow/agent-reference-docs/testing-workflows.md`.

## Standard Module Layout

Use this structure consistently in each section crate so seam changes stay mechanical:

- `src/ports/inbound.*`
- `src/ports/outbound.*`
- `src/adapters/inbound.*`
- `src/adapters/outbound.*`

Notes:

- Ports are contracts owned by the section.
- Adapters are concrete implementations and transport wiring.
- Keep naming uniform across `domain`, `providers`, `orchestration`, `desktop`, and `ui`.

## Repo Map

| Path | Purpose | Change Here When... |
| --- | --- | --- |
| `Cargo.toml` | Workspace members and shared dependencies | Adding crates or shared dep versions |
| `crates/domain/src/ports/mod.rs` | Domain-owned LLM invocation contract | Changing `AiPort` request/response contract |
| `crates/domain/src/ports/inbound.rs` | Domain inbound seam declarations | Adding domain-facing entry contracts |
| `crates/domain/src/ports/outbound.rs` | Domain outbound seam declarations | Adding external capability contracts used by domain |
| `crates/domain/src/adapters/` | Domain-level adapter placeholders | Introducing domain-local adapter wiring |
| `crates/domain/src/model.rs` | Workflow data model (nodes/edges/messages/reports) | Changing schema or default node behavior |
| `crates/domain/src/validation.rs` | DAG validation + execution layers | Changing graph rules or scheduling rules |
| `crates/domain/src/runner.rs` | Non-interactive workflow execution | Changing execution semantics or upstream payload shape |
| `crates/domain/src/interactive.rs` | Interactive engine poll loop + human input pauses | Changing pause/resume behavior or per-node interaction |
| `crates/orchestration/src/ports/` | Orchestration seam declarations | Defining app-level inbound/outbound contracts |
| `crates/orchestration/src/adapters/` | Orchestration adapter implementations | Adding storage/provider/runtime adapter wiring |
| `crates/orchestration/src/provider_config.rs` | Provider readiness and API-key resolution | Changing key precedence, env fallback, or provider setup rules |
| `crates/orchestration/src/settings_store.rs` | App settings persistence (`settings.json`) | Changing settings schema, defaults, or provider profile fields |
| `crates/orchestration/src/state.rs` | App edit state + mutations | Changing selection, edge creation, schema editor, status tracking |
| `crates/orchestration/src/storage.rs` | Workflow persistence (`workflows.json`) | Changing workflow save/load format or location |
| `crates/providers/src/ports/` | Provider seam declarations | Defining provider-side contracts |
| `crates/providers/src/adapters/` | Provider transport adapters | Implementing OpenAI/Anthropic or other wire integrations |
| `crates/desktop/src-tauri/src/ports/` | Desktop seam declarations | Defining desktop-side contracts |
| `crates/desktop/src-tauri/src/adapters/` | Tauri transport adapters | Wiring commands/events to orchestration contracts |
| `crates/desktop/src-tauri/src/lib.rs` | Tauri commands/events and app bootstrap | Changing frontend/backend IPC or desktop startup |
| `crates/ui/src/ports/` | UI seam declarations | Defining UI-side contracts and entry capability expectations |
| `crates/ui/src/adapters/` | UI invoke/event adapters | Wiring view layer to desktop/runtime calls |
| `crates/ui/src/App.tsx` | Main Solid desktop shell | Changing layout, app interactions, sidebar/header/dock behavior |
| `crates/ui/src/api.ts` | Typed Tauri invoke/event wrappers | Changing frontend RPC names or payloads |
| `crates/ui/src/types.ts` | Frontend DTO mirror types | Changing command payload/result shapes |
| `examples/*.workflow.json` | Example workflows | Adding demos and smoke workflows |
| `docs/superpowers/plans/` | Historical implementation plans | Reviewing prior design intent and rollout details |

## Common Change Paths

| Goal | Primary Files |
| --- | --- |
| Add a workflow rule or validation | `domain/src/validation.rs`, tests in same file |
| Add a new model/backend adapter | New crate or module implementing `AiPort`, then wire in `crates/orchestration/src/lib.rs` |
| Add or change a seam contract | `*/src/ports/inbound.*`, `*/src/ports/outbound.*` in owning section |
| Add or change a concrete integration | `*/src/adapters/inbound.*`, `*/src/adapters/outbound.*` in owning section |
| Change canvas look/behavior | `crates/ui/src/canvas/`, `crates/ui/src/index.css` |
| Change inspector controls or spacing | `crates/ui/src/App.tsx`, `crates/ui/src/index.css` |
| Change settings UX or toast behavior | `crates/ui/src/App.tsx`, `crates/ui/src/api.ts`, `settings_store.rs` |
| Change provider config or key resolution | `crates/orchestration/src/provider_config.rs`, `settings_store.rs` |

## Dev Entry Points

- Full desktop app: `npm --prefix crates/desktop run start -- dev`
- Frontend only: `npm --prefix crates/ui run dev`
- Frontend typecheck: `npm --prefix crates/ui run typecheck`

## Runtime/Persistence Locations

- Workflow files save to `dirs::data_local_dir()/step-through-agentic-workflow/workflows.json`.
- Settings save to `dirs::data_local_dir()/step-through-agentic-workflow/settings.json`.
- Provider API keys are stored in the OS credential store/keychain using provider-specific key refs from settings.
- API key resolution order (highest to lowest): transient input panel → OS credential store → provider env var fallback (`OPENAI_API_KEY`, `ANTHROPIC_API_KEY`, etc.).

## Verification Commands

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets
cargo clippy-max
cargo test --workspace
```

## Standards Docs

- `/Users/philipbotar/Developer/Step-through-agentic-workflow/agent-reference-docs/README.md`
- `/Users/philipbotar/Developer/Step-through-agentic-workflow/agent-reference-docs/coding-patterns.md`
- `/Users/philipbotar/Developer/Step-through-agentic-workflow/agent-reference-docs/testing-workflows.md`
