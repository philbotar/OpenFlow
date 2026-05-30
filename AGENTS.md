# AGENTS.md

Single-file orientation for contributors and coding agents.

## 30-Second Intake

1. This is a Rust workspace with three crates: `workflow-core`, `openai-client`, `agent-workflow-app`.
2. Core rule: keep domain logic in `workflow-core`; keep API transport in `openai-client`; keep UI/state/storage in `agent-workflow-app`.
3. Start reference docs at `/Users/philipbotar/Developer/Step-through-agentic-workflow/agent-reference-docs/README.md`.
4. Coding patterns and implementation rules are in `/Users/philipbotar/Developer/Step-through-agentic-workflow/agent-reference-docs/coding-patterns.md`.
5. Workflow acceptance and live-AI verification rules are in `/Users/philipbotar/Developer/Step-through-agentic-workflow/agent-reference-docs/testing-workflows.md`.
6. UI styling and spacing standards are in `/Users/philipbotar/Developer/Step-through-agentic-workflow/agent-reference-docs/ui-styling-and-padding.md`.

## Repo Map

| Path | Purpose | Change Here When... |
| --- | --- | --- |
| `Cargo.toml` | Workspace members and shared dependencies | Adding crates or shared dep versions |
| `crates/workflow-core/src/model.rs` | Workflow data model (nodes/edges/messages/reports) | Changing schema or default node behavior |
| `crates/workflow-core/src/validation.rs` | DAG validation + execution layers | Changing graph rules or scheduling rules |
| `crates/workflow-core/src/runner.rs` | Non-interactive workflow execution | Changing execution semantics or upstream payload shape |
| `crates/workflow-core/src/interactive.rs` | Interactive engine poll loop + human input pauses | Changing pause/resume behavior or per-node interaction |
| `crates/workflow-core/src/ports.rs` | AI boundary trait (`AiPort`) + request/response DTOs | Changing AI contract between core and adapters |
| `crates/openai-client/src/lib.rs` | OpenAI Responses API adapter | Changing HTTP request/response mapping |
| `crates/agent-workflow-app/src/ui/` | UI panels, canvas, theme, inspector, settings | Changing visuals, layout, interactions, keyboard behavior |
| `crates/agent-workflow-app/src/state.rs` | App edit state + mutations | Changing selection, edge creation, schema editor, status tracking |
| `crates/agent-workflow-app/src/storage.rs` | Workflow persistence (`workflows.json`) | Changing workflow save/load format or location |
| `crates/agent-workflow-app/src/settings_store.rs` | App settings persistence (`settings.json`) | Changing settings schema or defaults |
| `crates/agent-workflow-app/src/main.rs` | Desktop bootstrapping + viewport + fonts | Changing app startup, window defaults, or font loading |
| `examples/*.workflow.json` | Example workflows | Adding demos and smoke workflows |
| `docs/superpowers/plans/` | Historical implementation plans | Reviewing prior design intent and rollout details |

## Common Change Paths

| Goal | Primary Files |
| --- | --- |
| Add a workflow rule or validation | `workflow-core/src/validation.rs`, tests in same file |
| Change what each node receives as input | `workflow-core/src/runner.rs` (`build_node_input`) and `interactive.rs` |
| Add a new model/backend adapter | New crate or module implementing `AiPort`, then wire in `agent-workflow-app/src/ui/mod.rs` |
| Change canvas look/behavior | `agent-workflow-app/src/ui/canvas.rs`, `ui/theme.rs`, `canvas_math.rs` |
| Change inspector controls or spacing | `agent-workflow-app/src/ui/inspector.rs`, `ui/widgets.rs`, `ui/theme.rs` |
| Change settings UX or defaults | `agent-workflow-app/src/ui/settings.rs`, `settings_store.rs` |

## Runtime/Persistence Locations

- Workflow files save to `dirs::data_local_dir()/step-through-agentic-workflow/workflows.json`.
- Settings save to `dirs::data_local_dir()/step-through-agentic-workflow/settings.json`.
- API key precedence: UI settings value first, `OPENAI_API_KEY` fallback.

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
- `/Users/philipbotar/Developer/Step-through-agentic-workflow/agent-reference-docs/ui-styling-and-padding.md`
