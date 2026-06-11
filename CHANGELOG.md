# Changelog

## Unreleased

### Added

- **AI retry policy:** default 3 auto-retry attempts with exponential backoff (base `backoff_ms`, capped at 30s) in `InteractiveEngine::run` and `WorkflowRunner`; cancellation-aware backoff sleep; gear-panel controls for max attempts and backoff; `RetryPolicy::delay_for_attempt`.
- **Reasoning effort controls:** per-provider default in Settings plus per-node override in the inspector (effort level and budget tokens).
- **Review-driven tests:** interrupt during slow bash tool emits `NodeInterrupted`; parallel retry does not re-emit sibling `NodeAwaitingInput`; headless runs return `MissingRetry` on retryable node failure; chat-completions body forwards `reasoning_effort` / budget fields.
- **Per-node interrupt and retry:** interrupt a thinking/running-tool node without stopping the run (`interrupt_node`); retry failed or interrupted nodes with transcript preserved (`retry_node`); canvas stop/retry actions on node status row; `AgentStatus::Interrupted` and retryable `NodeErrored` / `NodeInterrupted` telemetry while the run stays active.
- **UI polish overhaul:** `motion` animation library; motion tokens and `prefers-reduced-motion` support; animated modals (fade + scale) with focus trap and Escape-to-close; inspector panel slide-in; screen crossfade; dock height transition; canvas node pulse on `started` / `running_tool` and animated edges during runs; streaming caret and thinking-bubble shimmer; tool output expand/collapse; shared `Spinner` and bootstrap skeleton; keyboard shortcut cheatsheet (`?` or sidebar); dark mode (system/light/dark) in Settings; header button shortcut tooltips.
- **Provider thinking in node chat:** stream `reasoning_content` / `reasoning` from OpenAI-compatible APIs into collapsible `ThinkingBubble` rows in the selected node's conversation (collapsed preview by default; expand for full reasoning).
- **ROADMAP.md restructure:** single prioritized queue (30 sequenced items across 6 tiers + unsequenced backlog) replacing category tables; detail specs preserved below the queue. New items: macOS Keychain key storage, pre-run workflow validation, token & cost tracking, canvas editing QoL (undo/redo, duplicate node), onboarding & templates, macOS distribution (signing, notarization, auto-update). Status corrections: chat presentation marked In progress; single Tokio runtime marked Done.
- **Testing conventions:** standardised test placement rules — inline `#[cfg(test)] mod tests` by default, sibling `foo_tests.rs`/`tests.rs` extraction past ~150 lines, crate-level `tests/` for integration, Vitest siblings for frontend — documented in `docs/contributing/testing-workflows.md` and enforced via `.cursor/rules/testing-conventions.mdc`.
- **Bash tool:** agent `bash` builtin (oh-my-pi–aligned) — `command`, optional `cwd`/`env`/`timeout`; non-interactive env defaults; merged stdout/stderr; wall-time and exit-code notices; `ToolTier::Exec` with critical-pattern approval override; opt-in via node tool config.
- **ROADMAP.md:** [Context used](docs/ROADMAP.md#context-used) — structured per-turn context breakdown in composer and chat; ledger of shared context, rules, skills, attachments, and upstream artifacts.
- **ROADMAP.md:** [Attachments & file references](docs/ROADMAP.md#attachments--file-references) — attach button, drag-drop, and image paste; expands prior file-references plan.
- **ROADMAP.md:** model thinking settings — workflow default in gear panel plus per-node inspector override (Thinking & chat presentation).
- **ROADMAP.md:** [Global chat](docs/ROADMAP.md#global-chat) — unified chat pane across node progression; execution-layer message ordering; separate reply bubbles for parallel awaiting nodes.
- **ROADMAP.md:** [Canvas run feedback](docs/ROADMAP.md#canvas-run-feedback) — scrollable in-node subagent list; colored status icons per agent state (thinking, done, etc.).
- **Node status labels:** canvas nodes show descriptive statuses — Thinking, Waiting for Input, Awaiting Approval, Running Tool, and more — with matching colors for each state.
- **Chat markdown:** assistant, user, system, and thinking messages render as Markdown (`solid-markdown`) with styled headings, lists, code blocks, tables, and links.

### Changed

- **Inspector panel:** collapsible sections (Agent, Output schema, Tools, Callable agents) via `InspectorSection`; schema and Apply button grouped; header actions stay visible; Agent open by default.
- **Dark mode tool/agent cards:** inspector tool and callable-agent option bubbles use theme surfaces instead of hardcoded light gray.
- **Settings screen:** full-page shell replaces sidebar and top bar; left nav (Appearance, Authentication, Provider, Reasoning, Models) with section content on the right; Back to editor in settings nav; toast offset adjusts when top bar is hidden.
- **Tool interrupt:** per-node cancel token stops in-flight tool execution and marks the node interrupted without aborting the run.
- **Headless runs:** `NodeErrored` / `NodeInterrupted` return `WorkflowExecutionError::MissingRetry` instead of hanging.
- **Drive retry loop:** retrying one node no longer breaks out of the interaction wait while siblings still await input or approval.
- **OpenAI chat-completions:** forward `reasoning_effort` and `reasoning.max_tokens` from `AgentRequest` when set.
- **Dark mode form fields:** inspector, settings, and agent editor text inputs and textareas use theme-aware `--input-bg` instead of a hardcoded white background.
- **Dark mode dock and canvas:** bottom dock tab bar, chat composer, overview/trace panels, and React Flow controls/nodes use theme variables; canvas board and xyflow `colorMode` no longer fight custom light-only overrides.
- **Dark mode secondary chrome:** top bar, dock tab strip, and chat composer use a warmer navy palette aligned with the sidebar; composer is lifted above the dock body instead of near-black.
- **Dark mode sidebar selection:** workflow and project folder hover/active states use subtle translucent overlays instead of bright light-gray fills.
- **Sidebar list rows:** long workflow and agent titles truncate with ellipsis before the rename (pencil) action; full title on hover via `title` attribute.
- **Backend test split:** extract the 365-line inline test module from `backend/mod.rs` into sibling `backend/tests.rs` per the new testing conventions; fix `clamp_bash_timeout` to use `u64::clamp` (clippy `manual_clamp`).
- **Tool bubble row:** collapsed row shows tool name plus invocation target (file path, search pattern, command) from arguments — not tool output; status label only when target is not yet known.
- **Tool-call XML in chat:** strip `<tool_call>` / ` ```tool_call ` markup from streamed assistant messages on every delta (UI + backend); hide partial `<tool_call` prefixes while streaming; drop empty bubbles that were only tool markup.
- **File changes panel:** collapsible header (file count + chevron) so long change lists do not cover the chat composer; expanded list scrolls inside a capped height.
- **Architecture cleanup:** rename workflow storage adapters to `app_workflow_store.rs` / `project_workflow_store.rs`; remove orchestration `#[path]` flat module aliases — import paths match folder layout (`run::execution`, `workflow::catalog`, etc.); delete stale `docs/file-structure.md`; fix provider layout docs (flat `providers/src/`).
- **Architecture enforcement:** `crates/engine/clippy.toml` I/O bans; `crates/workspace-checks` runs `check-architecture.sh` via `cargo test --workspace`; engine public API snapshot + `scripts/check-engine-public-api.sh` in `verify.sh`.

### Fixed

- **Settings dark mode:** main shell, settings panel, and Save button no longer use light/white backgrounds when dark theme is active.
- **AI loading hang:** SSE streams time out after 90s without data (`AgentError::Transient`); terminal `RunError::NodeFailed` from the drive loop emits `NodeFailed` (not `Error`) so nodes no longer stick on Started; dropped execution events are logged; action-channel close aborts the run cooperatively.
- **User chat XML:** stop stripping `<tool_call>` markup from user messages in the conversation pane; tool-call echo cleanup now applies only to assistant and thinking rows.
- **Canvas node deletion:** block React Flow from removing nodes in the canvas UI (`deletable: false`, `onBeforeDelete`, filter `remove` changes); use the inspector Delete button to remove nodes from the workflow.
- **Tauri / WebKit dev startup:** pre-bundle `remark-parse`, `remark-rehype`, and `unified` in Vite `optimizeDeps` so `solid-markdown` chat rendering no longer throws `Importing binding name 'default' cannot be resolved by star export entries` in Safari/WebKit (macOS desktop dev).
- **Streamed tool-call markup:** strip echoed `<tool_call>` / ` ```tool_call ` blocks when an assistant streaming message finalizes; drop markup-only rows so `openflow_submit_node_output` JSON no longer appears in chat.
- **Awaiting-input chat noise:** stop projecting system "awaiting human input" and upstream `Context:` blocks into the conversation when a node pauses — status pill and run trace still show the pause.
- **Human-input questions:** when the model streams a preamble then calls `openflow_request_user_input`, emit the tool's `assistant_message` to chat if it was not already streamed; reject non-question `assistant_message` values (preamble/narration) and retry the model turn before pausing — clarifying questions no longer disappear after "let me confirm one detail".
- **Tool approval:** file-edit preview uses server-stored tool arguments (avoids UI JSON round-trip mismatch); Approve stays enabled when preview fails (warning shown); `submit_tool_approval` emits `run-state` for consistent UI updates.
- **Multiplex chat:** composer only blocks on approvals for the selected node, not another node's pending approval.
- **Parallel tool batches:** return one result per tool call; unknown tools no longer default to parallel execution.
- **Headless runs:** match manual inputs and approvals by node/approval id in the queue, not only the front entry.
- **Live SSE:** stream chat-completion chunks as they arrive; decode SSE lines from byte buffers for valid UTF-8.
- **Streaming cleanup:** finalize or clear streaming chat bubbles on AI errors; dedupe node lifecycle events per `model_attempt`.
- **Cancel:** parallel model invocations abort promptly when the run is cancelled.

### Added

- **ROADMAP.md:** pretty tool names in chat — human-readable labels for builtins and subagents instead of raw ids.
- **Assistant streaming:** `AiPort::invoke_stream` + `AiStreamSink`; OpenAI Chat Completions SSE transport; `RunTelemetry::ChatMessageDelta` + `ChatMessage.id`/`streaming` for incremental token updates in chat.
- **Parallel shared tools:** `ToolPortImpl` runs contiguous `ToolConcurrency::Shared` batches concurrently; `Exclusive` tools use per-name semaphores.
- **Parallel DAG layers:** `InteractiveEngine` runs all ready nodes in a layer concurrently (`join_all`); multiplex pauses via `EngineRunResult::NeedsInteraction` (multiple awaiting inputs + approval batches).
- **Multiplex run UI:** `awaitingNodeIds` + stacked `pendingApprovals` in run state; chat composer targets any awaiting node; approval queue switches focus by node.
- **Single Tokio runtime (desktop):** `AppBackend` takes an injected `Handle`; Tauri `setup` passes `async_runtime` handle; tests keep an owned runtime via `with_default_paths`.
- **Cursor rules:** `.cursor/rules/hexagonal-core.mdc` (always-on) + scoped `hexagonal-engine.mdc` / `hexagonal-orchestration.mdc` — crate layers, folder layout, port seams, glossary naming, and `check-architecture.sh` verification.
- **Roadmap:** [Project rules](docs/ROADMAP.md#project-rules) — per-linked-project agent guidance (`.flow/rules/`), discovery, run-time injection into shared context.

### Added

- **Ripgrep-backed search tool:** replace naive WalkDir+regex `search` with `grep-searcher` + `ignore` in `adapters/tool_impl/grep.rs` behind `tool_ports::ContentSearch`; gitignore-aware walks, binary skip, 500-match cap; optional `gitignore` arg (default true).
- **Run performance timing:** `RunTelemetry::PhaseTimed` records AI invoke and tool execution durations; entries appear in Run trace (`ai_invoke: … · 3.2s`) and `[perf]` lines in the desktop log.
- **macOS app bundle:** enable Tauri bundling (`bundle.active`, `app` target); `npm --prefix crates/desktop run build` produces `OpenFlow.app`; README documents install and Gatekeeper steps; gate `open_devtools` to debug builds so release bundle compiles.

### Docs

- **Roadmap:** [Upstream read-file context](docs/ROADMAP.md#upstream-read-file-context) — propagate read-tier tool paths (and optional excerpts) to downstream nodes via `read_files` in node input; per-node ledger, transitive merge, workflow opt-in.
- **Roadmap:** near-term [Chat presentation — thinking bubbles & tool cleanup](docs/ROADMAP.md#chat-presentation--thinking-bubbles--tool-cleanup) — collapsible thinking bubbles, compact tool rows, args one-liner; expand Thinking & chat presentation gap table and execution order.
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
- **Engine `InteractiveEngine::run()`:** self-driving async loop; returns `EngineRunResult` (`NeedsInteraction`, `Completed`, `Failed`, `Cancelled`) for orchestration to handle.
- **Docs:** add [`docs/file-structure.md`](docs/file-structure.md) — full repository directory tree (source and docs; excludes build artifacts).

### Changed

- **Runtime I/O:** `resolve_execution_cwd` and large artifact spills run on `spawn_blocking`; `AiInvocationAdapter` centralizes telemetry, timing, and streaming for main + subagent invokes.
- **Read tool:** default reads keep a 300-line cap but now emit an explicit truncation notice with total line count and selector hints (`:start-end`, `:raw`); tool description documents the limit.
- **Rename `domain` → `engine`:** crate directory, Cargo package name, and all `use engine::` imports across orchestration, providers, and desktop; flat `engine::` re-exports preserved for downstream crates.
- **Orchestration `drive.rs`:** thin loop around `engine.run()` — handles input/approval waits and events only; tool execution moved to `tool_port.rs`.
- **Architecture docs:** [`docs/architecture/contract.md`](docs/architecture/contract.md) and [`docs/architecture/README.md`](docs/architecture/README.md) updated for Engine layer, `ToolPort`, and self-driving run loop.

- **UI port:** move `UiDesktopOutboundPort` from `crates/ui/src/lib/desktopClient.ts` to `crates/ui/src/port.ts` at the UI root alongside `api.ts`.
- **Orchestration layout:** reorganize `crates/orchestration/src` into entity-grouped hexarc folders (`workflow/`, `agent/`, `project/`, `run/`, `settings/`, `template/`, `skill/`, `adapters/infrastructure/`); `lib.rs` `#[path]` re-exports preserve existing module paths; flatten adapter-only `template/` and `skill/` (no `adapters/` subfolder).
- **Docs:** add [`docs/sections/orchestration/layout.md`](docs/sections/orchestration/layout.md) — explains entity folders, hexarc roles, disk vs Rust module paths, and where to add code.

### Fixed

- **Parallel shared tools:** unavailable tools in a parallel batch no longer short-circuit the batch; denied results are recorded per call and remaining shared tools still run.
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
