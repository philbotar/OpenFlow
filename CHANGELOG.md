# Changelog

## Unreleased

### Fixed

- **Headless run queue desync:** replace `.expect` panics when scripted input/approval queues miss a matching entry with `MissingManualInput` / `MissingApproval` errors.
- **Verify gate fixes:** settings nav tests include MCP Servers; backend project tests use isolated temp dirs; `deny.toml` skips duplicate `nix`/`cfg_aliases` from `rmcp` + `portable-pty`.

### Changed

- **God-module split (orchestration):** `patch.rs` → `patch/{mod,hunk,search}.rs`; `coordinator.rs` → `coordinator/{mod,session,checkpoint}.rs` — file splits only, no API change.

### Changed

- **MCP external tools plan:** slim [`2026-06-15-mcp-external-tools.md`](docs/superpowers/plans/2026-06-15-mcp-external-tools.md) — run-scoped `rmcp` clients, no global manager, `ToolTier::Write` for `mcp/` tools.
- **Settings Providers page:** consolidate Authentication, Provider, Reasoning, and Models into one Providers settings page with readiness status, grouped subsections, and a save bar; nav is now Appearance + Providers; fix inline-form button sizing on Add model row; compact model chips with outer spacing.
- **Sidebar zoom hint:** remove hover popup showing zoom percentage on Shortcuts/Settings footer; ⌘/Ctrl +/−/0 shortcuts unchanged.

### Added

