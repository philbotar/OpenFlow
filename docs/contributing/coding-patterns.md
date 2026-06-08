# Coding Patterns

Patterns we follow in this repo.

## Architecture Rules

1. `domain` is pure domain logic.
2. `providers` is an adapter crate that implements `AiPort` for BYOK providers.
3. `orchestration` composes runtime, state, and persistence.
4. `desktop` is a transport adapter only; keep Tauri commands and app bootstrap there.
5. `ui` owns rendering and interaction; it should talk through typed desktop invokes/events.
6. Domain crate must not depend on HTTP clients or async runtimes beyond what's needed for tests.
7. `orchestration` must call domain APIs; do not duplicate domain rules in `orchestration`.
8. Add a port/trait only when a consumer is typed on that interface; otherwise call the concrete type directly.
9. Real seams today: `domain/src/ports/` (`AiPort`, human/tool input), `providers/src/client.rs` (`AiClient`), `ui/src/lib/desktopClient.ts` (`UiDesktopOutboundPort`).

## Ownership By Concern

| Concern | Source of Truth |
| --- | --- |
| Workflow types, defaults, `WorkflowSettings` | `crates/domain/src/model.rs` |
| Graph validity and layer order | `crates/domain/src/validation.rs` |
| Batch run semantics | `crates/domain/src/runner.rs` |
| Interactive pause/resume semantics | `crates/domain/src/interactive.rs` |
| Node templates (defaults, locked fields) | `crates/domain/src/template.rs` |
| Template persistence | `crates/orchestration/src/template_store.rs` |
| LLM invocation contract (`AiPort`, `AgentRequest`) | `crates/domain/src/ports/outbound.rs` |
| Human input / tool approval inbound ports | `crates/domain/src/ports/inbound.rs` |
| LLM transport mapping and tool-arg repair | `crates/providers/src/mapping.rs`, `openai_compat.rs`, `anthropic.rs` |
| Provider client (`AiClient`, `create_provider`) | `crates/providers/src/client.rs`, `lib.rs` |
| UI desktop seam | `crates/ui/src/lib/desktopClient.ts` |
| App backend composition and IPC surface | `crates/orchestration/src/backend.rs` |
| Run execution, shared context, callable agents, execution cwd | `crates/orchestration/src/execution.rs` |
| Mutable run/edit state transitions | `crates/orchestration/src/state.rs` |
| App workflow file store | `crates/orchestration/src/storage.rs` |
| Project metadata and workflow bindings | `crates/orchestration/src/project_store.rs` |
| Project workflow files (`.flow/workflows/`) | `crates/orchestration/src/flow_store.rs` |
| Saved agent definitions | `crates/orchestration/src/agent_store.rs` |
| Skill discovery (read-only) | `crates/orchestration/src/skill_store.rs` |
| Provider readiness and API-key resolution | `crates/orchestration/src/provider_config.rs` |
| Settings persistence | `crates/orchestration/src/settings_store.rs` |
| Tool registry and approval | `crates/orchestration/src/tools/` |
| Tauri command/event surface | `crates/desktop/src/lib.rs` |
| Frontend invoke wrappers and UI state wiring | `crates/ui/src/api.ts`, `crates/ui/src/context/` |
| Frontend DTO types | `crates/ui/src/lib/types.ts` |
| Editor layout and panels | `crates/ui/src/screens/EditorScreen.tsx`, `crates/ui/src/panels/` |
| Sidebar and navigation | `crates/ui/src/components/sidebar/` |
| Conversation and tool bubbles | `crates/ui/src/components/conversation/` |

## Runtime Semantics

Keep these execution rules in `orchestration/src/execution.rs`; do not reimplement in UI or desktop:

1. **Shared context** — `WorkflowSettings.shared_context` is trimmed and appended to every node's system prompt (and ad-hoc/saved subagent prompts) for the run.
2. **Execution cwd** — resolved at run start from the bound project's `default_execution_cwd`, else the process cwd. Filesystem tools run against this path.
3. **Callable agents** — `AgentNodeConfig.callable_agents` (or `allow_all_callable_agents`) is snapshotted at run start via `domain::resolve_callable_agent_snapshots` into `CallableAgent` records; persisted agents use the same type (`orchestration::agent_store::AgentDefinition` alias). Invocable through runtime builtins `openflow_call_subagent` and `openflow_declare_subagents`.
4. **Provider override** — when `WorkflowSettings.provider_id` is set, it overrides the active settings provider for that run.
5. **Workflow storage split** — app-local workflows live in `workflows.json`; project-linked workflows persist under `{project}/.flow/workflows/`. `AppBackend` merges both on load.

## Persistence Conventions

1. Newer stores (`agents.json`, `projects.json`, `templates.json`) use the `openflow` data-dir slug and migrate from `step-through-agentic-workflow` on first read.
2. `workflows.json` and `settings.json` still use the legacy `step-through-agentic-workflow` slug.
3. Project workflow files use the `{workflowId}.workflow.json` suffix under `.flow/workflows/`.
4. Provider API keys persist in `settings.json` on each `ProviderProfile.api_key` (plaintext on disk). UI loads settings redacted and fetches keys via dedicated IPC. Env vars (`OPENAI_API_KEY`, etc.) remain fallback.

## Implementation Conventions

1. Keep constants at top of file and name by intent (`*_WIDTH`, `*_HEIGHT`, `*_GAP`, `*_PADDING`).
2. Keep helper functions near usage and private unless reused.
3. Keep tests in `#[cfg(test)] mod tests` in the same file as behavior.
4. Use typed errors with `thiserror`; include actionable error strings.
5. Preserve deterministic order where it affects behavior by sorting IDs (existing pattern in `validation.rs` and `runner.rs`).
6. For mutating state, prefer dedicated methods on `WorkflowRunState` / `AppBackend` rather than direct map/vector edits across modules.
7. UI sidebar lists use shared primitives (`SidebarNavButton`, `SidebarIconButton`, `SidebarList`, `SidebarListRow`) for consistent hover/rename behavior.
8. Hide the inspector when no node is selected; show `WorkflowSettingsPanel` or `InspectorPanel` in the right column based on editor state.

## Error Handling Rules

1. Map external/system errors into local domain language at crate boundaries.
2. Return `Result<_, _>` from operations that can fail; avoid panics outside tests.
3. Use `expect(...)` only for invariants that are guaranteed by validated flow.

## Test Strategy

1. Add focused unit tests in the same module for new behavior.
2. Test externally visible behavior, not private implementation detail.
3. For workflow logic changes, cover:
   - validation outcomes,
   - layer/execution ordering,
   - upstream input shape,
   - shared context injection,
   - callable agent snapshot resolution,
   - execution cwd resolution,
   - failure propagation.

See [`testing-workflows.md`](testing-workflows.md) for acceptance and live-AI verification details. Layer rules live in [`../architecture/contract.md`](../architecture/contract.md).

## Dependency Boundary

1. Add workspace deps in root `Cargo.toml` first, then consume in crate manifests.
2. Keep crate dependencies minimal and role-specific:
   - `domain`: model/validation/runner/interactive/templates only.
   - `providers`: HTTP + provider payload parsing/auth only.
   - `orchestration`: runtime/state/persistence/execution.
   - `desktop`: Tauri adapter only.
   - `ui`: frontend rendering and interaction only.

## Local Run Commands

1. Desktop app: `npm --prefix crates/desktop run start -- dev`
2. Frontend only: `npm --prefix crates/ui run dev`
3. Frontend typecheck: `npm --prefix crates/ui run typecheck`

## Change Checklist

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
