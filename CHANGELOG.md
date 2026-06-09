# Changelog

## Unreleased

### Added

- **Ripgrep-backed search tool:** replace naive WalkDir+regex `search` with `grep-searcher` + `ignore` in `adapters/tool_impl/grep.rs` behind `tool_ports::ContentSearch`; gitignore-aware walks, binary skip, 500-match cap; optional `gitignore` arg (default true).
- **Run performance timing:** `RunTelemetry::PhaseTimed` records AI invoke and tool execution durations; entries appear in Run trace (`ai_invoke: … · 3.2s`) and `[perf]` lines in the desktop log.
- **macOS app bundle:** enable Tauri bundling (`bundle.active`, `app` target); `npm --prefix crates/desktop run build` produces `OpenFlow.app`; README documents install and Gatekeeper steps; gate `open_devtools` to debug builds so release bundle compiles.

### Docs

- **Roadmap:** File references section — `@` file attachments in chat/entrypoint, structured submit payload, path jail, composer pills; mirrors `/skill` invocation pattern.
- **Roadmap:** File edit tooling section — mark builtins, approval, ledger, diff preview, and git revert as Done; document `ToolRef.tier` (`read` explicit, `write` default for `write`/`edit`/`apply_patch`); mark T4 tool-approval policy Done.
- **Roadmap:** Refactor section — track removal of legacy snake_case ↔ camelCase / PascalCase serde aliases after T16 casing unification.

### Added

- **Orchestration domain/adapter separation:** port traits for project workflows (`ProjectWorkflowStore`), skills (`SkillCatalog`), and project bindings (`project/domain.rs`); settings types in `settings/model.rs`, provider resolution in `settings/provider.rs`; `backend/` wires all `File*Store` adapters; fix duplicate module compilation via `lib.rs` aliases for `tools`, storage, and `lsp`.
- **CI architecture checks (Phase C):** domain folders must not import flat store modules (`agent_store`, `flow_store`, …); legacy `domain` crate alias matcher excludes `project::domain` submodules.
- **CI architecture checks (Phase B / Tier 3):** extend [`docs/architecture/arch-check-rules.toml`](docs/architecture/arch-check-rules.toml) — `orchestration → providers` symbol allowlist (`AiClient` banned), engine invocation locality (`InteractiveEngine::new` only in `run/execution/`), orchestration domain folders must not `use crate::adapters::`, UI `@tauri-apps/*` seam (`api.ts` / `port.ts` only).
- **UI Tauri seam:** `getAppWindow` / `openNativeDialog` wrappers in `api.ts`; `AppProvider` no longer imports `@tauri-apps` directly.
- **CI architecture checks (Phase A):** [`docs/architecture/arch-check-rules.toml`](docs/architecture/arch-check-rules.toml) — Tier 2 baseline rules (workspace `Cargo.toml` graph, forbidden cross-crate `use`, engine transport/GUI dep denylist, legacy `domain`/`workflow_core` bans). [`scripts/check-architecture.sh`](scripts/check-architecture.sh) reads the TOML; fixed for `engine` rename and current crate paths.
- **Docs:** one-line crate role summary in [`docs/architecture/contract.md`](docs/architecture/contract.md), [`AGENTS.md`](AGENTS.md), and [`docs/README.md`](docs/README.md) — engine = valid workflow + run behavior; orchestration = store/load/wire/host; providers = LLM transport; ui/desktop = user interaction.
- **Engine `ToolPort`:** outbound port for tool and subagent execution; `ToolPortImpl` in orchestration handles filesystem tools, declare/call subagent builtins, and file-change telemetry.
- **Engine `InteractiveEngine::run()`:** self-driving async loop over `poll()`; returns `EngineRunResult` (`NeedsInput`, `NeedsApproval`, `Completed`, `Failed`, `Cancelled`) for orchestration to handle.
- **Docs:** add [`docs/file-structure.md`](docs/file-structure.md) — full repository directory tree (source and docs; excludes build artifacts).