- **Miri (engine UB checks):** `./scripts/miri.sh` runs [Miri](https://github.com/rust-lang/miri) on the `engine` crate; included in `./scripts/verify.sh --deep` and a separate GitHub Actions `miri` job. On macOS, cross-interprets as `x86_64-unknown-linux-gnu` (Miri lacks kqueue support on Darwin host).
- **Playwright:** browser-only E2E for Settings → Providers (navigate, switch provider, save API key).
- **Delete workflow from settings:** Workflow Settings panel danger zone permanently deletes the active workflow (confirm dialog; blocked while a run is active on that workflow).
- **MCP external tools (v1):** `McpSettings` on `AppSettings`, `rmcp` stdio adapter, registry merge + dispatch, run-scoped client wiring in `drive.rs`, `probe_mcp_server` IPC, Settings **MCP Servers** section.
- **Amazon Bedrock provider:** builtin `bedrock` profile using AWS Converse/ConverseStream (`aws-sdk-bedrockruntime`), credential-chain auth, region in settings, and Settings **Refresh from AWS** for `ListFoundationModels` model catalog.
- **Sidebar projects collapse:** Projects section chevron toggle matches workflows — collapse/hide project folders; preference persists in localStorage.
- **Legal entrypoints:** MIT `LICENSE`, `SECURITY.md`, and root `CONTRIBUTING.md` pointing to `docs/contributing/`.
### Changed

- **InteractiveEngine ownership:** index-based layer iteration avoids cloning whole node layers; move `NodeId` into poll/run payloads instead of extra clones; replace recursive `poll()` with a loop after stale in-flight recovery.
- **CI verify gate:** GitHub Actions runs lean `./scripts/verify-ci.sh` (fmt, clippy, test-fast with workflow acceptance, arch, ui-test, deny) instead of full `./scripts/verify.sh`; drops nightly public-api, machete, typos, doc, and desktop/Tauri compile from the blocking path. Run `./scripts/verify.sh` locally before handoff.
- **Public release readiness plan:** [`docs/superpowers/plans/2026-06-20-public-release-readiness.md`](docs/superpowers/plans/2026-06-20-public-release-readiness.md) — phased checklist for settings consolidation, UI shell polish, Bedrock/MCP, CI/docs/GTM before public launch.
- **Chat parallel hint:** when multiple agents run in parallel on the All view, show a status banner above the composer bar directing users to select a node to view and reply.
- **Orchestration headless E2E:** `MockAiStack` test helper (`crates/orchestration/tests/support/`) pops scripted `AiPort` responses from a stack; `workflow_e2e.rs` covers happy path, auto-retry, missing input/approval, exhausted stack, and interrupt during slow tools — no real providers.
- **Roadmap:** [#38 In-app file viewer from node output](docs/ROADMAP.md#in-app-file-viewer-from-node-output) — clickable file references in node/chat output open paths in an in-app reader (syntax highlight, markdown, line ranges); Tier 5, further out.
- **Roadmap:** [Workflow insights](docs/ROADMAP.md#workflow-insights) — design-time advisory panel for graph smells, config gaps, and best-practice suggestions (queue #36); distinct from pre-run blocking validation (#10) and post-run run insights (#27).

### Fixed

- **Settings field grid controls:** match TextSelect dropdown height to adjacent text inputs in Connection and tool config grids.
- **TextSelect scroll:** scrolling long option lists no longer closes the dropdown; ancestor scroll still dismisses the menu.
- **Schedule workflow picker theme:** node-picker list options use semantic surface tokens in dark mode (fixes low-contrast white-on-grey rows); long workflow names truncate to a single line.
- **Search missing path:** `search` on a non-existent literal path now returns `[not_found]` instead of success with "No matches found"; headless acceptance `search_missing_path_surfaces_not_found_not_empty_success` guards regression.
- **Workflow switch chat restore:** replace run-state snapshots on workflow switch instead of cross-workflow `reconcile` merges; refresh from backend when returning to the live-run workflow; canvas node clicks open Chat and route to the node's transcript (live pick or settled filter).
- **Malformed submit_output:** when the model calls `openflow_submit_node_output` with only `assistant_message` (no `output` wrapper), salvage that prose into the node output schema instead of failing after retries; retry feedback now includes the node's output schema.
- **Malformed submit_output incidents:** each failed AI invoke now emits `AiInvokeFailed` telemetry and persists an `ai.malformed_submit_output` incident (category `ai_invoke`, retryable) to `{data_local}/openflow/incidents.jsonl` via the existing incident recorder.
- **Schedule topbar title:** show "Schedule" in the app header on the schedule screen instead of the previously active workflow name.
- **Agents screen background:** agents list and detail panels fill the viewport height so `surface-ground` extends to the bottom instead of exposing the main-shell gradient.
- **Chat send flash:** instant auto-scroll on new messages, stable composer footer during kickoff, and no fade-in on user bubbles — panel no longer jumps on send.
- **Idle chat kickoff:** skip entrypoint chat record for manual (`auto_start: false`) root nodes — the same text is recorded once when the UI auto-submits to the awaiting node.

### Changed

- **Chat item spacing:** uniform `--chat-item-gap` between tool lines, thinking rows, and messages inside a segment (removes stacked per-type padding).
- **Chat tool lines:** tool invocations show text-only verb labels (`Reading` → `Read`) without status icons; thinking rows collapse to `Thinking` / `Thought for a while` with expand for full reasoning.
- **UI module layout:** `components/` and `lib/` roots now contain only subdirectories plus a root `index.ts` barrel; 14 generic components moved into per-component folders; all lib modules moved into folder barrels; `@/*` path alias added for new imports.
- **Chat segment spacing:** hairline dividers and `--chat-segment-gap` rhythm between agent sections; translucent sticky headers replace opaque grey bands; completed status demoted to muted text; focus flash uses header accent bar.
- **Roadmap:** [#37 Agent prompt skill references](#agent-prompt-skill-references) — `/skill` tokens in saved-agent and node system/task prompts with expansion at run-start (mirrors composer slash skills).
- **Phase 2 UI refinement:** semantic typography, radius, and layout tokens; shared empty-state styling via `PanelEmptyState`; route-level cards normalized onto palette tokens (no per-screen page headers).
- **Scrollbar styling:** thin token-based scrollbars (WebKit + Firefox) replace default OS chrome across scrollable panels.
- **Run history status badges:** use scoped chip classes so run rows no longer inherit canvas status-dot backgrounds (fixes low-contrast "Stopped" in light mode).
- **Project workflow copy:** per-project **Copy from…** picker duplicates a workflow into the target project (new ID, independent edits) instead of linking the same workflow across projects.
- **Search tool wiring:** remove unused `ContentSearch` port trait and `RipgrepSearch` wrapper; `blocking_ops` calls `search_at` directly.
- **Run execution timing:** inline `timing.rs` helper into `run/execution/mod.rs` beside `send_or_log`.
- **Shell navigation motion:** disable View Transition API for route changes (opacity fade only) — VT size morph conflicts with UI zoom in the Tauri webview.
- **Runs dock tab:** remove redundant manual refresh and duplicate "Runs / History" header; history loads when you open the tab.
- **Dock empty states:** Overview, Terminal, and Runs tabs use the shared `PanelEmptyState` pattern (icon, title, description) matching chat.
- **Select styling:** replace native `<select>` with custom `TextSelect` listbox so dropdowns render below the trigger with app tokens instead of macOS native menus.
- **Schedule repeat interval:** add minutes, hours, days, and weeks unit selector; cron mapping stays UI-side (orchestration already evaluates arbitrary cron).
- **Schedule time picker:** replace inline time input and weekday chips in the TIME / EVERY column with a compact summary button that opens a dialog.
- **Schedule day selection:** replace Daily/Weekdays/Weekly presets with an at-time mode that supports multi-day toggles plus All/Weekdays shortcuts.
- **Schedule time/day layout:** modal uses a single-row day grid for at-time editing.
- **Schedule repeat controls:** keep Every / value / unit on one horizontal row (drop clashing `schedule-field` grid wrapper, fixed widths, wider column).
- **Schedule timezone:** remove timezone picker from the schedule table; saves always use the computer's local timezone.
- **Schedule repeat validation:** clamp day repeat intervals to cron-safe ranges (days cannot exceed 31); heal invalid persisted schedules like `*/210`. Remove weeks unit — cron cannot represent multi-week intervals honestly; use days (e.g. 7, 14) or At time for weekly patterns.
- **Workflow validation UX:** remove the manual Validate toolbar button; validate the DAG automatically when nodes or edges are added (invalid edges are reverted with an error toast).

### Added

- **Phase 1 UI hierarchy and compact shell:** unified base + semantic palette contract in `styles/index.css` with legacy bridges; labeled primary Run/Continue/Stop actions in the topbar; stronger dock tab hierarchy with active-panel context; compact viewports use an overlay sidebar drawer so the canvas stays first-class; focused header/shell/dock tests.
- **Chat segment visual regression:** Playwright snapshot test (`crates/desktop/e2e/tests/chat-segments.visual.spec.ts`) for multi-node settled transcript spacing in dark theme.
- **Screen view transitions:** native View Transition API for SolidJS shell navigation — lateral cross-fade between sidebar routes, directional slides for settings and workflow authoring, persistent sidebar/header isolation; CSS recipes from the view-transitions skill in `styles/index.css`; `lib/viewTransition.ts` helper with unit tests.
- **Engine change skill (rewritten):** intake gates (purity, validity/behavior/contract, execution mode, subagent vs catalog); engine-specific verification ladder; distinct from orchestration skill template.
- **Orchestration change skill:** `.cursor/skills/openflow-orchestration-change/SKILL.md` — process-only guide for `crates/orchestration` changes; auto-attaches via `globs: crates/orchestration/**`; wired into `docs/contributing/development-lanes.md` skill table.
- **Approval-mode-only tool config:** replace per-tool checkboxes with a single approval-mode dropdown (`read_only`, `write`, `always_ask`, `yolo`); all builtins available except in `read_only`; static read/write capability classes; remove `ToolPolicyOverride`, catalog selection, and critical-bash YOLO guard.
- **Tauri Playwright E2E:** optional `e2e-testing` feature wires `tauri-plugin-playwright` into the desktop shell; `crates/desktop/e2e/` runs browser-only (mocked IPC) or native webview tests via `@srsholmes/tauri-playwright` (`npm --prefix crates/desktop run e2e:browser` / `e2e:tauri`).
- **Verify parallel runs:** `./scripts/verify.sh` uses an isolated `target/verify-<pid>` by default so multiple agents can verify concurrently without blocking on Cargo's `target/.cargo-lock`; set `VERIFY_SHARE_TARGET=1` for faster solo runs against shared `./target`.
- **Ponytail (Cursor):** install [ponytail](https://github.com/DietrichGebert/ponytail) lazy-senior-dev rule (`.cursor/rules/ponytail.mdc`, always-on) and skills (`.cursor/skills/ponytail*`) for review, audit, debt, gain, and help.
- **Durable run replay UX:** replay mode no longer shows the idle kickoff composer (which started a fresh run on send); replay state applies to the active workflow directly; chat shows a read-only banner with resume action; live run events only exit replay when the run is active again.
- **Durable run persistence:** interactive runs persist to disk under `{project}/.flow/runs/` or `{data_local}/openflow/runs/` with append-only checkpoints combining engine state and UI projection; list/replay/resume via desktop IPC; Runs dock tab for history, read-only replay, and durable resume after restart.
- **macOS run notifications:** native desktop notifications when a workflow needs input, requests tool approval, completes, errors, or aborts; classification lives in `crates/desktop/src/run_notifications.rs` and fires from the run event bridge after each applied execution event.

### Fixed

- **Chat composer chip focus:** hide native text selection on the mirror textarea so clicking @/skill chips no longer flashes a blue rectangle over the chip overlay.
- **Terminal tab close icon:** use Lucide `X` instead of `Square` on session tabs so close reads as dismiss, not stop.
- **Schedule and workflow authoring tokens:** define `--surface-ground`, `--surface-panel`, `--surface-raised`, `--border-subtle`, and semantic text tokens so those screens no longer fall through to browser defaults.
- **Chat composer send button:** restore compact 36px circle styling after Phase 1 `.primary-button` rules overrode `.composer-send-button` padding and shadow.
- **Dock tab bar:** revert Phase 1 pill/context styling to the prior flat VS Code-style tabs (muted labels, subtle active fill).
- **Clippy:** allow test panics in `subagent_runtime` tests; fix `mapping.rs` usage extraction casts, wildcard match arm, and redundant clone on early return.
- **Chat composer:** align the caret with inline skill and file chips by sizing highlight spacers to the raw token text instead of the chip bubble width.
- **Chat composer:** render slash-command skills as inline chips in the input (same bubble styling as file and folder references) instead of a separate pill beside the send button.
- **Chat composer:** pin the chat input bar to the bottom of the dock panel so it stays visible while scrolling message history; tool approvals and kickoff/live pickers render in the same sticky footer.
- **Chat panel:** remove the inline "files changed" block from below conversation bubbles to keep the composer area focused.
- **Chat bottom bubble stability:** keep the assistant text bubble visible at the bottom even when content is currently empty, so layout no longer jumps while waiting for text.
- **Run stop / chat restart:** stop orphaned runs when the execution task is already gone; ignore stale run events after stop; re-read canonical run state before emitting UI updates so force-stop no longer leaves chat stuck on "Starting workflow…".
- **Workflow chat:** switching workflows clears the chat panel and restores each workflow's own run history, drafts, and continuable-run controls instead of showing the previous workflow's conversation.
- **Workflow canvas:** stop `focusChatNode` tick spam on repeated run-state events and unchanged node selection — breaks the `fitView` feedback loop that triggered "Maximum update depth exceeded"; skip viewport panning when chat-focus mode collapses the canvas; replace `onSelectionChange` with click handlers and ignore programmatic `select` changes to break the Solid↔React Flow selection sync loop; bail out of node/edge reconcile when data is unchanged; fix Solid/React canvas host initial render race.
- **Workflow authoring errors:** map AI turn failures to `workflow authoring failed` instead of mislabeling them as file edit preview errors.
- **Chat composer busy state:** remove the broken `is-busy` pseudo-element border animation that rendered a blue bar over the input; keep the send-button spinner and a subtle border highlight instead.
- **Parallel chat node switching:** keep live node picker bubbles selectable after choosing a running node so you can switch active chat target between concurrent nodes without waiting for completion.
- **Chat node filter chips:** show all running and settled nodes in the top filter bar (including parallel live nodes); remove the duplicate bottom live-node picker; only one chip highlights at a time (live pick vs history filter are mutually exclusive); keep chip order stable as nodes move between live and settled.
- **Ad-hoc subagent output schema:** ad-hoc subagents now get the default structured `summary` output schema instead of `null`, so `openflow_submit_node_output` tool parameters validate on strict OpenAI-compatible providers; provider mapping also falls back when `output_schema` is null; subagent AI invocations retry transient provider errors using the workflow `retry_policy`.
- **Workflow authoring parsing:** accept flat draft fields and `workflow_draft` aliases when models omit the `workflowDraft` wrapper; tighten the authoring system prompt with an explicit required shape.
- **Workflow authoring clarification:** disable `request_user_input` for authoring turns, forbid clarifying questions in the system prompt, and retry once with draft-required feedback when a model still pauses for input.
- **Workflow authoring submit-output:** retry up to three times with corrective feedback when the model's `openflow_submit_node_output` call is malformed, matching interactive run retry behavior.
- **Workflow authoring thinking display:** render `thinking` authoring messages with the same collapsible thinking bubble used in node chat, so Build with AI shows expandable reasoning previews.
- **Workflow authoring draft readability:** stack the Build with AI preview above chat instead of side-by-side, and auto-layout AI-generated draft nodes by DAG layer before preview/apply so nodes do not overlap.
- Settings bootstrap creates timestamped `.bak` files instead of clobbering; write errors propagate
- JSON atomic writes fsync temp file before rename

### Changed

- **Chat file references:** increase composer file/folder chip size (padding, font, icon), show longer paths before truncating, and reserve chip width in the highlight layer so following text no longer overlaps.
- **Chat file references:** render completed `@{path}` tokens as inline file/folder chips in the composer instead of raw brace syntax.
- **Schedule sidebar:** new Schedule screen for cron-based workflow runs while the desktop app is open; persists schedules on `WorkflowSettings.schedule`; orchestration owns cron evaluation and due-run claiming; desktop timer bridge starts scheduled runs and emits schedule status events; workflow picker opens in a modal like the node picker instead of an inline list.
- **Terminal tab chrome:** replace the path + large Stop toolbar with a compact Codex-style session tab (folder name, inline stop icon, new-terminal control); full cwd stays on hover only; `+` opens additional PTY sessions instead of blocking on an existing one.
- **Workflow authoring UI:** "Build with AI" is a routed screen inside the main shell (sidebar + topbar stay visible) instead of a full-screen modal overlay; composer stays typeable while provider readiness loads and shows an inline warning when the provider is not ready; live read-only canvas preview appears beside the chat once the AI proposes a workflow with nodes.
- **Terminal theme:** xterm background, foreground, and selection colors follow the inspector panel surface (`--panel-surface`) instead of a hardcoded dark palette.
- **Run trace dark mode:** trace list/detail panels and status pills use theme variables instead of hardcoded light backgrounds.
- **Sidebar workflows chevron:** move collapse control to the right of the label (before the new-workflow button); show on section hover only.
- **Dock panel tabs:** restyle Overview / Chat / Run trace switcher to flat Cursor-like buttons — 4px radius, no border or shadow, subtle hover fill.
- **Workflow canvas:** hide the default React Flow corner attribution via `proOptions.hideAttribution`; fit the full graph when a workflow loads and pan to a node on canvas selection or chat focus (`fitView` via `useReactFlow`).
- **Node runtime preamble:** `NODE_RUNTIME_PREAMBLE` now documents all builtin and harness tools (read/search/find/ast_grep, write/edit/apply_patch, bash, subagents) with when-to-use guidance and usage conventions.

### Documentation

- **ROADMAP.md:** cross-audit queue vs codebase — correct premature Done on #14 (terminal vs jobs), #4 (hook seam vs registration), provider thinking (OpenAI-compat vs Anthropic), terminal warn-on-close; mark #15 attachments, #23 cron, and #29 shortcuts as In progress with shipped slices; refresh stale Layer \| Gap tables (thinking, global chat, attachments); add [Cron / scheduled runs](#cron--scheduled-runs) detail spec; mark T20–T21 Done in Phase 2.
- **ROADMAP.md:** [Programmatic / non-AI nodes](docs/ROADMAP.md#programmatic--non-ai-nodes) — queue item #25 expanded with Code/Transform/Http node kinds, engine `CallProgrammatic` branch, and sandboxed script execution. [Workflow orchestration & reinvoke](docs/ROADMAP.md#workflow-orchestration--reinvoke) — new queue item #35; child `invoke_workflow` runs, foreach-over-repo-files batch pattern, in-app `.flow/scripts/` driver, and partial `reinvoke_from_node`; promoted from deferred multi-run orchestration.
- **ROADMAP.md:** [Run insights & self-learning](docs/ROADMAP.md#run-insights--self-learning) — queue item #27; post-run insight extraction, human approve/dismiss gate, workflow/project/node-scoped injection into future runs; insight taxonomy, storage layout, and dependency on run persistence (#24). Renumbered queue items #28–#34.
- **OMP parity plans (2026-06-15):** add implementation plans for live bash/jobs/terminal, MCP external tools, browser tool, eval tool, and run persistence under `docs/superpowers/plans/`; update read parity plan status (Task 0 done).
- **ROADMAP.md:** audit queue and detail specs against the codebase — mark chat presentation, thinking/reasoning, transcript correctness (T9–T12), in-session continue, T19, global chat status dots, and node completion NC-11/NC-12 as Done or in progress where shipped.
- **ROADMAP.md:** [Node handoff artifacts & output review](docs/ROADMAP.md#node-handoff-artifacts--output-review) — queue item #17; per-node `plan.md` (or custom) under `.flow/runs/{run_id}/handoffs/{node_id}/`; per-node opt-in review gate before downstream handoff; in-app plan review UI modeled on `tools/plan-review.html`. Renumbered queue items #18–#33.
- **ROADMAP.md:** [Node completion](#node-completion) acceptance criteria (NC-1–NC-14) — submit contract, upstream gate, chat summary bubble, trace output, and remaining T10/checkpoint gaps; linked from queue item #5.
- Document that new or changed builtin tools must update `NODE_RUNTIME_PREAMBLE` alongside `tool/registry.rs` (`AGENTS.md`, `technical-overview.md`, hexagonal rules).
- Add Staff+ deep audit report (`docs/audits/2026-06-13-deep-audit.md`) covering checkpoint/continue, bash tool, persistence, IPC, and refactor backlog.
- Add four TDD implementation plans in `docs/superpowers/plans/2026-06-13-*.md` (checkpoint integrity, bash hardening, persistence atomicity, headless retry parity).
- **ROADMAP.md:** queue item **#14 Project terminal** — interactive shell tab in the bottom dock (PTY backend, xterm.js, project/run cwd); promoted from backlog; renumbered items #15–#32.

### Added

- **Workflow default reasoning effort:** gear-panel control for per-workflow `reasoning_effort` and budget tokens; orchestration applies workflow defaults before provider defaults at run start; new nodes inherit workflow then provider defaults.
- **OMP read tool parity:** `tool/read/` module with OMP-style selectors (`:raw`, `:N-M`, `:N+M`, multi-range), structural summaries for bare code reads, depth-limited directory listings, and unified rendering for local/URL/artifact paths.
- **Natural language workflow builder:** ChatGPT-style **Build with AI** overlay — describe workflows in conversation, see DAG + semantic validation after each turn, apply valid drafts to the editor.
- **Tool retry & resilience (T20–T21):** transient tool failures retry per workflow `retry_policy` before surfacing `is_error` results; `ToolRetrying` telemetry; engine fills missing tool-batch results so cancelled/interrupted tools resume `CallAi` instead of aborting the run.
- **Persistent error reporting (backend slice, UI deferred):** structured incidents now persist in `{data_local}/openflow/incidents.jsonl` with run/node scope; `RunCoordinator::apply_execution_event` records tool/node/subagent/run execution failures; backend IPC errors plus terminal start and workflow/settings persistence failures are captured; retention pruning (`incident_retention_max`) and incident lifecycle commands (`list_incidents`, `dismiss_incident`, `clear_resolved_incidents`) ship through backend + Tauri.
- **Incident IPC (no UI):** `IncidentSummary` DTO, `list_incident_summaries` on `AppBackend`, and `list_incidents` / `dismiss_incident` Tauri commands for unresolved incident list and dismiss.
- **Incident retention policy:** `AppSettings.incident_retention_max` (default 500); `IncidentRecorder` prunes oldest resolved then oldest overall after append; `clear_resolved_incidents` on `AppBackend` and desktop IPC.
- **Incident JSONL store:** append/list/dismiss/clear for structured incidents at `{data_local}/openflow/incidents.jsonl` via `FileIncidentStore` and `IncidentStore` port.
- **Terminal and persistence incident capture:** `start_terminal` failures record `terminal.start_failed`; explicit `save_settings` / `save_workflow` / `save_workflows` failures record `persistence.settings_save` and `persistence.workflow_save`.
- **Incident from telemetry:** map `RunTelemetry` failure events (`ToolCompleted` errors, `ToolDenied`, `NodeErrored`, `NodeFailed`, `SubagentFailed`, `Error`) to `IncidentRecord` via `incident_from_execution_event`.
- **Run-scoped incident capture:** `RunCoordinator` now assigns per-run `run_id`s and records incident records from execution events before state projection.
- **Incident domain model:** structured `IncidentRecord` with severity, category, scope, and camelCase IPC serialization in `orchestration::incident`.
- **Project terminal:** add a Terminal dock tab backed by a native PTY session, xterm.js rendering, project-cwd startup, resize handling, and app-close cleanup.
- **Chat file references:** type `@` in the chat composer to search project files (gitignore-aware), pick files as `@{path}` tokens, and include bounded UTF-8 file contents in kickoff and paused-node chat submissions.
- **Start run from global chat:** message the idle workflow composer to start a run with entrypoint text; header **Run** still starts without entrypoint for zero-input workflows; manual root nodes auto-receive the same text via `submit_user_input` when exactly one node awaits.
- **Stop and continue runs:** user stop snapshots `InteractiveEngine` state in-session; **Continue** resumes from checkpoint with transcripts, outputs, and pause points preserved; **Run** still starts fresh; `continue_run` / `is_run_continuable` desktop IPC; header Continue + fresh-run buttons when a stopped run is resumable; ⌘/Ctrl+Enter continues when continuable.
- **Sleep prevention during runs:** active workflow runs hold an OS idle/sleep assertion (display may still turn off); released when the run stops, completes, or the app closes.
- **LLM-usable tool errors and docs (ROADMAP T19):** typed `ToolError` variants (`NotFound`, `PermissionDenied`, `InvalidArgs`, `Timeout`, `Cancelled`, `ExecutionFailed`) with actionable hints and `is_retryable()` for transient classification; spilled output readable via `read` + `artifact:{id}` selectors; expanded builtin tool schemas/descriptions; cache-hit stubs point at prior call id with a head excerpt; tool denial threads optional user reason through engine → orchestration → desktop IPC; removed dead `ToolCall.intent`.
- **Right panel hide/show toggle:** toggle button in editor toolbar and ⌘/Ctrl+J shortcut to hide/show the inspector/workflow-settings panel; panel state persisted in localStorage; canvas expands to full width when hidden; auto-unhide when opening workflow settings.
- **Provider prompt caching:**
- **Plan review tool:** standalone `tools/plan-review.html` — load or paste markdown plans, select text to comment, verdict chips (approve/block/question), threaded replies, import exported reviews with plan diff, export review notes; documented in [ROADMAP.md](docs/ROADMAP.md#interactive-plan-review-tool). Session storage is v2-only (`plan-review-session-v2`); v1 localStorage is wiped on load.
- **`scripts/verify.sh` hardening:** LLM-friendly output (run all steps, one-line PASS/FAIL, truncated logs, repro summary); new gates for `doc`, `ui-typecheck`, `machete`, `typos`, and clippy-max strictness; optional `--deep` adds `cargo mutants` (missed-mutant note on failure); positional step filter (`./scripts/verify.sh clippy ui-test`); `VERIFY_FAIL_FAST=1` and `VERIFY_MAX_LINES` overrides; root `typos.toml` and `.cargo/mutants.toml`; contract documented in `docs/contributing/testing-workflows.md`, `AGENTS.md`, README, and `.cursor/rules/Verification-and-Lint.mdc`.
- **Lint anti-silencing:** workspace `allow_attributes_without_reason = "deny"` — every `#[allow]` / `#[expect]` must carry `reason = "..."`.
- **Global chat:** settled run history in execution-layer order above a live strip of per-node columns; per-node composers and approval cards; node filter chips; canvas selection no longer steals focus on pause (`projectChatLayout`, `LiveNodeColumn`, `ConversationSegmentMessages`).
- **ROADMAP.md:** [Run checkpoint, history, and replay](docs/ROADMAP.md#run-checkpoint-history-and-replay) — persist checkpoints to disk, run history UI, resume paused runs, read-only replay, and fork-from-checkpoint; expands queue item #23 and ties to persistence policy (#6).
- **AI retry policy:** default 3 auto-retry attempts with exponential backoff (base `backoff_ms`, capped at 30s) in `InteractiveEngine::run` and `WorkflowRunner`; cancellation-aware backoff sleep; gear-panel controls for max attempts and backoff; `RetryPolicy::delay_for_attempt`.
- **Reasoning effort controls:** per-provider default in Settings plus per-node override in the inspector (effort level and budget tokens).
- **Review-driven tests:** interrupt during slow bash tool emits `NodeInterrupted`; parallel retry does not re-emit sibling `NodeAwaitingInput`; headless runs return `MissingRetry` on retryable node failure; chat-completions body forwards `reasoning_effort` / budget fields.
- **Per-node interrupt and retry:** interrupt a thinking/running-tool node without stopping the run (`interrupt_node`); retry failed or interrupted nodes with transcript preserved (`retry_node`); canvas stop/retry actions on node status row; `AgentStatus::Interrupted` and retryable `NodeErrored` / `NodeInterrupted` telemetry while the run stays active.
- **UI polish overhaul:** `motion` animation library; motion tokens and `prefers-reduced-motion` support; animated modals (fade + scale) with focus trap and Escape-to-close; inspector panel slide-in; screen crossfade; dock height transition; canvas node pulse on `started` / `running_tool` and animated edges during runs; streaming caret and thinking-bubble shimmer; tool output expand/collapse; shared `Spinner` and bootstrap skeleton; keyboard shortcut cheatsheet (`?` or sidebar); dark mode (system/light/dark) in Settings; header button shortcut tooltips.
- **Provider thinking in node chat:** stream `reasoning_content` / `reasoning` from OpenAI-compatible APIs into collapsible `ThinkingBubble` rows in the selected node's conversation (collapsed preview by default; expand for full reasoning).
- **ROADMAP.md restructure:** single prioritized queue (30 sequenced items across 6 tiers + unsequenced backlog) replacing category tables; detail specs preserved below the queue. New items: macOS Keychain key storage, pre-run workflow validation, token & cost tracking, canvas editing QoL (undo/redo, duplicate node), onboarding & templates, macOS distribution (signing, notarization, auto-update). Status corrections: chat presentation marked In progress; single Tokio runtime marked Done.
- **Testing conventions:** standardised test placement rules — inline `#[cfg(test)] mod tests` by default, sibling `foo_tests.rs`/`tests.rs` extraction past ~150 lines, crate-level `tests/` for integration, Vitest siblings for frontend — documented in `docs/contributing/testing-workflows.md` and enforced via `.cursor/rules/testing-conventions.mdc`.
- **Coverage tests:** `api.test.ts` exercises Tauri IPC wrappers; component tests for workflow settings panel, tool approval card, and project folder row; backend/coordinator tests for rename, settings persistence, run-state idle, workflow unassign, and denied tool approval with reason.
- **Bash tool:** agent `bash` builtin (oh-my-pi–aligned) — `command`, optional `cwd`/`env`/`timeout`; non-interactive env defaults; merged stdout/stderr; wall-time and exit-code notices; `ToolTier::Exec` with critical-pattern approval override; opt-in via node tool config.
- **ROADMAP.md:** [Context used](docs/ROADMAP.md#context-used) — structured per-turn context breakdown in composer and chat; ledger of shared context, rules, skills, attachments, and upstream artifacts.
- **ROADMAP.md:** [Attachments & file references](docs/ROADMAP.md#attachments--file-references) — attach button, drag-drop, and image paste; expands prior file-references plan.
- **ROADMAP.md:** model thinking settings — workflow default in gear panel plus per-node inspector override (Thinking & chat presentation).
- **ROADMAP.md:** [Global chat](docs/ROADMAP.md#global-chat) — unified chat pane across node progression; execution-layer message ordering; separate reply bubbles for parallel awaiting nodes.
- **ROADMAP.md:** [Canvas run feedback](docs/ROADMAP.md#canvas-run-feedback) — scrollable in-node subagent list; colored status icons per agent state (thinking, done, etc.); chat filter chips and live-node picker to use the same status colors as canvas.
- **ROADMAP.md:** [Global chat](docs/ROADMAP.md#global-chat) — chat node bar status color parity (fix `.chat-filter-status-dot` gray override).
- **Node status labels:** canvas nodes show descriptive statuses — Thinking, Waiting for Input, Awaiting Approval, Running Tool, and more — with matching colors for each state.
- **Chat markdown:** assistant, user, system, and thinking messages render as Markdown (`solid-markdown`) with styled headings, lists, code blocks, tables, and links.

### Fixed

- Headless workflow runs auto-retry transient node errors (parity with interactive engine).
- Tool errors with transient indicators (timeout, connection reset) are retryable.
- **Checkpoint / continue integrity:** reject checkpoints whose node ids no longer exist in the workflow (`StaleNodeIds` / `CheckpointIncompatible`); retain in-flight tool batches across stop (prevents duplicate side effects on resume); `prepare_resume` returns nodes it could not retry; resolve approval `node_id` from engine pending batch before `on_tool_decision` clears it.
- **Bash tool hardening:** timeout/cancel kills the process group (grandchildren no longer survive); timeout preserves partial stdout/stderr; incremental pipe reads replace `read_to_end` (groundwork for live streaming).
- **`scripts/verify.sh`:** failure log headers no longer pass `---` strings to `printf` as format literals (macOS treats them as flags).
- **`providers`:** clippy/doc fixes for `prompt_cache.rs`; extract Anthropic cache-control test fixtures to satisfy `too_many_lines`.
- **`desktop`:** bootstrap debug logging uses `inspect_err` instead of identity `map_err`.
- **Standards cleanup:** remove temporary debug-session instrumentation from `desktop`, `settings_store`, and `AppProvider`; extract `anthropic.rs` wire tests to `anthropic_tests.rs`.

### Changed

- **Chat presentation:** thinking bubbles constrain to pane width (long code/tables scroll inside); removed horizontal dividers between messages, segments, tool rows, and expanded thinking bodies; markdown `hr` hidden in conversation.
- **Global chat (single node):** one running node appends into the main history stream instead of a separate live column; parallel live nodes still use the live strip until each finishes; assistant messages inside a segment header no longer repeat the node label; settled segments sort by run/interaction order (trace + append ledger) instead of re-sorting parallel siblings by DAG layer.
- **Engine crate refactor:** `string_id!` macro for `NodeId`/`EdgeId`/`WorkflowId`; shared `tool_results` and `retry` helpers; `NodeFailureKind` replaces stringly `RunError::NodeFailed` messages; `InteractiveEngine` split into `mod.rs` / `completion.rs` / `tools.rs` / `tests.rs`; public mutators take `&NodeId`; `PendingToolApproval` and `EditBatch` use `NodeId`; `tool_decision_for_call` single-pass policy lookup; validation helpers for duplicate ids and edge endpoints.
- **Storage paths:** all app persistence under `{data_local}/openflow/`; removed `step-through-agentic-workflow` directory fallback and legacy path migration.
- **Orchestration deepening (architecture review):** shared `JsonFileStore` helpers (`atomic_write`, `read_json_file`/`write_json_file`) for agent/project/workflow/settings stores; `template_store` and `project_workflow_store` use shared atomic writes; LSP settings flow through `ToolExecutionContext` (via `ToolPortImpl`) instead of a `ToolRunner` field; blocking edit/read ops moved to `tool/blocking_ops.rs`; subagent AI loop extracted to `run/execution/subagent_session.rs`; builtin dispatch split to `tool/dispatch.rs`; `finish_run_session` centralizes run teardown in `RunCoordinator`.
- **Clippy-max in verify:** `engine` and `providers` pass pedantic/nursery/cargo lints; `orchestration` and `desktop` retain documented crate-level opt-outs until adapter lint backlog is cleared.
- **Unused dependencies:** removed `async-trait` from `desktop` and `grep-matcher` from `orchestration` (machete).
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

- **Chat bar on run start:** keep the live strip visible while a run is active but no node has reached a live status yet (`Starting workflow…` placeholder); classify `awaitingNodeIds` as live before `statusByNode` catches up; open the chat dock when Run is clicked; fix `AgentStatus` IPC encoding (`awaiting_input` not `awaitingInput`) so live chat columns and canvas pills recognize paused/running-tool nodes.
- **Bootstrap loading hang:** invalid or legacy `openflow/settings.json` (missing `providers` wrapper) no longer blocks startup — file is renamed to `.json.bak`, defaults are written, and the app loads with empty settings.
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
