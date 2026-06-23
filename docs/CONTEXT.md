# Context

Domain terms for the Step-through-agentic-workflow architecture.

| Term | Definition |
|---|---|
| **Composition root** | The crate responsible for constructing and wiring all dependencies. Here, orchestration is the composition root — `AppBackend` delegates to `WorkflowCatalog`, `AgentLibrary`, `ProjectRegistry`, `SettingsFacade`, and `RunCoordinator`. Provider construction uses the factory pattern (`create_provider`). |
| **WorkflowCatalog** | Orchestration module: workflow CRUD, app/project merge (project wins on ID collision), assign/unassign. Adapters: `app_workflow_store`, `project_workflow_store`. |
| **RunCoordinator** | Orchestration module: active run session, action channel, `start_run` / `submit_*` / event projection entry points. Calls `finish_run_session` when a run becomes inactive to clear session-scoped resources. |
| **JsonFileStore** | Adapter-internal module (`adapters/storage/json_file_store.rs`): atomic JSON file load/save. Port traits unchanged. |
| **SubagentSession** | `run/execution/subagent_session.rs`: subagent AI-invocation loop extracted from `ToolPortImpl` for locality. |
| **CallableAgent** | Engine type (`engine::CallableAgent`): saved agent snapshotted at run start for subagent invocation. Persisted as `openflow/agents.json`; orchestration alias `AgentDefinition`. |
| **RunTelemetry** | Domain enum for interactive run events (chat, tools, subagents). Orchestration type alias `ExecutionEvent`; projected into `WorkflowRunState` by `events.rs`. |
| **Factory pattern** | The `providers` crate exposes a single public factory function (`create_provider`) that returns `Box<dyn AiPort>`. Orchestration never names a concrete provider type. This is the contract boundary between orchestration and providers. |
| **Seam** | A typed boundary between layers. Examples: `engine::AiPort`, `UiDesktopOutboundPort` in `ui/src/port.ts`. Add a seam only when a consumer depends on the interface, not the concrete type. |
| **Dependency graph** | `engine → (none)`, `providers → engine`, `orchestration → engine + providers`, `desktop → orchestration`, `ui → desktop`. |
| **Allowed import scope** | Target-state submodule limits on cross-crate imports (e.g. `orchestration → providers`: factory + config types only). Not enforced in baseline CI; deferred to Phase B after code matches. |
| **Architecture check rollout** | **Phase A (Tier 2):** inter-crate Cargo graph + forbidden `use`. **Phase B (Tier 3):** providers allowlist, engine invocation locality, domain `adapters::` ban, UI Tauri seam. **Phase C (Tier 3 continued):** domain folders must not import flat `*_store` modules — use port traits; `backend/` wires adapters. **Deferred:** `tool/` → `lsp` ban; `pub(crate)` on all concrete adapters. |
| **Port trait** | Domain module depends on a trait (e.g. `WorkflowStore`, `SkillCatalog`); `backend/` constructs `File*Store` impls. Replaces direct adapter imports in catalog code. |
| **Architecture rules file** | Machine-readable CI source of truth for Phase A checks (e.g. `docs/architecture/arch-check-rules.toml`): workspace `Cargo.toml` allowlists, forbidden cross-crate `use` tables, engine forbidden external deps. `scripts/check-architecture.sh` reads it; `contract.md` links to it. Phase A scope: Rust workspace crates only (`engine`, `providers`, `orchestration`, `desktop`); UI/TypeScript rules deferred to Phase B. |
| **Engine forbidden deps** | External crates the engine hexagon must not depend on (Phase A denylist in architecture rules file): transport/GUI deps such as `reqwest`, `tauri`, `tauri-build`. Distinct from workspace-member edges. |
| **Legacy crate alias** | Pre-rename package paths (`domain`, `workflow_core`) that must not appear in Rust `use` statements. Phase A CI bans them workspace-wide to catch rename regressions. |
| **Architecture check scope** | Phase A forbidden-import scans cover all Rust in each workspace crate: `src/`, crate-root `tests/`, and `#[cfg(test)]` modules — same ban tables as production code. |
| **Engine invocation rule** | Only `orchestration/run/execution/` may construct `InteractiveEngine` or `WorkflowRunner`. Enforced in CI (Tier 3). |
| **UI Tauri seam** | `@tauri-apps/*` imports confined to `api.ts` and `port.ts`; components use `getAppWindow` / `openNativeDialog` wrappers instead of direct Tauri imports. |
| **Orchestration providers seam** | `orchestration/src` imports only allowlisted symbols from `providers::`; `AiClient` banned — runtime uses `create_provider` → `Box<dyn AiPort>`. |
| **Violation class** | Taxonomy of architecture violations. Blocking: banned Cargo dep, banned import. Advisory: empty seam, missing re-export. |
| **Re-export boundary** | Engine types that cross layers (e.g., `Workflow`, `Node`) are re-exported through orchestration via `pub use`. Desktop imports `orchestration::Workflow`, never `engine::Workflow`. This satisfies the "desktop must not depend on engine" rule without a DTO mapping layer. |
| **ApprovalMode** | Node-level tool prompt strategy. Four values: `read_only` (read-class tools only, auto-approved), `write` (all tools; read-class auto, write-class prompt — default), `always_ask` (all tools; prompt every call), `yolo` (auto-approve all; critical bash still prompts). UI is a single dropdown — no per-tool toggles. |
| **Tool capability class** | Static read/write grouping for every builtin tool. **Read:** `read`, `search`, `find`, `ast_grep`. **Write:** `write`, `edit`, `apply_patch`, `bash`, `openflow_declare_subagents`, `openflow_call_subagent`. Not user-configurable per node. In non-`read_only` modes, all tools (both classes) are offered to the model. |
| **NodeToolConfig** | Persisted per node/agent as `{ approvalMode }` only. Tool catalog and per-tool overrides removed — availability derived at runtime from registry + mode. |
| **CallableAgent tool policy** | Saved agents carry their own `approvalMode` (dropdown on Agents screen). Ad-hoc declared subagents inherit the parent node's mode. |
| **Tool config migration** | No backward-compat migration. Drop `catalog` and `overrides` from schema; default new nodes/agents to `write`. Pre-release — old saved JSON with tool lists is not supported. |
| **ApprovalMode (UI labels)** | Dropdown order: `read_only` → "Read only"; `write` (default) → "Read auto-approve, write prompt"; `always_ask` → "Always ask"; `yolo` → "Auto-approve all". |
| **YOLO semantics** | Literally never prompt — remove critical-bash guard; all tools auto-run including destructive shell commands. |