### Changed

- **Read tool:** default reads keep a 300-line cap but now emit an explicit truncation notice with total line count and selector hints (`:start-end`, `:raw`); tool description documents the limit.
- **Rename `domain` → `engine`:** crate directory, Cargo package name, and all `use engine::` imports across orchestration, providers, and desktop; flat `engine::` re-exports preserved for downstream crates.
- **Orchestration `drive.rs`:** thin loop around `engine.run()` — handles input/approval waits and events only; tool execution moved to `tool_port.rs`.
- **Architecture docs:** [`docs/architecture/contract.md`](docs/architecture/contract.md) and [`docs/architecture/README.md`](docs/architecture/README.md) updated for Engine layer, `ToolPort`, and self-driving run loop.

- **UI port:** move `UiDesktopOutboundPort` from `crates/ui/src/lib/desktopClient.ts` to `crates/ui/src/port.ts` at the UI root alongside `api.ts`.
- **Orchestration layout:** reorganize `crates/orchestration/src` into entity-grouped hexarc folders (`workflow/`, `agent/`, `project/`, `run/`, `settings/`, `template/`, `skill/`, `adapters/infrastructure/`); `lib.rs` `#[path]` re-exports preserve existing module paths; flatten adapter-only `template/` and `skill/` (no `adapters/` subfolder).
- **Docs:** add [`docs/sections/orchestration/layout.md`](docs/sections/orchestration/layout.md) — explains entity folders, hexarc roles, disk vs Rust module paths, and where to add code.

### Fixed

- **Node runtime preamble:** engine assembles `AgentRequest.system_messages` (`NODE_RUNTIME_PREAMBLE`, node prompt, workflow context); providers only call `system_content()` for wire transport. Task context user message is node/task/upstream only.
- **Node completion:** downstream nodes fail fast when upstream output is missing.
- **Remove max tool rounds:** drop `NodeToolConfig.max_tool_rounds` / `maxToolRounds` from engine, UI, and workflows; agents may call tools until they submit node output.

- **Subagent runtime test:** `advance_subagent_invoke_records_tool_calls_before_results` catch-all arm now panics on unexpected variants instead of asserting an impossible `NeedAi` match.
- **File changes panel:** scope "Changed files" and revertible batches to the selected node (`changedFilesByNode` in run state; `FileChangesPanel` filters by `selectedNodeId`).
- **Verify pipeline:** derive `Default` for `NodeToolConfig`; move `events.rs` test module after production items (clippy `items_after_test_module`).
- **Git diff / undo (Phase 9):** batch revert removes only matching `batch_id` records; syncs `InteractiveEngine` and hashline snapshots; deletes created paths before restoring sources; skips non-UTF-8 snapshot capture; keeps execution cwd after run end for post-run git diff.
- **LSP writethrough (Phase 8):** `diagnostics_on_write` no longer appends a false error while the language-server client is unimplemented.
- **LSP writethrough (Phase 8):** `LspSettings::from_persisted` applies env overrides (`PI_LSP_*`) after persisted app settings so operational env toggles are not discarded.
- **LSP writethrough (Phase 8):** format-on-write runs before ledger/snapshot recording; tool diffs and hashline tags reflect post-format disk content; persisted `AppSettings.lsp` wired through `ToolRunner`; formatter stderr surfaced on failure; binary availability cached per process.
- **File edit UI (Phase 6):** `preview_file_edit` IPC passes `approvalId` and backend matches the specific pending approval instead of always using the first queue entry.
- **File edit UI (Phase 6):** `read` snapshot keys now use `canonical_snapshot_path` (same as hashline); per-run snapshot store shared between execution and approval preview; preview IPC requires matching pending approval; Approve disabled until file-edit preview succeeds; rename-aware dedup in `FileChangesPanel` and upstream `changed_files`.
- **File edit UI (Phase 6):** subagent write-tier tools now emit `FileChanged` and update run state; approval card restores raw args for non-edit tools; rename previews show destination; `FileChangesPanel` dedupes by path; create/delete/patch mutations include `diff_summary`; partial `apply_patch` errors include applied hunk output.
- **File edit engine (Phase 5):** per-call file ledger always drains (no cross-tool contamination); partial `apply_patch` failures return applied `file_changes` with `is_error`; `apply_patch` skips no-op update records; transitive upstream `changed_files` propagate by latest timestamp.
- **File edit tools (Tier C):** `write` checks for no-op overwrites before touching disk; `EditIo::preview_text_after_write` shares normalization with `write_text`; headless test cwd override is canonicalized.
- **File edit engine (Tier C):** `resolve_writable` canonicalizes existing path segments so relative paths cannot escape the execution folder via symlinks; `EditIo::write_text` matches patch trailing-newline policy for existing files; notebook paths rejected on `exists`.
- **File edit engine (Tier B):** patch paths are confined to the execution folder; create rejects existing files; create/delete/move verify disk state; move verifies destination before deleting source and rolls back destination on source delete failure.
- **File edit engine (Tier B):** patch post-write verification now fails on unreadable writes, checks expected bytes on disk (including no-op and rename), reports duplicate exact sequence matches, and slices leading whitespace by byte length.

