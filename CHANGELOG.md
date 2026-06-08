# Changelog

## Unreleased

### Added

- **File edit engine (Tier B):** implement OMP-parity `find_match` and `adjust_indentation` in `orchestration::tools::edit`; error formatting handles identical-line closest matches; tests split into `replace_tests.rs`.
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
