# Codebase Structure

**Analysis Date:** 2026-05-30

## Directory Layout

```
[project-root]/
├── Cargo.toml                     # Workspace definition
├── crates/
│   ├── workflow-core/             # Domain model + execution engine
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── model.rs
│   │   │   ├── validation.rs
│   │   │   ├── runner.rs
│   │   │   ├── interactive.rs
│   │   │   └── ports.rs
│   │   └── Cargo.toml
│   ├── openai-client/             # OpenAI API adapter
│   │   ├── src/
│   │   │   └── lib.rs
│   │   └── Cargo.toml
│   └── agent-workflow-app/        # Desktop app (eframe + egui)
│       ├── src/
│       │   ├── main.rs
│       │   ├── lib.rs
│       │   ├── state.rs
│       │   ├── storage.rs
│       │   ├── settings_store.rs
│       │   ├── execution.rs
│       │   ├── provider_config.rs
│       │   ├── canvas_math.rs
│       │   └── ui/
│       │       ├── mod.rs
│       │       ├── canvas.rs
│       │       ├── inspector.rs
│       │       ├── settings.rs
│       │       ├── nav.rs
│       │       ├── widgets.rs
│       │       └── theme.rs
│       ├── tests/
│       │   ├── workflow_acceptance.rs
│       │   └── live_workflow.rs
│       └── Cargo.toml
├── examples/
│   └── feature_plan.workflow.json # Example workflow JSON
└── agent-reference-docs/          # Standards and patterns
    ├── README.md
    ├── coding-patterns.md
    ├── testing-workflows.md
    └── ui-styling-and-padding.md
```

## Directory Purposes

**`crates/workflow-core/src/`:**
- Purpose: Workflow graph schema, DAG validation, and both batch + interactive execution.
- Contains: Pure domain logic with no UI or networking dependencies.
- Key files: `model.rs`, `validation.rs`, `runner.rs`, `interactive.rs`, `ports.rs`

**`crates/openai-client/src/`:**
- Purpose: HTTP client implementing the `AiPort` trait for OpenAI and OpenAI-compatible endpoints.
- Contains: Single-file crate (`lib.rs`) with `OpenAiClient`, request/response extraction, and endpoint normalization.

**`crates/agent-workflow-app/src/`:**
- Purpose: Desktop application entry point, mutable state, persistence, provider resolution, and UI.
- Contains: `main.rs` (binary), `state.rs` (central mutable state), `storage.rs` / `settings_store.rs` (persistence), `execution.rs` (tokio bridge), `provider_config.rs` (env/ui merge), `canvas_math.rs` (geometry constants).
- Key files: `ui/mod.rs` (top-level app), `ui/canvas.rs` (custom painter), `ui/inspector.rs` (node properties).

**`crates/agent-workflow-app/tests/`:**
- Purpose: Integration and acceptance tests that exercise the full app stack.
- Contains: `workflow_acceptance.rs` (scripted AI headless run), `live_workflow.rs` (real OpenAI API smoke tests, ignored by default).

**`examples/`:**
- Purpose: Example workflow JSON files.
- Contains: `feature_plan.workflow.json`.

**`agent-reference-docs/`:**
- Purpose: Human-readable standards and coding patterns for contributors.
- Contains: `README.md`, `coding-patterns.md`, `testing-workflows.md`, `ui-styling-and-padding.md`.

## Key File Locations

**Entry Points:**
- `crates/agent-workflow-app/src/main.rs` — Desktop binary bootstrap.
- `crates/agent-workflow-app/src/lib.rs` — Library root, re-exports `WorkflowApp`.
- `crates/workflow-core/src/lib.rs` — Core library root.

**Configuration:**
- `Cargo.toml` — Workspace members and shared dependencies.
- `crates/*/Cargo.toml` — Per-crate dependencies.

**Core Logic:**
- `crates/workflow-core/src/model.rs` — Workflow data model.
- `crates/workflow-core/src/validation.rs` — DAG validation + topological layers.
- `crates/workflow-core/src/runner.rs` — Batch execution engine.
- `crates/workflow-core/src/interactive.rs` — Interactive execution with human pauses.

**UI:**
- `crates/agent-workflow-app/src/ui/mod.rs` — Top-level app state and event loop.
- `crates/agent-workflow-app/src/ui/canvas.rs` — Node/edge rendering and canvas interaction.
- `crates/agent-workflow-app/src/ui/inspector.rs` — Floating property panel.
- `crates/agent-workflow-app/src/ui/settings.rs` — Settings panel.
- `crates/agent-workflow-app/src/ui/nav.rs` — Left sidebar navigation.
- `crates/agent-workflow-app/src/ui/theme.rs` — Color palette and egui visuals.
- `crates/agent-workflow-app/src/ui/widgets.rs` — Shared UI helpers.

**Testing:**
- Co-located unit tests inside `#[cfg(test)] mod tests` in nearly every `.rs` file.
- Integration tests in `crates/agent-workflow-app/tests/`.

## Naming Conventions

**Files:**
- Modules use `snake_case.rs` (e.g., `settings_store.rs`, `provider_config.rs`).
- UI panel files match their function (`canvas.rs`, `inspector.rs`, `settings.rs`).

**Directories:**
- Crate names use `kebab-case` (`workflow-core`, `openai-client`, `agent-workflow-app`).
- Source directories follow standard Cargo layout (`src/`, `tests/`).

**Types:**
- Id newtypes are tuple structs: `NodeId(pub String)`, `EdgeId(pub String)`, `WorkflowId(pub String)`.
- Status enums use `PascalCase` with explicit labels: `AgentStatus`, `TraceStatus`, `RunEventKind`.
- Output structs for UI panels use `Output` suffix: `InspectorOutput`, `NavOutput`, `BottomPanelOutput`.

## Where to Add New Code

**New Feature (domain logic):**
- Primary code: `crates/workflow-core/src/`
- Tests: same file under `#[cfg(test)] mod tests`

**New Component/Module (UI panel):**
- Implementation: `crates/agent-workflow-app/src/ui/{panel_name}.rs`
- Registration: add `mod {panel_name};` to `crates/agent-workflow-app/src/ui/mod.rs`

**New AI Backend Adapter:**
- Implementation: either new crate or `crates/openai-client/src/lib.rs` if extending existing adapter.
- Trait implementation: implement `AiPort` from `workflow-core`.
- Wiring: construct adapter in `WorkflowApp::start_interactive_execution()` inside `ui/mod.rs`.

**New Persistence Format:**
- Change: `crates/agent-workflow-app/src/storage.rs` (workflows) or `src/settings_store.rs` (settings).
- Provide legacy migration in `FileSettingsStore::load()` if schema changes.

**New Example Workflow:**
- Location: `examples/*.workflow.json`

**New Acceptance/Test:**
- Location: `crates/agent-workflow-app/tests/`

## Special Directories

**`crates/agent-workflow-app/assets/`:**
- Purpose: Contains `Nunito-Regular.ttf` embedded at compile time via `include_bytes!` in `main.rs`.
- Generated: No
- Committed: Yes

**`examples/`:**
- Purpose: Example workflow JSON files for demos and smoke tests.
- Generated: No
- Committed: Yes

**`agent-reference-docs/`:**
- Purpose: Contributor-facing standards documentation.
- Generated: No
- Committed: Yes

---

*Structure analysis: 2026-05-30*