### Added

- **Cursor skill:** `.cursor/skills/rust-hexarc-organizer` — OpenFlow hexarc guide (entity grouping, service/repository roles, inbound/outbound adapters, domain model and error boundaries; integrates [howtocodeit.com hexarc guide](https://www.howtocodeit.com/guides/master-hexagonal-architecture-in-rust)).
- **Hashline patch engine (Phase 7):** mechanical Rust port of `@oh-my-pi/hashline` at `orchestration::tools::edit::hashline` — tokenizer, parser, applier, snapshot store, sync `Patcher`, `XxHash32` (seed 0) 4-hex tags; `edit` tool hashline mode (`{ input }` with `¶path#TAG`); per-run snapshot store on `ToolRunner` (recorded on `read`); hashline dry-run preview; 7 patcher/execute tests.
- **LSP writethrough (Phase 8):** `orchestration::lsp` module — CLI format-on-write (`rustfmt`, `prettier`, `gofmt`, `black`, `stylua`); wired through `EditIo`, `apply_patch`, and hashline writes; diagnostics appended to tool results; settings in `AppSettings.lsp` + env (`PI_LSP_FORMAT_ON_WRITE`, `PI_LSP_ENABLED`, `PI_LSP_DIAGNOSTICS_ON_WRITE`); full language-server client deferred.
- **Git diff / undo (Phase 9):** `orchestration::git` wraps `git diff` / `restore` scoped to execution cwd; pre-edit `EditBatch` snapshots on write/edit/apply_patch; `git_diff_file` and `revert_edit_batch` IPC; `FileChangesPanel` shows git diff per file and one-click batch revert.
- **File edit UI (Phase 6):** `preview_file_edit` IPC dry-run diffs for write/edit/apply_patch approval; `ToolApprovalCard` shows numbered preview; `FileChangesPanel` lists run `changedFiles` with expandable diff summaries.
- **File edit engine (Phase 5):** `FileChangeRecord` ledger — `RunTelemetry::FileChanged`, cumulative `WorkflowRunState.changed_files`, per-node tracking in `InteractiveEngine`, downstream `changed_files` in node input JSON; `ToolRunner` drains ledger after write/edit/apply_patch.
- **File edit engine (Phase 4):** `tools::edit::auto_generated` guard blocks writes/updates/deletes to generated files (`@generated`, `Code generated by sqlc`, `.pb.go`, etc.); wired through `EditIo` and patch update/delete; toggle via `PI_EDIT_BLOCK_AUTO_GENERATED=0`.
- **File edit tools (Tier C):** register `write`, `edit`, and `apply_patch` builtins (`ToolTier::Write`, exclusive concurrency); wire handlers through `ToolRunner` on `spawn_blocking`; expose tools in node tool catalog UI; acceptance tests for write approval, file mutation, and path escape.
- **File edit engine (Tier C):** `tools::edit::path` (`resolve_writable` jail) and `tools::edit::io` (`EditIo` read/write with BOM/LF restore, notebook rejection); patch engine uses shared path jail.
- **File edit engine (Tier B):** `tools::edit::apply_patch` — Codex envelope parser (`parse_apply_patch`, `expand_apply_patch_to_inputs`, heredoc strip); 16 OMP-parity tests.

- **File edit engine (Tier B):** `tools::edit::patch` — OMP-parity patch applicator (`apply_patch_entry`) with fuzzy hunk matching, create/update/delete/rename, BOM/line-ending restore, and post-write byte verification (`PatchVerifyError`); 7 unit tests including path confinement, create guard, round-trip CRUD, and move rollback.
- **File edit engine (Tier B):** `parse_apply_patch_streaming` tests and `replace_sequence` unit tests for exact/EOF/context matching.
- **File edit engine (Tier B):** `tools::edit::diff` — `generate_diff_string`, `parse_diff_hunks`, `normalize_diff`, `replace_text`; BOM/LF/`normalize_unicode` helpers on `normalize`; 55 unit tests.
- **File edit engine (Tier B):** `replace_text` restores original line endings; `all` mode replaces occurrences one-by-one with indentation adjustment; ambiguous fuzzy matches error in single mode.
- **File edit engine (Tier B):** OMP-parity `find_match` and `adjust_indentation` in `orchestration::tools::edit`.
- **Provider API key storage:** persist keys in plaintext on `ProviderProfile.api_key` in `settings.json`; Settings screen documents on-disk risk; env-var fallback unchanged.
- **Run stop/cancel:** `stop_run` IPC command with cooperative cancellation (`CancellationToken`, `ExecutionAction::Stop`, `RunTelemetry::Aborted`); editor top-bar Stop button (Cmd+.) when a run is active; window close aborts active runs; `ast-grep` subprocess kill on cancel.
- **Project workflow menu:** per-project **+** button (shown on hover) with **New workflow** and **Add existing…**; existing workflows open a picker modal to link app or other-project workflows.
- **`docs/glossary.md`:** canonical domain glossary (module map, corrected enum names, WorkflowRunner vs InteractiveEngine); root `UBIQUITOUS_LANGUAGE.md` redirects here.
- **ROADMAP.md:** consolidate `TODO.md`, `todooo.md`, and `FEATURE_LIST.md` into a single prioritized roadmap (near-term engineering, product features, domain hardening phases).
- **ROADMAP.md:** add File changer tool; plan removal of per-node JSON output schema editing (out of scope); add terminal tab in bottom dock panel.
- **ROADMAP.md:** add tool invocation retry and resilient failure handling (T19–T21, D5); clarify T6 covers AI retries only.
- **ROADMAP.md:** add Accessibility section — keyboard shortcuts for sidebar hide/show and dock max/collapse, plus focus and shortcut reference items.
- **ROADMAP.md:** add Agent questions & todos section — structured question UI, todo builtins, and in-run todo panel; replace generic TODO list row.
- **ROADMAP.md:** add queued chat input — buffer user messages during active runs, drain on `AwaitInput`.
- **ROADMAP.md:** add Refactor section — per-crate Done/Planned structural cleanup for domain, providers, orchestration, desktop, and ui.
- **ROADMAP.md:** add File edit tooling section — write/patch builtins, approval, diff preview, changed-files ledger; consolidate prior file-changer rows.
- **ROADMAP.md:** add Thinking & chat presentation section — per-node thinking level, collapsible thinking blocks in chat, collapsible tool bubbles (summary collapsed, expand for full output); replace generic “hide tool output” and “per-node thinking amount” rows.
- **ROADMAP.md:** add near-term Provider API key storage — switch from OS keychain to plaintext in settings to avoid macOS unlock popup on every launch.
- **Workflow settings (v1):** portable `WorkflowSettings.shared_context` on each workflow, injected into every node's system prompt at run time; gear panel in the editor top bar to edit it.
- **Execution folder:** run-time folder derived from the linked project's path (or process cwd for independent workflows); read-only "Run in" chip in the top bar.
- **Projects sidebar:** folder-backed project groups with nested workflows, native folder picker (`Add project`), and per-project **+** dialog to create or link workflows. App workflows stay in the local store; project workflows are saved under `{project}/.flow/workflows/*.workflow.json` (skills-style repo layout).
- **Project store:** `projects.json` persistence for folder-scoped project metadata and workflow bindings; bootstrap and IPC commands for create/assign/unassign.
- **Workflow project assignment:** project dropdown in workflow settings to link or unlink a workflow from a project.
- **Workflow settings (schema):** optional `schedule`, `retry_policy`, and `provider_id` fields on `WorkflowSettings` for phase-2 automation; `provider_id` override is honored at run start when set.
- **Callable agents on nodes:** node inspector picker for saved agents; definitions snapshotted at run start and invocable via `openflow_call_subagent` alongside ad-hoc subagent declaration. Optional **Allow all agents** uses every saved agent at run start.
- **Skill description preview:** show each invoked `/skill` description above the chat composer while typing slash commands.

### Removed

- **OS keychain API key storage:** delete `credential_store` and `keyring` dependency; drop `key_ref` and legacy settings schema migrations for provider keys.
- **Planning docs:** delete `TODO.md`, `todooo.md`, and `FEATURE_LIST.md` (content moved to `ROADMAP.md`).

### Fixed

- **Malformed submit-output recovery:** auto-wrap flat schema fields under `output` when models omit the wrapper; retry the node up to three times with a correction prompt before failing.
- **Node completion chat:** push only the `summary` field from structured node output (not raw JSON) and render it in a dedicated “Node completed” bubble with a success cue.
- **Chat tool-call echo:** strip `<tool_call>` / fenced tool markup from assistant messages while keeping leading human text; apply on node completion and submit-output parsing so raw invocation XML no longer appears in chat.
- **LLM JSON recovery:** apply `jsonrepair-rs` to plain JSON completions and internal submit/input tool arguments, not only external tool-call argument strings.
- **Subagent tool-round transcripts:** record `ToolCall` items before `ToolResult` entries when advancing a subagent session so the next model turn sees paired call/result history.
- **Ad-hoc subagent system prompts:** append workflow `shared_context` when building ad-hoc subagent requests, matching saved-agent subagents.

### Changed

- **User-initiated stop presentation:** `TraceStatus::Stopped` and `AgentStatus::Stopped` replace failed styling for aborted runs — canvas nodes, overview feed, and trace list show **stopped** (neutral gray), not failed.
- **Sidebar layout:** Projects section fills remaining sidebar height; project list scrolls independently below Workflows.
- **Subagent runtime in domain:** add `subagent_runtime` (declare/call builtins, `SubagentInvokeSession` turn machine) and `subagents` helpers; `drive.rs` delegates I/O only (~530 lines, down from ~810).
- **Unified run telemetry:** `domain::RunTelemetry` is the canonical interactive event enum; `orchestration::ExecutionEvent` is a type alias; `merge_shared_context` replaces duplicate orchestration helper.
- **CallableAgent in domain:** add `domain::CallableAgent`, `resolve_callable_agent_snapshots`, and `build_predefined_subagent_summaries`; `agent_store::AgentDefinition` is a type alias for persistence JSON; execution host uses domain snapshots.
- **Thin orchestration backend:** split `AppBackend` into `WorkflowCatalog`, `AgentLibrary`, `ProjectRegistry`, `SettingsFacade`, and `RunCoordinator`; move IPC DTOs to `api.rs` and `BackendError` to `error.rs`. `backend.rs` delegates one line per command (~240 lines of wiring vs ~730 lines of implementation).
- **Domain module tree:** reorganize `crates/domain` into vocabulary-aligned modules (`graph/`, `template/`, `execution/`, `conversation/`, `tools/`, `ports/`); extract shared `node_invocation` for `WorkflowRunner` and `InteractiveEngine`; move `FileTemplateStore` to orchestration; keep flat `domain::` re-exports for downstream crates.
- **Orchestration rust-skills pass:** run filesystem tool I/O on `spawn_blocking`; cache line-selector regex in `LazyLock`; split `execution.rs` into `execution/{mod,drive,events,headless,subagents,tests}.rs`; replace stringly `BackendError::Execution` with typed variants (`AgentNotFound`, `InvalidExecutionCwd`, `ProjectOperation`); avoid cloning all projects on load when cleanup is a no-op; handle missing parent node in subagent calls without panicking.
- **Domain conventions:** replace `anyhow` in `template_store` with `TemplateStoreError`; make template mutations atomic with rollback on persist failure; return `NotFound` for missing updates; extract shared `build_upstream_map` / `workflow_system_prompt` helpers; reduce `InteractiveEngine::poll` cloning; add docs to `model`, `tools`, and interactive poll contracts; convert sync interactive tests from `#[tokio::test]` to `#[test]`.
- **Ports/adapters trim:** remove unused `ports/` and `adapters/` scaffolding from orchestration, desktop, domain, and providers; keep real seams (`domain::AiPort`, human/tool input ports, `UiDesktopOutboundPort`). Flatten UI into `crates/ui/src/lib/desktopClient.ts`; inline `create_provider` and provider wire helpers in `providers`.
- **Orchestration:** remove dead code — legacy `AppState`, unused `tools/approval` module, `find_project_for_workflow`, and unused `AppBackend::upsert_project` / `delete_project` helpers.
- **Orchestration:** remove unused `canvas_math` module and `EditState::move_node_by_delta` (node dragging is handled in the UI).
- **Docs:** move contributor reference docs from `agent-reference-docs/` into `docs/` (`README.md`, `coding-patterns.md`, `testing-workflows.md`) with relative links and a unified index alongside architecture docs.
- **Docs layout:** reorganize `docs/` into `contributing/` and `architecture/` (with `diagrams/`); add section READMEs and a root filesystem index.
- **AGENTS.md:** refresh repo map, persistence paths, docs tree, and common change paths for projects, agents, skills, and current crate layout.
- **Docs sections:** add `docs/sections/` with one folder per crate (`domain`, `providers`, `orchestration`, `desktop`, `ui`) and empty README stubs for author-owned what/why notes.
- **Workflows sidebar header:** move **New workflow** into the section label row as a right-aligned **+** icon (shown on hover) instead of a separate nav row.
- **Projects sidebar header:** move **Add project** into the section label row as a right-aligned **+** icon (shown on hover) instead of a separate nav row.
- **Project–workflow linking UI:** remove workflow settings **Also appear in projects** checkboxes; project assignment returns via the redesigned per-project **+** menu.
- **Project workflow storage:** drop the Global pseudo-project folder; app workflows list independently at the top of the sidebar, and repo workflows persist under `{project}/.flow/workflows/`.
- Hide the inspector panel when no node is selected so the canvas uses the full workspace width.
- Extract reusable sidebar primitives (`SidebarNavButton`, `SidebarIconButton`, `SidebarList`, `SidebarListRow`) from the root sidebar and use them in the Agents screen for consistent nav buttons and list rows.
- Add inline agent rename in the Agents sidebar list, matching workflow rename with the pencil action and commit-on-blur behavior.
- Replace hand-rolled truncated JSON suffix recovery in provider tool-call argument parsing with `jsonrepair-rs`, improving recovery for truncated and malformed LLM tool arguments.
- Render tool invocations in chat as full-width bubbles with a fixed tool-name header and a height-constrained, auto-scrolling output area; one bubble per tool call backed by structured run state, with legacy plain-text tool lines grouped for older sessions.
- Suppress redundant model-emitted tool-call XML from chat when structured tool calls are already parsed and shown in the tool bubble.
