# Coding patterns

Patterns we follow in this repo.

## Architecture rules

1. `engine` owns workflow model rules, execution semantics, templates, and ports.
2. `providers` is an adapter crate that implements `AiPort` for BYOK providers.
3. `orchestration` composes runtime, state, persistence, tools, projects, settings, and app services.
4. `desktop` is a transport adapter only; keep Tauri commands and app bootstrap there.
5. `ui` owns rendering and interaction; it should talk through typed desktop invokes/events.
6. `engine` must not depend on HTTP clients, filesystem/tool I/O, Tauri, providers, orchestration, desktop, or UI.
7. `orchestration` must call engine APIs; do not duplicate workflow validity or execution rules in `orchestration`.
8. Add a port/trait only when a consumer is typed on that interface; otherwise call the concrete type directly.
9. Real seams today: `engine/src/ports/` (`AiPort`, `ToolPort`), `providers/src/client.rs` (`AiClient`), and `ui/src/api.ts` (typed Tauri invoke/event wrappers).

## Ownership by concern

| Concern | Source of Truth |
| --- | --- |
| Workflow types, defaults, `WorkflowSettings` | `crates/engine/src/graph/workflow.rs` |
| Graph validity and execution layer order | `crates/engine/src/graph/validation.rs` |
| Batch / headless run semantics | `crates/orchestration/src/run/execution/headless.rs` |
| Interactive pause/resume semantics | `crates/engine/src/execution/interactive_engine/` |
| Node invocation and prompt assembly | `crates/engine/src/execution/node_invocation.rs` |
| Node templates, defaults, locked fields, and `TemplateStore` trait | `crates/engine/src/template/` |
| LLM invocation contract (`AiPort`, `AgentRequest`) | `crates/engine/src/ports/outbound.rs` |
| Tool execution contract (`ToolPort`) | `crates/engine/src/ports/outbound.rs`, `crates/orchestration/src/run/execution/tool_port.rs` |
| Human input and tool approval resume behavior | `crates/engine/src/execution/interactive_engine/` |
| LLM transport mapping and tool-arg repair | `crates/providers/src/mapping/`, `rig_adapter/` |
| Provider client (`AiClient`, `create_provider`) | `crates/providers/src/client.rs`, `lib.rs` |
| UI desktop seam | `crates/ui/src/api.ts` |
| App backend composition and IPC surface | `crates/orchestration/src/backend/mod.rs` |
| Run execution, shared context, callable agents, execution cwd | `crates/orchestration/src/run/execution/` |
| Mutable run/edit state transitions | `crates/orchestration/src/run/state.rs` |
| App workflow file store | `crates/orchestration/src/adapters/storage/app_workflow_store.rs` |
| Project metadata and workflow bindings | `crates/orchestration/src/adapters/storage/project_store.rs` |
| Project workflow files (`.flow/workflows/`) | `crates/orchestration/src/adapters/storage/project_workflow_store.rs` |
| Saved agent definitions | `crates/orchestration/src/adapters/storage/agent_store.rs` |
| Skill discovery (read-only) | `crates/orchestration/src/adapters/storage/skill_store.rs` |
| Provider readiness and API-key resolution | `crates/orchestration/src/settings/provider.rs` |
| Settings persistence | `crates/orchestration/src/adapters/storage/settings_store.rs` |
| Tool registry and approval | `crates/orchestration/src/tool/` |
| Tauri command/event surface | `crates/desktop/src/lib.rs` |
| Frontend invoke wrappers and UI state wiring | `crates/ui/src/api.ts`, `crates/ui/src/context/` |
| Frontend DTO types | `crates/ui/src/lib/types.ts` |
| Editor layout and panels | `crates/ui/src/screens/EditorScreen.tsx`, `crates/ui/src/panels/` |
| Sidebar and navigation | `crates/ui/src/components/sidebar/` |
| Conversation and tool bubbles | `crates/ui/src/components/conversation/` |

## Runtime semantics

Keep these execution rules in `crates/orchestration/src/run/**` and `crates/engine/src/execution/**`; do not reimplement them in UI or desktop:

1. **Shared context** - `WorkflowSettings.shared_context` is trimmed and appended to every node's system prompt (and ad-hoc/saved subagent prompts) for the run.
2. **Execution cwd** - resolved at run start from the bound project's `default_execution_cwd`, else the process cwd. Filesystem tools run against this path.
3. **Callable agents** - `AgentNodeConfig.callable_agents` (or `allow_all_callable_agents`) is snapshotted at run start via `engine::resolve_callable_agent_snapshots` into `CallableAgent` records; persisted agents use the same type (`orchestration::AgentDefinition` alias). Invocable through runtime builtins `openflow_call_subagent` and `openflow_declare_subagents`.
4. **Provider override** - when `WorkflowSettings.provider_id` is set, it overrides the active settings provider for that run.
5. **Workflow storage split** - app-local workflows live in `workflows.json`; project-linked workflows persist under `{project}/.flow/workflows/`. `AppBackend` merges both on load.
6. **Tool concurrency** - shared tools may run in parallel within a batch; exclusive tools acquire a per-tool semaphore in `ToolPortImpl`.

## Persistence conventions

1. All app persistence files live under `{data_local}/openflow/` (`agents.json`, `projects.json`, `workflows.json`, `settings.json`).
2. Project workflow files use the `{workflowId}.workflow.json` suffix under `.flow/workflows/`.
3. Provider API keys persist in `settings.json` on each `ProviderProfile.api_key` (plaintext on disk). UI loads settings redacted and fetches keys via dedicated IPC. Env vars (`OPENAI_API_KEY`, `ANTHROPIC_API_KEY`, and provider-specific fallbacks) remain fallbacks.

## Implementation conventions

1. Keep constants at top of file and name by intent (`*_WIDTH`, `*_HEIGHT`, `*_GAP`, `*_PADDING`).
2. Keep helper functions near usage and private unless reused.
3. Keep tests in `#[cfg(test)] mod tests` in the same file as behavior.
4. Use typed errors with `thiserror`; include actionable error strings.
5. Preserve deterministic order where it affects behavior by sorting IDs (existing pattern in `validation.rs` and `runner.rs`).
6. For mutating state, prefer dedicated methods on `WorkflowRunState` / `AppBackend` rather than direct map/vector edits across modules.
7. UI sidebar lists use shared primitives (`SidebarNavButton`, `SidebarIconButton`, `SidebarList`, `SidebarListRow`) for consistent hover/rename behavior.
8. Hide the inspector when no node is selected; show `WorkflowSettingsPanel` or `InspectorPanel` in the right column based on editor state.

## Error handling rules

1. Map external/system errors into local domain language at crate boundaries.
2. Return `Result<_, _>` from operations that can fail; avoid panics outside tests.
3. Use `expect(...)` only for invariants that are guaranteed by validated flow.

## Test strategy

1. Add focused unit tests in the same module for new behavior.
2. Test externally visible behavior, not private implementation detail.
3. For workflow logic changes, cover:
   - validation outcomes,
   - execution layer ordering,
   - upstream input shape,
   - shared context injection,
   - callable agent snapshot resolution,
   - execution cwd resolution,
   - failure propagation.

See [`testing-workflows.md`](testing-workflows.md) for acceptance and live-AI verification details. Layer rules live in [`../architecture/contract.md`](../architecture/contract.md).

## Dependency boundary

1. Add workspace deps in root `Cargo.toml` first, then consume in crate manifests.
2. Keep crate dependencies minimal and role-specific:
   - `engine`: graph, execution, templates, conversations, tools, and ports only.
   - `providers`: HTTP + provider payload parsing/auth only.
   - `orchestration`: runtime/state/persistence/execution.
   - `desktop`: Tauri adapter only.
   - `ui`: frontend rendering and interaction only.

## Local run commands

1. Desktop app: `./scripts/start.sh`
2. Frontend only: `npm --prefix crates/ui run dev`
3. Frontend typecheck: `npm --prefix crates/ui run typecheck`

## Test file layout

| Pattern | When | Example |
| --- | --- | --- |
| Sibling `*_tests.rs` | Large ported or multi-case suites | `adapters/tool_impl/edit/patch_tests.rs` declared in parent `mod.rs` |
| Inline `#[cfg(test)] mod tests` | Small unit tests colocated with one source file | `adapters/tool_impl/edit/io.rs` |
| Folder `tests.rs` | Integration tests for a module subtree | `run/execution/tests.rs` |

Declare sibling test modules in the parent `mod.rs`:

```rust
#[cfg(test)]
mod patch_tests;
```

Do not mix patterns within one file - pick sibling file or inline block, not both.

## Change checklist

1. Edit the smallest owning module first.
2. Add or update tests in the same PR.
3. Run:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets
cargo clippy-max
cargo test --workspace
```

4. If behavior contracts changed, update this doc, [`AGENTS.md`](../../AGENTS.md), and [`glossary.md`](../glossary.md) when needed.