## Orchestration Crate Structure

The orchestration crate is the composition root and applies a clear **domain/adapter separation** (hexagonal architecture):

### Domain Folders (Orchestration Layer)
Flat structure for domains with application-level logic:

| Folder | Files | Purpose |
|--------|-------|---------|
| `agent/` | `library.rs` | Agent CRUD & metadata |
| `workflow/` | `catalog.rs` | Workflow CRUD & merge logic |
| `project/` | `registry.rs` | Project discovery & loading |
| `run/` | `coordinator.rs` + `execution/` + `state/` | Run coordination, execution, state projection |
| `settings/` | `facade.rs` | Settings aggregation |
| `tool/` | `mod.rs` + `registry.rs` + `runner.rs` + `dispatch.rs` + `blocking_ops.rs` + `output.rs` | Tool catalog, execution, blocking I/O dispatch, artifacts |

**No folder for:** `skill`, `template` — these have no domain logic, only persistence (see `adapters/storage/`).

**Structure principle:** No nested layers (no `domain/application/`). Files go directly in domain folders. Adapters are separate in `adapters/`.

### Adapter Folders (External System Integration)

#### `adapters/storage/`
Persistence implementations (CRUD adapters for each domain):
- `json_file_store.rs` - Shared atomic JSON persistence (`atomic_write`, `read_json_file`, `write_json_file`) under `{data_local}/openflow/`
- `agent_store.rs` - Agent file persistence
- `app_workflow_store.rs` - App-level workflow persistence (`workflows.json`)
- `project_workflow_store.rs` - Project workflow files (`.flow/workflows/`)
- `project_store.rs` - Project file persistence
- `settings_store.rs` - Settings file persistence + `provider_config.rs`
- `skill_store.rs` - Skill catalog persistence
- `template_store.rs` - Template persistence

**Rationale:** Consolidate all persistence by *concern* (storage), not by domain. Easier to reason about where data goes.

#### `adapters/tool_impl/`
Tool implementation details (edit tool, file patching, etc):
- `edit/` - Edit tool implementation (batch edits, patching, hashline-based replacement)
- `errors.rs` - Tool execution errors
- Re-exports tool application modules for backward compatibility

**Why separate from infrastructure:** Tools are defined as domain concepts in `engine::tools`. The implementation (edit, patching) is an adapter, but tool execution orchestration (registry, runner) is part of the application layer.

#### `adapters/infrastructure/`
External system integration (don't modify for business logic):
- `lsp/` - LSP protocol binding (writethrough, diagnostics, patching)
- `git/` - Git CLI integration

**Rule:** Only use infrastructure for code that talks to external systems. Never put business logic here.

### Module Exposure (lib.rs)

Top-level `pub mod` declarations in `lib.rs` re-export modules for consumers:
- Domain modules: `agent::library`, `workflow::catalog`, etc.
- Stores: `agent_store`, `workflow_store`, etc. (from `adapters/storage/`)
- Tool modules: `tool`, `tool_registry`, `tool_runner`, `tool_output`
- Infrastructure: `lsp`, `git`

This decouples consumers from the physical file structure.
