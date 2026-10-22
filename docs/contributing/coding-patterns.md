# Coding patterns

## Architecture rules

1. `engine` owns workflow model rules, execution semantics, templates, and ports.
2. `providers` is an adapter crate that implements `AiPort` for BYOK providers.
3. `orchestration` composes runtime, state, persistence, tools, projects, settings, and app services.
4. `desktop` is only a transport adapter: Tauri commands and app bootstrap.
5. `ui` owns rendering and interaction, via typed desktop invokes/events.
6. `engine` must not depend on HTTP clients, filesystem/tool I/O, Tauri, providers, orchestration, desktop, or UI.
7. `orchestration` calls engine APIs; it never duplicates workflow validity or execution rules.
8. Add a port/trait only when a consumer is typed on it; otherwise call the concrete type.
9. Existing boundaries: `engine/src/ports/` (`AiPort`, `ToolPort`), `providers/src/client.rs` (`AiClient`), and `ui/src/api.ts` (typed Tauri invoke/event wrappers).

## Ownership by concern

| Concern | Source of truth |
| --- | --- |
| Workflow types, defaults, `WorkflowSettings` | `crates/engine/src/graph/workflow.rs` |
| Graph validity and execution layer order | `crates/engine/src/graph/validation.rs` |
| Batch / headless run semantics | `crates/orchestration/src/run/execution/headless.rs` |
| Interactive pause/resume semantics | `crates/engine/src/execution/interactive_engine/` |
| Node invocation and prompt assembly | `crates/engine/src/execution/node_invocation.rs` |
| Node templates, defaults, and locked fields | `crates/engine/src/template/` |
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
| Frontend DTO types | `crates/ui/src/lib/types/index.ts` |
| Editor layout and panels | `crates/ui/src/screens/EditorScreen.tsx`, `crates/ui/src/panels/` |
| Sidebar and navigation | `crates/ui/src/components/sidebar/` |
| Conversation and tool bubbles | `crates/ui/src/components/conversation/` |

## Runtime semantics

These live in `crates/orchestration/src/run/**` and `crates/engine/src/execution/**`, never in UI or desktop.

1. **Shared context**: `WorkflowSettings.shared_context` is trimmed and appended to every node's system prompt for the run, and to ad-hoc/saved subagent prompts.
2. **Execution cwd**: the backend resolves it from the exact selected project's `default_execution_cwd` (else project root). App workflow/chat runs get an isolated app-managed folder under `{data_local}/openflow/workspaces/`. Filesystem tools run there.
3. **Callable agents**: `AgentNodeConfig.callable_agents` (or `allow_all_callable_agents`) is snapshotted at run start by `engine::resolve_callable_agent_snapshots` into `CallableAgent` records, the same type persisted agents use (`orchestration::AgentDefinition` alias). Invoked via runtime builtins `openflow_call_subagent` and `openflow_declare_subagents`.
4. **Provider routing**: `AgentNodeConfig.provider_id` beats `WorkflowSettings.provider_id`, which beats the active settings provider. The run-scoped router sends each `AgentRequest` to that provider; errors surface with no cross-provider failover.
5. **Model default**: at run prep, an empty `AgentNodeConfig.model` inherits the effective `ProviderProfile.default_model`. An explicit node model always wins.
6. **Workflow storage split**: app-local workflows live in `workflows.json`; project-linked workflows persist under `{project}/.flow/workflows/`. `AppBackend` merges both on load.
7. **Tool concurrency**: shared tools may run in parallel within a batch; exclusive tools take a per-tool semaphore in `ToolPortImpl`.

## Persistence conventions

1. All app persistence files live under `{data_local}/openflow/`: `agents.json`, `projects.json`, `workflows.json`, `settings.json`, `mcp-secrets.json`.
2. Project workflow files use the `{workflowId}.workflow.json` suffix under `.flow/workflows/`.
3. Provider API keys persist in plaintext in `settings.json`, on each `ProviderProfile.api_key`. The UI loads settings redacted and fetches keys over dedicated IPC. Env vars (`OPENAI_API_KEY`, `ANTHROPIC_API_KEY`, provider-specific ones) stay fallbacks.
4. MCP inputs and OAuth credentials persist in `mcp-secrets.json` (plaintext, mode `0600` on Unix). MCP config stores only opaque `mcp-secret:v1:*` refs.

## Implementation conventions

1. Constants go at the top of the file, named by intent (`*_WIDTH`, `*_HEIGHT`, `*_GAP`, `*_PADDING`).
2. Helpers sit near their use, private unless reused.
3. Tests go in `#[cfg(test)] mod tests` in the behavior's file.
4. Typed `thiserror` errors with actionable messages.
5. Where order affects behavior, sort IDs for determinism (as in `validation.rs` and `runner.rs`).
6. Mutate state through dedicated `WorkflowRunState` / `AppBackend` methods, not direct map/vector edits across modules.
7. UI sidebar lists use the shared primitives (`SidebarNavButton`, `SidebarIconButton`, `SidebarList`, `SidebarListRow`) for consistent hover/rename.
8. With no node selected, hide the inspector. The right column shows `WorkflowSettingsPanel` or `InspectorPanel` by editor state.

## Error handling rules

1. Map external/system errors into local domain language at crate boundaries.
2. Fallible operations return `Result<_, _>`. No panics outside tests.
3. `expect(...)` only for invariants a validated flow guarantees.

## Test strategy

1. New behavior gets focused unit tests in the same module.
2. Test visible behavior, not private implementation details.
3. For workflow logic changes, cover:
   - validation outcomes
   - execution layer ordering
   - upstream input shape
   - shared context injection
   - callable agent snapshot resolution
   - execution cwd resolution
   - failure propagation

[Testing workflows](testing-workflows.md) defines acceptance and live-AI checks. The [architecture contract](../architecture/contract.md) defines layer rules.

## Dependency boundary

1. Add workspace deps to the root `Cargo.toml` first, then to crate manifests.
2. Keep crate dependencies minimal and role-specific:
   - `engine`: graph, execution, templates, conversations, tools, and ports only.
   - `providers`: HTTP + provider payload parsing/auth only.
   - `orchestration`: runtime/state/persistence/execution.
   - `desktop`: Tauri adapter only.
   - `ui`: frontend rendering and interaction only.

## Local run commands

| Task | Command |
| --- | --- |
| Start the desktop app | `./scripts/start.sh` |
| Start the frontend only | `npm --prefix crates/ui run dev` |
| Typecheck the frontend | `npm --prefix crates/ui run typecheck` |

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

One pattern per file: sibling file or inline block, never both.

## Change checklist

1. Edit the smallest owning module first.
2. Add or update tests in the same PR.
3. Run:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets
cargo clippy-max
cargo nextest run --workspace
```

4. If behavior contracts changed, update this doc, [`AGENTS.md`](../../AGENTS.md), and [`glossary.md`](../glossary.md) as needed.
