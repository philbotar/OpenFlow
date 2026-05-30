<!-- refreshed: 2026-05-30 -->
# Architecture

**Analysis Date:** 2026-05-30

## System Overview

This is a Rust workspace with three crates organized by responsibility:

```text
┌─────────────────────────────────────────────────────────────────────────────┐
│                        Desktop Application (UI)                               │
│              agent-workflow-app — egui + eframe + tokio                       │
│  `src/ui/`  `src/state.rs`  `src/execution.rs`  `src/storage.rs`            │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    │ uses
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                        API Transport / AI Adapter                           │
│                    openai-client — reqwest + async_trait                    │
│                      `src/lib.rs` (implements AiPort)                       │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    │ implements trait
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                      Workflow Domain / Execution Engine                     │
│              workflow-core — serde, uuid, futures, thiserror                │
│  `src/model.rs`  `src/validation.rs`  `src/runner.rs`  `src/interactive.rs`  │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Design rule:** Domain logic remains in `workflow-core`. API transport stays in `openai-client`. UI, state, storage, and desktop bootstrapping live in `agent-workflow-app`.

## Component Responsibilities

| Component | Responsibility | File |
|-----------|----------------|------|
| Model | Workflow graph schema (nodes, edges, messages, reports) | `crates/workflow-core/src/model.rs` |
| Validation | DAG validation + topological layer computation | `crates/workflow-core/src/validation.rs` |
| Runner | Non-interactive batch workflow execution | `crates/workflow-core/src/runner.rs` |
| Interactive Engine | Poll-based interactive execution with human-input pauses | `crates/workflow-core/src/interactive.rs` |
| AI Port | Trait boundary between core and adapters | `crates/workflow-core/src/ports.rs` |
| OpenAI Client | OpenAI Responses API and Chat Completions adapter | `crates/openai-client/src/lib.rs` |
| App State | Mutable edit state (selection, edges, schema, status) | `crates/agent-workflow-app/src/state.rs` |
| Execution Glue | Tokio channel bridge between UI and `InteractiveEngine` | `crates/agent-workflow-app/src/execution.rs` |
| Canvas | Custom egui painter for nodes, edges, dot grid | `crates/agent-workflow-app/src/ui/canvas.rs` |
| Inspector | Floating property panel for selected node | `crates/agent-workflow-app/src/ui/inspector.rs` |
| Settings | Provider config, API keys, model lists | `crates/agent-workflow-app/src/ui/settings.rs` |
| Storage | JSON persistence for workflows and app settings | `crates/agent-workflow-app/src/storage.rs`, `src/settings_store.rs` |

## Pattern Overview

**Overall:** Clean layered architecture with trait-based adapter boundary.

**Key Characteristics:**
- **Trait-based AI boundary:** `AiPort` in `workflow-core` allows swapping backends without changing core execution logic.
- **Two execution modes:** Batch (`WorkflowRunner`) and interactive (`InteractiveEngine`) share the same validation and layer scheduling but differ in control flow.
- **Immediate-mode UI:** All rendering is done inside a single `eframe::App::update` tick with explicit event polling from a background Tokio task.
- **Single mutable app state:** `AppState` is the central mutable authority; UI panels receive `&mut AppState` and return output structs describing user actions.

## Layers

**Domain Layer (`workflow-core`):**
- Purpose: Defines the workflow graph, validates DAG constraints, and executes nodes.
- Location: `crates/workflow-core/src/`
- Contains: Data model (`model.rs`), validation (`validation.rs`), runner (`runner.rs`), interactive engine (`interactive.rs`), trait boundary (`ports.rs`).
- Depends on: `serde`, `uuid`, `thiserror`, `futures`.
- Used by: `openai-client`, `agent-workflow-app`.

**Adapter Layer (`openai-client`):**
- Purpose: Implements `AiPort` for OpenAI and OpenAI-compatible APIs.
- Location: `crates/openai-client/src/lib.rs`
- Contains: `OpenAiClient`, `OpenAiClientConfig`, `OpenAiWireApi` enum (Responses vs ChatCompletions).
- Depends on: `workflow-core`, `reqwest`, `async-trait`.
- Used by: `agent-workflow-app`.

**Application Layer (`agent-workflow-app`):**
- Purpose: Desktop GUI, persistence, and orchestration glue.
- Location: `crates/agent-workflow-app/src/`
- Contains: `WorkflowApp` (top-level app), `AppState`, UI panels, execution bridge.
- Depends on: `workflow-core`, `openai-client`, `eframe`, `egui`, `tokio`, `dirs`.
- Used by: `main.rs` binary.

## Data Flow

### Primary Interactive Execution Path

1. **User triggers run** — keyboard shortcut or button click in `ui/mod.rs` (`WorkflowApp::start_interactive_execution`, line 132).
2. **Validation** — `AppState::validate()` calls `workflow_core::validate_workflow()`.
3. **Provider resolution** — `provider_config::resolve_provider_config()` merges UI settings and environment variables.
4. **Client construction** — `OpenAiClient::with_config()` creates the adapter.
5. **Spawn background task** — `execution::spawn_interactive_workflow_run()` spawns a Tokio task that drives `InteractiveEngine`.
6. **Engine polling** — `InteractiveEngine::poll()` yields `EnginePollResult::CallAi` or `AwaitInput`.
7. **AI invocation** — background task calls `ai.invoke(request).await`.
8. **Event channel** — `ExecutionEvent` values flow back to the main thread via `tokio::sync::mpsc::unbounded_channel`.
9. **UI update** — `WorkflowApp::update()` drains the channel and mutates `AppState` (status, chat logs, run trace).

### Batch / Headless Execution Path

1. **Caller provides workflow and optional manual inputs**.
2. **Same `InteractiveEngine` is used** but driven synchronously by `execution::run_workflow_headless()`.
3. **Events are consumed into a `WorkflowRunSnapshot`** containing report, trace, chat logs, and outputs.
4. **Used by:** acceptance tests (`tests/workflow_acceptance.rs`) and live smoke tests (`tests/live_workflow.rs`).

### Persistence Path

1. **Save** — `WorkflowApp::save_all()` calls `FileWorkflowStore::save()` to write `workflows.json`.
2. **Load** — `WorkflowApp::new()` loads from `FileWorkflowStore::default_path()` on startup.
3. **Settings** — `FileSettingsStore` reads/writes `settings.json` with legacy migration support.

## Key Abstractions

**`AiPort` (trait):**
- Purpose: Decouple workflow execution from any specific AI backend.
- Location: `crates/workflow-core/src/ports.rs` (line 31).
- Pattern: `async_trait` with `Send + Sync` bounds.
- Implementors: `OpenAiClient` in `openai-client/src/lib.rs`.

**`InteractiveEngine`:**
- Purpose: Step-through execution with human-input checkpoints.
- Location: `crates/workflow-core/src/interactive.rs` (line 24).
- Pattern: State machine with explicit `poll()` / `on_ai_complete()` / `on_human_input()` transitions.

**`WorkflowRunner`:**
- Purpose: Fire-and-forget batch execution.
- Location: `crates/workflow-core/src/runner.rs` (line 18).
- Pattern: Generic over `AiPort`, uses `futures::future::join_all` for parallel layer execution.

**`AgentRequest` / `AgentResponse`:**
- Purpose: DTOs crossing the AI boundary.
- Location: `crates/workflow-core/src/ports.rs` (lines 7 and 18).
- Pattern: Plain structs with owned fields; schema enforcement is the adapter's concern.

## Entry Points

**Desktop binary:**
- Location: `crates/agent-workflow-app/src/main.rs`
- Triggers: OS launches the executable.
- Responsibilities: Builds viewport, loads custom fonts (Nunito + Phosphor), runs `eframe::run_native` with `WorkflowApp`.

**Library root:**
- Location: `crates/agent-workflow-app/src/lib.rs`
- Re-exports: `WorkflowApp` from `ui` module.

**Core library root:**
- Location: `crates/workflow-core/src/lib.rs`
- Re-exports: All public types from submodules.

## Architectural Constraints

- **Threading:** Desktop UI is single-threaded (immediate-mode egui). Background AI calls run on a Tokio multi-threaded runtime spawned at app startup (`tokio::runtime::Runtime::new()` in `WorkflowApp::new`).
- **Global state:** The Tokio runtime and channel endpoints (`event_rx`, `action_tx`) are owned by `WorkflowApp`. No static singletons.
- **Circular imports:** None detected.
- **Validation must precede execution:** `WorkflowRunner::run()` and `InteractiveEngine::new()` both call `execution_layers()` first; invalid workflows are rejected before any network call.
- **DAG layer ordering:** Nodes within the same topological layer execute in parallel (via `join_all`). Downstream nodes receive upstream outputs sorted by `NodeId`.

## Anti-Patterns

### State Mutation Scattered Across UI Modules

**What happens:** Multiple UI functions (`ui/mod.rs`, `ui/canvas.rs`, `ui/inspector.rs`) directly mutate `AppState` fields. This can make it hard to trace where a mutation originated.
**Why it's wrong:** Risk of inconsistent state when multiple panels mutate the same field in one frame.
**Do this instead:** Prefer funneling mutations through `AppState` methods (e.g., `state.add_agent_node()`, `state.connect_link_to()`) as already done for core operations. Extend the same pattern for UI-specific mutations.

### Large Monolithic UI Files

**What happens:** `ui/mod.rs` (~614 lines) and `ui/canvas.rs` (~1407 lines) contain app logic, event handling, and rendering code together.
**Why it's wrong:** Harder to navigate and refactor; canvas rendering and chat composer logic are mixed.
**Do this instead:** Keep existing output-struct pattern (`InspectorOutput`, `BottomPanelOutput`, `NavOutput`) and push more behavior into smaller focused helpers or submodules.

## Error Handling

**Strategy:** `thiserror`-derived enums with explicit variants at each layer.

**Patterns:**
- `WorkflowValidationError` (`validation.rs`): Empty workflow, duplicates, missing endpoints, cycles, self-edges.
- `RunError` (`runner.rs`): Wraps `WorkflowValidationError` or reports per-node failures.
- `AgentError` (`ports.rs`): String-wrapped failure from adapters.
- `WorkflowExecutionError` (`execution.rs`): Headless execution failures, missing manual input.
- `ProviderConfigError` (`provider_config.rs`): Missing API key with provider/env context.

## Cross-Cutting Concerns

**Logging:** No structured logging framework detected. Status and errors are surfaced through UI toasts (`last_error`) and chat logs.
**Validation:** Centralized in `workflow-core/src/validation.rs`; used by both interactive and batch paths.
**Authentication:** API keys are resolved via `ProviderEnv` (env vars) with UI input overriding; keys are never persisted to disk.

---

*Architecture analysis: 2026-05-30*
