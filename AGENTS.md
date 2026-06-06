# AGENTS.md

Single-file orientation for contributors and coding agents.

## 30-Second Intake

1. This is a Rust workspace with four crates: `workflow-core`, `ai`, `app-backend`, `agent-workflow-desktop`.
2. Core rule: keep domain logic in `workflow-core`; keep API transport/auth quirks in `ai`; keep UI/state/storage in `app-backend` and Tauri/Solid desktop code in `agent-workflow-desktop`.
3. Start reference docs at `/Users/philipbotar/Developer/Step-through-agentic-workflow/agent-reference-docs/README.md`.
4. Coding patterns and implementation rules are in `/Users/philipbotar/Developer/Step-through-agentic-workflow/agent-reference-docs/coding-patterns.md`.
5. Workflow acceptance and live-AI verification rules are in `/Users/philipbotar/Developer/Step-through-agentic-workflow/agent-reference-docs/testing-workflows.md`.

## Repo Map

| Path | Purpose | Change Here When... |
| --- | --- | --- |
| `Cargo.toml` | Workspace members and shared dependencies | Adding crates or shared dep versions |
| `crates/workflow-core/src/model.rs` | Workflow data model (nodes/edges/messages/reports) | Changing schema or default node behavior |
| `crates/workflow-core/src/validation.rs` | DAG validation + execution layers | Changing graph rules or scheduling rules |
| `crates/workflow-core/src/runner.rs` | Non-interactive workflow execution | Changing execution semantics or upstream payload shape |
| `crates/workflow-core/src/interactive.rs` | Interactive engine poll loop + human input pauses | Changing pause/resume behavior or per-node interaction |
| `crates/app-backend/src/provider_config.rs` | Provider readiness and API-key resolution | Changing key precedence, env fallback, or provider setup rules |
| `crates/app-backend/src/settings_store.rs` | App settings persistence (`settings.json`) | Changing settings schema, defaults, or provider profile fields |
| `crates/app-backend/src/state.rs` | App edit state + mutations | Changing selection, edge creation, schema editor, status tracking |
| `crates/app-backend/src/storage.rs` | Workflow persistence (`workflows.json`) | Changing workflow save/load format or location |
| `crates/agent-workflow-desktop/src-tauri/src/lib.rs` | Tauri commands/events and app bootstrap | Changing frontend/backend IPC or desktop startup |
| `crates/agent-workflow-desktop/src/App.tsx` | Main Solid desktop shell | Changing layout, app interactions, sidebar/header/dock behavior |
| `crates/agent-workflow-desktop/src/api.ts` | Typed Tauri invoke/event wrappers | Changing frontend RPC names or payloads |
| `crates/agent-workflow-desktop/src/types.ts` | Frontend DTO mirror types | Changing command payload/result shapes |
| `examples/*.workflow.json` | Example workflows | Adding demos and smoke workflows |
| `docs/superpowers/plans/` | Historical implementation plans | Reviewing prior design intent and rollout details |
## Common Change Paths

| Goal | Primary Files |
| --- | --- |
| Add a workflow rule or validation | `workflow-core/src/validation.rs`, tests in same file |
| Add a new model/backend adapter | New crate or module implementing `AiPort`, then wire in `crates/app-backend/src/lib.rs` |
| Change canvas look/behavior | `agent-workflow-desktop/src/canvas/`, `index.css` |
| Change inspector controls or spacing | `agent-workflow-desktop/src/App.tsx`, `index.css` |
| Change settings UX or toast behavior | `agent-workflow-desktop/src/App.tsx`, `api.ts`, `settings_store.rs` |
| Change provider config or key resolution | `crates/app-backend/src/provider_config.rs`, `settings_store.rs` |
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