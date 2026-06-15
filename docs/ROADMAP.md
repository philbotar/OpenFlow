# Roadmap

A single prioritized queue. Work top to bottom — each numbered item is meant to be a self-contained chunk you can finish before moving on. Detailed specs for the larger items live in [Detailed specs](#detailed-specs) below; domain task IDs (T1–T21) are specced in [Domain engine hardening](#domain-engine-hardening).

**Status:** Done · In progress · Planned

---

## The queue

### Tier 1 — Finish what's started

| # | Item | Status | Details |
| --- | --- | --- | --- |
| 1 | **Chat presentation** — thinking bubbles, collapsible tool rows, tool intent summaries, live tool updates, pretty tool names, args one-liners | In progress | [Chat presentation](#chat-presentation--thinking-bubbles--tool-cleanup) — thinking bubbles, collapsible rows, args one-liners, and streaming reasoning **Done**; pretty tool names, live tool UI, and tool intent **remain** |
| 2 | **Entrypoint wiring** — pass entrypoint text from UI through `start_run` to root node input | Done | [Entrypoint wiring](#wire-entrypoint-text-through-the-desktop-run-path) |

Entrypoint wiring is small but blocks attachments (#15) and any "kick off a run with instructions" flow — do it early.

### Tier 2 — Reliability core

Make runs survivable before adding features on top. A failed tool call or transient provider error should never kill a run.

| # | Item | Status | Details |
| --- | --- | --- | --- |
| 3 | **Error taxonomy + AI retry** — T1 (`AgentError` transient/permanent), T2 (collapse templates), T3 (node lookup index), T5 (tool deny/decision resume), T6 (`retry_policy` with exponential backoff, default 3 attempts) | Done | [Phase 1–2](#phase-1--foundations) |
| 4 | **Tool retry, hooks & resilient failure** — T19 (tool error taxonomy), T20 (tool invocation retry), T21 (failed tools feed transcript and resume `CallAi`; never abort the run), before/after tool hooks for approval/audit/guards | In progress | [Tool retry](#tool-invocation-retry-and-resilience) — T19 **Done**; T20–T21 and hooks **remain** |
| 5 | **Transcript & event correctness** — T9 (strip redundant tool-call XML), T10 (validate node id in `on_ai_complete`), T11 (run-event semantics), T12 (template store persistence errors); [node completion](#node-completion) acceptance | In progress | [Phase 2–3](#phase-2--functional-gaps) · [Node completion](#node-completion) — T9–T12 and NC-1–NC-10 **Done**; NC-13–NC-14 **remain** |
| 6 | **Run lifecycle leftovers** — clean up `openflow-run-*` temp dirs, store event-bridge task handle, decide checkpoint/persistence policy and durable artifact layout (in-memory only vs. disk checkpoints vs. resume after restart) | Planned | [Run lifecycle](#run-lifecycle) |
| 7 | **Secure key storage** — move provider API keys from plaintext `settings.json` to macOS Keychain (keep env-var fallback); migrate existing keys on first launch | Planned | *New* |

### Tier 3 — Daily-driver UX

The things you hit every single run.

| # | Item | Status | Details |
| --- | --- | --- | --- |
| 8 | **Canvas run feedback** — colored status icons per agent state; scrollable in-node subagent list (drop `+N more`); chat node chips use same status colors as canvas | Planned | [Canvas run feedback](#canvas-run-feedback) |
| 9 | **Thinking levels** — `reasoning_effort` schema (node + provider default), gear-panel + inspector controls, provider reasoning param wiring, thinking transcript items | In progress | [Thinking & chat presentation](#thinking--chat-presentation) — per-node inspector, provider Settings default, provider wiring, and `ThinkingBubble` **Done**; workflow gear-panel default and per-run override **remain** |
| 10 | **Pre-run workflow validation** — validate before `start_run`: dangling edges, cycles, missing provider/model/key, empty prompts; surface as canvas badges + blocking dialog | Planned | *New* |
| 11 | **Project rules** — `.flow/rules/` under linked projects; discovered on load, merged into shared context at run start | Planned | [Project rules](#project-rules) |
| 12 | **Input queue + structured questions** — type ahead during active runs (buffer per node, drain on `AwaitInput`); option-card questions via extended `openflow_request_user_input` | Planned | [Agent questions & todos](#agent-questions--todos) |
| 13 | **Token & cost tracking** — per-turn usage from provider responses; per-node and per-run totals in trace and overview; rough cost estimate per model | Planned | *New* |
| 14 | **Project terminal & jobs** — interactive shell tab in the bottom dock; cwd follows linked project / active run execution root; background job handles for long-running commands | Done | [Project terminal](#project-terminal) |

### Tier 4 — Context & attachments

Getting the right context into and out of agents.

| # | Item | Status | Details |
| --- | --- | --- | --- |
| 15 | **Attachments & file references** — attach button, `@` token combobox, drag-drop; resolved content in submit payload and entrypoint | Planned | [Attachments](#attachments--file-references) |
| 16 | **Upstream read-file context** — read-tier ledger per node; `read_files` in downstream node input alongside `changed_files` | Planned | [Upstream read-file context](#upstream-read-file-context) |
| 17 | **Node handoff artifacts & output review** — per-node plan/md files in a canonical run dir; per-node opt-in review gate before downstream starts (Cursor-like) | Planned | [Node handoff artifacts & output review](#node-handoff-artifacts--output-review) |
| 18 | **Context used panel** — per-turn ledger of shared context, rules, skills, attachments, upstream artifacts; composer panel + per-turn attribution | Planned | [Context used](#context-used) |
| 19 | **Global chat** — unified run-wide transcript in execution-layer order; per-awaiting-node reply bubbles for parallel pauses | Done | [Global chat](#global-chat) |

### Tier 5 — Power features

| # | Item | Status | Details |
| --- | --- | --- | --- |
| 20 | **Branching join semantics** — nodes wait for all upstream outputs before continuing | Planned | |
| 21 | **In-run todos** — `openflow_update_todos` builtin; run-state projection; dock/chat chrome UI | Planned | [Agent questions & todos](#agent-questions--todos) |
| 22 | **MCP integration** — settings-gated MCP servers as external tool sources on agent nodes; default external tools to prompt/exec until sandboxing exists | Planned | [MCP integration](#mcp-integration) |
| 23 | **Cron / scheduled runs + workflow retry loop** — execute the schedule/retry schema fields that already exist | Planned | |
| 24 | **Run checkpoint, history, and replay** — persist run checkpoints to disk; browse run history; resume paused runs or replay from a checkpoint (read-only trace or forked re-execution); depends on persistence policy (#6) | Planned | [Run checkpoint & replay](#run-checkpoint-history-and-replay) |
| 25 | **Programmatic / non-AI nodes** — API-call and transform nodes between agent nodes | Planned | |
| 26 | **External connectors** — Composio / n8n-style integration nodes | Planned | |

### Tier 6 — Polish & distribution

| # | Item | Status | Details |
| --- | --- | --- | --- |
| 27 | **Canvas editing QoL** — undo/redo for graph edits, duplicate node, copy/paste between workflows | Planned | *New* |
| 28 | **Accessibility & keyboard shortcuts** — panel toggles, focus management, shortcut reference overlay | Planned | [Accessibility](#accessibility) |
| 29 | **Onboarding & templates** — first-run empty state, 2–3 bundled example workflows, "new from template" | Planned | *New* |
| 30 | **macOS distribution** — code signing, notarization, auto-update (Tauri updater); bundle already builds | Planned | *New (expands packaging)* |
| 31 | **Serde casing unification** — T16 (one wire convention) then T16b (drop legacy aliases/shims) | Planned | [Phase 4](#phase-4--cleanup) |
| 32 | **Cleanup pass** — T13–T15, T18 (clippy `-D warnings`), refactor polish: slim `AppProvider`, typed desktop DTOs, store catalog audit, provider module audit | Planned | [Refactor](#refactor) |

### Dev & agent tooling

| # | Item | Status | Details |
| --- | --- | --- | --- |
| 33 | **Interactive plan review tool** — standalone HTML+JS for markdown plan review | Done | [Plan review tool](#interactive-plan-review-tool) |

### Backlog (unsequenced)

Small or speculative items — pick up opportunistically or when a tier item touches the same code:

- Remove `Context:` / `Task:` labels from chat (upstream `Context:` blocks no longer projected on pause; model prompt labels may still appear in trace)
- Skill discovery settings — unified skills section in Settings (currently scans Cursor/Codex/Claude roots)
- Remove per-node JSON output schema editing (keep internal defaults)
- System-level notifications (run complete, agent question while unfocused)
- Sidebar search across workflows and agents
- Warn on close: unsaved changes / active run
- Per-workflow path allowlist beyond execution-cwd jail
- Git stage/commit helpers from changed-files panel
- Full LSP language-server client
- Error logging stored locally (**backend slice Done**; UI + agent auto-fix loop follow-up) — [persistent error reporting plan](superpowers/plans/2026-06-15-persistent-error-reporting.md)
- Workflow version control (per-change revert)
- Natural language workflow definition
- T7 node-local max-tool-rounds (only if D4 changes), T17 concurrent layer siblings in headless runner (stretch)

**Deferred** until cron, retry loops, and repo workflows land: background job start/stop/resume, multi-run orchestration.

---

## Detailed specs

### Interactive plan review tool

Standalone browser tool for reviewing implementation plans before execution — no build step, no server.

| Path | Role |
| --- | --- |
| `tools/plan-review.html` | Single-file HTML+JS app — open in any browser |

| Capability | Status |
| --- | --- |
| Load `.md` / paste / drag-drop plan text | Done |
| Render markdown (headings, lists, code, tables, TOC) | Done |
| Select text → inline comment with quote anchor | Done |
| Comment sidebar — jump to highlight, delete | Done |
| Export review as markdown (comments + source) | Done |
| Session persistence via `localStorage` | Done |
| Verdict chips — plan-level and per-comment (`approve` / `block` / `question`) | Done |
| Threaded replies on comments | Done |
| Import exported review — merge comments; line diff when plan text differs | Done |

**Usage:**

```bash
open tools/plan-review.html
```

**Target:** Reviewers (human or LLM handoff) load a plan spec, attach anchored comments on specific passages, set verdicts, thread replies, and export a single markdown file the implementing agent can act on — without editing the plan in-repo. Re-import an exported review to merge feedback or apply an updated plan with a line diff preview.

### Wire entrypoint text through the desktop run path

Domain supports entrypoint injection (`run_with_entrypoint` → `InteractiveEngine` → `build_node_input`). Desktop IPC and UI now pass optional entrypoint text through `start_run`; idle global chat composer can kick off a run with the same text.

| Layer | Status |
| --- | --- |
| `crates/desktop/src/lib.rs` | `start_run` accepts and forwards `entrypoint` |
| `crates/ui/src/api.ts` | `startRun` accepts optional `entrypoint` |
| `crates/orchestration/src/backend/mod.rs` | `start_run` forwards entrypoint to coordinator |

**Target:** User entrypoint text from the UI reaches root agent nodes as `{ "entrypoint": { "text": "..." }, "upstream": [] }` in `AgentRequest.input`.

**Reference:** Domain test `injects_entrypoint_into_root_node_input_only` in `crates/engine/src/execution/workflow_runner.rs`.

### Run lifecycle

| Item | Priority | Status |
| --- | --- | --- |
| `stop_run` command — abort execution handle, clear channels, mark run inactive | High | Done |
| Wire stop/cancel to UI (stop button during active runs) | High | Done |
| Handle window close (`CloseRequested`) — abort active run before exit | High | Done |
| Graceful shutdown — cancel in-flight AI calls and tool subprocesses on close | Medium | Done |
| User stop shows `stopped` on canvas, overview, and trace (not `failed`) | Medium | Done |
| Unify on one Tokio runtime — `AppBackend` takes injected `Handle` | Medium | Done |
| In-session checkpoint on user stop — snapshot engine state for same-session **Continue** | High | Done |
| `continue_run` / `is_run_continuable` IPC + header Continue button | High | Done |
| Stale checkpoint validation — reject resume when workflow node ids no longer match | High | Done |
| Clean up temp artifact dirs (`openflow-run-*`) on completion or abort | Medium | Planned |
| Store event bridge task handle for independent cancellation | Medium | Planned |
| Decide and document checkpoint/persistence policy — in-memory only, auto-checkpoint on pause/layer, manual checkpoint, resume after restart | Medium | Planned |
| Durable run artifact layout — move tool spill files and handoff outputs under the chosen run record before replay/history lands | High | Planned |
| Auto-checkpoint on run pause (`AwaitInput`, `AwaitToolApproval`) and layer completion | Medium | Planned |
| Persist artifact dir refs alongside checkpoint (not only ephemeral `openflow-run-*`) | Medium | Planned |
| Warn on close when workflows have unsaved changes | Low | Planned |
| Warn on close when a run is still active | Low | Planned |
| Offer to save checkpoint on close when a run is paused or in progress | Low | Planned |

### Run checkpoint, history, and replay

Runs today live entirely in memory: `RunCoordinator` holds `WorkflowRunState`; `InteractiveEngine` holds transcripts, scheduling, and pause state; tool artifacts sit under ephemeral `openflow-run-*` temp dirs. Closing the app or losing the process drops all of it. `WorkflowRunSnapshot` (headless path) captures projection fields after a finished run but is not persisted or reloadable. There is no run history, no resume, and no replay from an earlier point.

| Layer | Gap |
| --- | --- |
| `crates/engine/src/execution/interactive_engine.rs` | Engine state (transcripts, completed nodes, pending retries, layer cursor) is not serializable; no `restore_from_checkpoint` |
| `crates/orchestration/src/run/coordinator.rs` | `RunSession` is in-memory only; no run store; `start_run` always creates a fresh engine |
| `crates/orchestration/src/run/state/` | `WorkflowRunState` is a UI projection — sufficient for browse/replay UI but not alone for resume |
| `crates/orchestration/src/run/execution/drive.rs` | No checkpoint hook on pause/completion; artifact root is always a new temp dir |
| `crates/orchestration/src/adapters/storage/` | No `RunCheckpointStore` — no `{project}/.flow/runs/` (or app-data) layout |
| `crates/desktop/src/lib.rs` | No IPC for list/load/resume/replay runs |
| `crates/ui/src/` | No run history panel; no "resume" or "replay from here" affordances |

**Decisions (resolve before coding — ties to #6):**

| ID | Question | Recommendation |
| --- | --- | --- |
| R1 | What is the unit of persistence? | One **run record** per execution attempt; **checkpoints** are append-only snapshots inside that record |
| R2 | Where do runs live? | Project-scoped: `{project}/.flow/runs/{run_id}/`; app-only workflows use app data dir mirror |
| R3 | What does a checkpoint contain? | **Minimum resume set:** workflow id + content hash at run start, entrypoint, execution cwd, serialized `InteractiveEngine` snapshot, `WorkflowRunState`, artifact manifest (paths under run dir). **Browse-only** checkpoints may omit engine snapshot and store event log + projection only |
| R4 | When to auto-checkpoint? | On each engine pause (`AwaitInput`, `AwaitToolApproval`), on layer completion, and on terminal outcome; optional debounced checkpoint during long `CallAi` streams |
| R5 | Replay vs resume? | **Resume** continues the same run id from latest checkpoint (reconstruct engine + drive). **Replay (read-only)** renders stored trace/chat without invoking providers. **Replay (fork)** starts a new run id seeded from a checkpoint (copy artifacts + engine snapshot); user edits workflow before fork if desired |
| R6 | Provider re-invocation on fork? | Default fork **re-executes** from checkpoint cursor (calls providers again). Optional later: "trace replay" mode that never calls AI (depends on storing enough transcript to simulate turns) |

| Item | Priority | Status |
| --- | --- | --- |
| Persistence policy ADR — document R1–R6; choose resume-after-restart vs history-only for v1 | High | Planned |
| Run record schema — `run.json` metadata + `checkpoints/{seq}.json` + `artifacts/` per run dir | High | Planned |
| `InteractiveEngine` snapshot — serialize/deserialize resume-critical fields; round-trip tests | High | Planned |
| `RunCheckpointStore` — create run, append checkpoint, list by workflow/project, load latest | High | Planned |
| Auto-checkpoint in drive loop — on pause, layer done, completed/failed/stopped | High | Planned |
| Manual checkpoint — IPC + UI action during active run | Medium | Planned |
| Resume run — `resume_run(run_id)` reconstructs engine, artifacts, and drive from latest checkpoint | High | Planned |
| Run history UI — list past runs for workflow/project; status, started/finished, checkpoint count | High | Planned |
| Read-only replay — open completed run: trace, chat, outputs, changed files without execution | High | Planned |
| Fork from checkpoint — `replay_run(checkpoint_id)` starts new run seeded from snapshot; optional workflow diff warning | Medium | Planned |
| Replay from node — fork from checkpoint taken after a specific node completed (extends `retry_node` with persistence) | Medium | Planned |
| App restart — detect incomplete run records on launch; offer resume or discard | Medium | Planned |
| Prune/retention — max runs per workflow, max checkpoint depth, delete run dir on discard | Low | Planned |
| Export run — zip run dir for sharing or CI artifacts | Low | Planned |

**Target:** Every meaningful pause and layer boundary writes a durable checkpoint under the linked project. You can close the app, reopen, and resume a paused run. Completed and in-progress runs appear in history. Open any past run to inspect trace and chat read-only, or fork a new run from an earlier checkpoint without re-entering context by hand.

**Depends on:** #6 (persistence policy). **Unlocks:** #22 (scheduled runs need durable run records), #17 (handoff artifact paths), deferred multi-run orchestration, audit/compliance use cases.

**Reference:** Live projection — `WorkflowRunState` in `crates/orchestration/src/run/state/mod.rs`; headless snapshot — `WorkflowRunSnapshot` in `crates/orchestration/src/run/execution/mod.rs`; artifact temp dirs — `drive.rs` (`openflow-run-{uuid}`).

### Provider API key storage

| Item | Priority | Status |
| --- | --- | --- |
| Persist keys in `settings.json` (`ProviderProfile.api_key`) | High | Done |
| Settings UI plaintext risk notice | High | Done |
| Env var fallback unchanged | High | Done |
| macOS Keychain storage — keys out of plaintext; migrate on first launch (#7) | High | Planned |

### Tool invocation retry and resilience

Today a failed tool call becomes a single `is_error: true` [`ToolResult`](crates/domain/src/tools/config.rs) fed back to the model. [`retry_policy`](crates/domain/src/graph/workflow.rs) (T6) applies only to transient **AI** [`AgentError`](crates/domain/src/ports/outbound.rs), not tool-runner failures. The drive loop can still **exit the run** on orchestration/engine mismatches (`on_tool_results` error → `ExecutionEvent::Error`) or on AI invoke failure after retries.

| Layer | Gap |
| --- | --- |
| `crates/orchestration/src/tools/runner.rs` | `ToolRunnerError` has no transient/permanent classification; no retry/backoff | **Partial** — `ToolError::is_retryable()` in `tool/errors.rs`; runner retry loop still missing |
| `crates/orchestration/src/execution/drive.rs` | Tool execute fails once → `denied()` result; no retry loop; handler errors abort drive |
| `crates/domain/src/graph/workflow.rs` | `RetryPolicy` is AI-oriented; no tool-specific retry knobs |
| `crates/domain/src/execution/interactive_engine.rs` | Tool errors in transcript do not increment AI retry counters; run should continue |

**Target behavior:**

1. Classify tool failures as retryable (timeout, rate limit, transient I/O) vs permanent (bad args, missing file, policy deny). **Done** — `ToolError::is_retryable()` in `orchestration/src/tool/errors.rs`.
2. Retry retryable tool invocations per workflow/node policy (`max_attempts`, `backoff_ms`) **before** surfacing an error result to the model. **Planned** — no retry loop in `drive.rs` yet.
3. On exhausted retries or permanent failure, append `is_error: true` tool result and **resume the agent loop** (`CallAi`) — do not terminate the run or crash the host. **Planned**
4. Reserve run-level failure for unrecoverable host errors (engine state corruption, cancelled run), not individual tool calls. **Planned**

**oh-my-pi imports to keep:**

| Pattern | Step-through shape |
| --- | --- |
| Catch all expected tool failures | `ToolRunnerError` becomes a structured `ToolResult { is_error: true }` whenever the engine can continue |
| Tool retry classification | Keep `ToolError::is_retryable`; add retry/backoff in orchestration before `ToolCompleted` error projection |
| Before/after tool hooks | Done: orchestration-side hook seam exists around `ToolRunner::execute`; approval/audit/guard hooks can now be registered without changing individual tools |
| Abort/cancel distinction | Preserve user stop / node interrupt as cancellation, not model-visible tool failure |
| Structured output metadata | Never truncate silently; preserve `ToolOutputMeta` and artifact references in run history |

**Depends on:** T1 (error taxonomy pattern), T6 (retry policy wiring). See T19–T21 in domain hardening.

### Chat presentation — thinking bubbles & tool cleanup

Assistant token streaming is wired (`ChatMessageDelta` → chat log). Next chat polish: show provider reasoning as first-class thinking bubbles and replace always-expanded tool panes with compact, expandable rows.

| Item | Priority | Status |
| --- | --- | --- |
| Collapsible tool bubbles — collapsed row shows tool name + one-line outcome; expand for args and full output | High | Done |
| Thinking bubble UI — collapsible reasoning block in chat; distinct from assistant messages; collapsed by default | High | Done |
| Provider thinking in transcript — parse reasoning blocks from Anthropic/OpenAI responses; project to chat (not legacy `ChatRole::Thinking` tool lines) | High | Done |
| Tool intent field — add optional `_i` / `intent` text to tool-call schema; show it as the collapsed tool-row summary when present | High | Done |
| Pretty tool names — human-readable labels in chat (e.g. Read, Search, Edit file) instead of raw builtin ids (`read`, `ast_grep`, `openflow_call_subagent`) | Medium | Planned |
| Tool row chrome — drop `Tool Invocation:` header; status chip (running / completed / failed); chevron expand | Medium | Done |
| Args summary — one-line path/query preview when collapsed; full formatted JSON only when expanded | Medium | Done |
| Live tool updates — emit `ToolUpdated` / tail events for long-running tools; stream current tail into the expanded row while preserving final full output or artifact | High | Done |
| Streaming thinking — append reasoning tokens into the thinking bubble during active turns | Medium | Done |

**Reference:** [`ToolBubble.tsx`](crates/ui/src/components/conversation/ToolBubble.tsx); full spec in [Thinking & chat presentation](#thinking--chat-presentation).

### Canvas run feedback

During a run, agent nodes show a status row and optional subagent rows. Subagents are capped at three visible entries with a `+N more` overflow line; status is a colored dot plus text label (`WorkflowNode.react.tsx`, `agentStatus.ts`).

| Layer | Gap |
| --- | --- |
| `crates/ui/src/canvas/WorkflowNode.react.tsx` | `MAX_VISIBLE_SUBAGENTS = 3` truncates the list; no scroll container |
| `crates/ui/src/styles/index.css` | `.node-subagent-list` is static; no max-height / overflow-y |
| `crates/ui/src/canvas/WorkflowNode.react.tsx` | Status is dot + text only — no distinct icon per `AgentStatus` |
| `crates/ui/src/lib/agentStatus.ts` | Labels only; no icon or color token mapping for canvas chrome |
| `crates/ui/src/components/conversation/ConversationMessages.tsx` | Filter chips use `status-${segment.status}` on dots but `.chat-filter-status-dot` forces `var(--text-muted)` |
| `crates/ui/src/components/conversation/ChatPanel.tsx` | Live-node picker chips share the same gray-dot override |
| `crates/ui/src/styles/index.css` | Canvas `.status-*` palette exists; chat chip dots do not inherit it |

| Item | Priority | Status |
| --- | --- | --- |
| Chat status color parity — filter chips and live-node picker dots use the same `.status-*` colors as canvas nodes | High | Planned |
| Shared status tokens — single CSS variable or `agentStatus` color map consumed by canvas, handles, and chat chrome | Medium | Planned |
| Scrollable subagent list — show all in-run subagents inside the node; max-height + `overflow-y: auto`; drop `+N more` truncation | High | Planned |
| Subagent row polish — keep status dot + name; optional purpose tooltip; readable at small node widths | Medium | Planned |
| Status icons — replace or augment the dot with a distinct colored icon per state (thinking, waiting for input, awaiting approval, running tool, done, failed, stopped) | High | Planned |
| Icon + label pairing — icon at a glance; text label on hover or when node is selected / zoomed in | Medium | Planned |
| Handle chrome — match icon color on left/right handles for quick scan across the graph | Low | Planned |

**Target:** Glance at the canvas and tell what each node is doing from icon color and shape. Open a busy agent node and scroll its full subagent roster without losing entries behind a `+N more` line.

### Project terminal

Interactive shell in the bottom dock — a fourth tab beside Overview, Chat, and Run trace. Lets you run commands in the linked project without switching to an external terminal. Complements (does not replace) the agent `bash` tool and live bash output in chat.

| Layer | Status |
| --- | --- |
| `crates/ui/src/panels/DockPanel.tsx` | Terminal tab + `TerminalPanel` host |
| `crates/ui/src/context/` | Terminal session state, output buffer, lifecycle handlers |
| `crates/desktop/src/lib.rs` | PTY spawn, resize, write, output-stream IPC |
| `crates/orchestration/src/terminal/` | `TerminalManager` with cwd resolution |

| Item | Priority | Status |
| --- | --- | --- |
| Terminal tab — add **Terminal** to dock tab switcher; persist selected tab in session | High | Done |
| PTY backend — spawn login shell via `portable-pty`; stream stdout/stderr to UI; handle resize | High | Done |
| Cwd policy — default to linked project root; during an active run, optionally follow execution cwd; `cd` in terminal is session-local | High | Done |
| xterm.js frontend — fit-to-panel, scrollback, copy/paste, basic ANSI colors | High | Done |
| Job manager — long-running terminal/agent bash commands can return a job id; status/output/stop actions are available without blocking the run harness | High | Planned |
| Job output retention — job output uses the same durable run artifact store and truncation metadata as tool results | Medium | Planned |
| Lifecycle — one terminal session per editor window; kill PTY on app close; warn if shell still running (ties to warn-on-close backlog) | Medium | Done |
| New terminal / split — restart shell or open a second tab (v2) | Low | Planned |
| Inject command from chat — "Run in terminal" on bash tool rows (optional; depends on live bash output) | Low | Planned |

**Target:** Open the bottom dock → Terminal → get a project-scoped shell immediately. Run `cargo test`, `git status`, or `./scripts/verify.sh` while a workflow run is paused or in progress. Cwd matches where agents execute when a run is active.

**Not in v1:** Remote SSH shells, root/sudo elevation UI, or replaying agent bash invocations as read-only panes (chat tool rows remain the audit trail).

**Reference:** Dock tabs — [`DockPanel.tsx`](crates/ui/src/panels/DockPanel.tsx); execution cwd — `orchestration/src/run/execution/`; bash tool — [`bash.rs`](crates/orchestration/src/adapters/tool_impl/bash.rs).

### MCP integration

External tools should enter the harness through the same tool contract as builtins, not through ad-hoc provider-specific paths. Use oh-my-pi's adapter and discovery ideas, but avoid dynamic code loading until there is a sandbox story.

| Layer | Gap |
| --- | --- |
| `crates/orchestration/src/tool/registry.rs` | Builtin-only registry; no external source metadata or MCP-backed definitions |
| `crates/orchestration/src/settings/` | No MCP server config, discovery mode, or per-server enable/disable |
| `crates/engine/src/tools/config.rs` | Node tool catalog can name tools, but has no source namespace or collision policy |
| `crates/orchestration/src/run/execution/tool_port.rs` | Executes builtins/subagents only; no MCP dispatch adapter |
| `crates/ui/src/screens/SettingsScreen.tsx` | No MCP server setup, health, or discovered-tool list |

| Item | Priority | Status |
| --- | --- | --- |
| MCP server config — add settings for command/HTTP servers, enabled state, and discovery mode (`off`, `mcp-only`, `all`) | High | Planned |
| Tool adapter — convert MCP tool schemas into `ToolDefinition`s and execute through `ToolRunner` / `ToolPort` result normalization | High | Planned |
| Source namespacing — prevent collisions with builtins; show `mcp:<server>/<tool>` or equivalent in catalogs and trace | High | Planned |
| Conservative approval — default MCP tools to `exec` or prompt until explicitly overridden by node/tool policy | High | Planned |
| Discovery UI — settings page lists servers, health, and available tools; node inspector can opt tools in | Medium | Planned |
| Optional tool search — later BM25 discovery over available external tools; do not make discovery a required path for normal use | Low | Planned |
| No dynamic extension loading in v1 — MCP process/HTTP transport only; custom code loading requires sandbox and audit design first | High | Planned |

**Target:** Add MCP tools as first-class external tool definitions with explicit source, approval, and execution boundaries. Builtins remain stable and cannot be shadowed silently.

### Accessibility

Keyboard QoL exists for run, save, delete, and zoom (`AppProvider` global handler; see README). Panel chrome (sidebar hide, dock max/collapse) is mouse/drag only.

| Item | Priority | Status |
| --- | --- | --- |
| Sidebar hide/show — macOS hide control + keyboard shortcut | Medium | Planned |
| Bottom dock — maximize height / collapse + keyboard shortcut | Medium | Planned |
| Inspector and workflow settings — toggle via keyboard | Low | Planned |
| Shortcut reference — Settings panel or `?` overlay | Low | Done |
| Focus management — modals, dock tabs, sidebar nav | Medium | Planned |
| Canvas and run status — screen-reader labels | Low | Planned |

**Target:** Every primary panel toggle (sidebar, dock, inspector) has a documented shortcut; shortcuts skip when focus is in a text field.

### Thinking & chat presentation

Providers expose extended reasoning (Anthropic thinking blocks, OpenAI reasoning effort, etc.), but the app has no per-node knob and no first-class UI for model reasoning. `ChatRole::Thinking` today is reused for legacy tool-line parsing and pause context — not provider reasoning. Tool bubbles always show full output in a fixed-height scroll region.

| Layer | Gap |
| --- | --- |
| `crates/engine/src/graph/workflow.rs` | No `thinking_level` (or budget) on `AgentNodeConfig` / `CallableAgent` or `WorkflowSettings` default |
| `crates/engine/src/ports/outbound.rs` | `AgentRequest` has no thinking/reasoning field for adapters |
| `crates/providers/src/` | Wire payloads omit provider-specific reasoning params; responses do not parse thinking blocks into transcript items |
| `crates/engine/src/conversation/mod.rs` | No dedicated transcript item for provider reasoning (distinct from `ChatRole::Thinking` log lines) |
| `crates/orchestration/src/execution/events.rs` | Run projection does not emit structured thinking events to chat |
| `crates/ui/src/forms/` | Inspector has no thinking-level control (off / low / medium / high or provider-aligned presets) |
| `crates/ui/src/components/conversation/` | No collapsible thinking block component; `PlainMessage` renders thinking role like assistant text |
| `crates/ui/src/components/conversation/ToolBubble.tsx` | Always expanded fixed-height scroll pane; `Tool Invocation:` header; raw builtin ids (`read`, `openflow_call_subagent`) with no display-name mapping |
| `crates/engine/src/tools/config.rs` / provider mapping | Tool calls have raw args only; no optional model-supplied intent field for user-readable "why this tool is running" copy |
| `crates/orchestration/src/run/execution/events.rs` | No live tool-update event for incremental bash/eval/job output; UI only sees start/completion |
| `crates/ui/src/components/conversation/ConversationMessages.tsx` | No `ThinkingBubble`; tool markers and legacy thinking lines share the same bubble path |
| `crates/ui/src/lib/parseLegacyToolMessages.ts` | Legacy `ChatRole::Thinking` grouped as tool bubbles — conflates provider reasoning with tool I/O |

| Item | Priority | Status |
| --- | --- | --- |
| Thinking level schema — `reasoning_effort` + `reasoning_budget_tokens` on agent node + saved agent | High | Done |
| Provider default — pick default reasoning effort in Settings → Reasoning (applied at run start when node unset) | High | Done |
| Workflow settings control — pick default thinking level in gear panel (off / low / medium / high or provider-aligned presets) | High | Planned |
| Inspector control — pick thinking level per node; inherit provider default when unset | High | Done |
| Provider wiring — map level to Anthropic/OpenAI-compat reasoning params; parse thinking blocks from responses | High | Done |
| Thinking transcript items — stream reasoning into chat as `ThinkingBubble` rows (distinct from legacy `ChatRole::Thinking` tool lines) | High | Done |
| Collapsible tool bubbles — collapsed row shows tool name + one-line outcome; expand for args and full output | High | Done |
| Tool intent field — support optional `_i` / `intent` in tool-call args; collapsed tool row prefers intent over raw-arg summaries | High | Done |
| Live tool updates — add `ToolUpdated` event and tail-buffer UI for bash/eval/job output while a tool is still running | High | Done |
| Pretty tool names — map builtin/subagent ids to short human labels in `ToolBubble`, `ToolApprovalCard`, and trace rows | Medium | Planned |
| Tool row chrome — icon + name + status chip; remove `Tool Invocation:` label; chevron toggle | Medium | Done |
| Args one-liner — path/query/file summary when collapsed; `prettyJson` args only when expanded | Medium | Done |
| Streaming thinking — append reasoning tokens into the thinking bubble during active turns | Medium | Done |
| Hide legacy thinking tool lines — stop using `ChatRole::Thinking` for tool request/result prose once structured bubbles land | Medium | Planned |
| Per-run thinking override — transient level tweak from chat chrome without editing the workflow | Low | Planned |

**Target:** Users choose how much model reasoning each node uses. Provider thinking appears as collapsible blocks in chat. Tool invocations show a compact “what it did” line until expanded — no always-on scroll panes or raw-args dumps in the default view.

### Agent questions & todos

Agents can already ask for free-text input via `openflow_request_user_input` (`AgentNeedUserInput` → `AwaitInput` → chat composer when `awaitingNodeId` matches). There is no structured question UI, no todo model, and no way to send input while a node is still running.

| Layer | Gap |
| --- | --- |
| `crates/providers/src/mapping.rs` | `request_input_tool` accepts one string only; no options or question id |
| `crates/domain/src/execution/interactive_engine.rs` | No in-run todo state; questions resume as plain user messages |
| `crates/orchestration/src/run_coordinator.rs` | `submit_user_input` rejects unless `awaiting_node_id` matches |
| `crates/orchestration/src/execution/drive.rs` | `ProvideInput` ignored during tool approval; no input buffer |
| `crates/orchestration/src/state.rs` | Run state has no todo or pending-question projection; no input queue |
| `crates/ui/src/components/conversation/` | Composer disabled unless node is awaiting; no queued-message UI |

| Item | Priority | Status |
| --- | --- | --- |
| Input queue — accept chat while node is active; buffer per node in run state | High | Planned |
| Drain queue on `AwaitInput` — deliver oldest-first when agent requests input | High | Planned |
| Queued input UI — show pending messages in composer; allow edit/remove before delivery | Medium | Planned |
| Structured questions — option cards / multiple-choice in chat | High | Planned |
| Question builtin — extend or replace `openflow_request_user_input` with options, allow-multiple, question id | High | Planned |
| In-run todo list — agent-managed tasks visible in dock or chat chrome | Medium | Planned |
| Todo builtin — `openflow_update_todos` internal tool + run-state projection to UI | Medium | Planned |
| Notify when an agent asks a question while user is on another node | Medium | Planned |
| Persist todos per workflow run; optional export under project `.flow/` | Low | Planned |

**Target:** Users can type ahead during active runs; queued input drains when the agent pauses. Structured questions and todos render in-run and sync back to the model each turn.

### Global chat

Today the dock Chat tab shows only the **selected** node's `chatLogs` entry (`AppProvider.chatMessages` keys off `selectedNodeId`). Advancing the workflow or selecting another node swaps the transcript; prior node conversation disappears from view unless you re-select that node. Parallel siblings at the same execution layer each have their own log, but the UI exposes one node at a time.

| Layer | Gap |
| --- | --- |
| `crates/ui/src/context/AppProvider.tsx` | `chatMessages` is per selected node; no merged run-wide transcript |
| `crates/ui/src/components/conversation/ConversationMessages.tsx` | Renders a single node's log; no node header or layer ordering |
| `crates/ui/src/components/conversation/ConversationComposer.tsx` | Composer targets selected node only; no per-awaiting-node reply affordance |
| `crates/orchestration/src/run/state/` | `chatLogs` is `Record<NodeId, ChatMessage[]>`; no global projection or execution-layer index |
| `crates/ui/src/context/AppProvider.tsx` | `chatEnabledMemo` requires selected node ∈ `awaitingNodeIds` — global pane cannot accept input for a sibling without selecting it |

| Item | Priority | Status |
| --- | --- | --- |
| Unified transcript — merge all node `chatLogs` into one scrollable pane for the active run | High | Done |
| Persist on progression — keep showing prior nodes' messages as the run advances; do not clear or hide when focus moves | High | Done |
| Execution-layer ordering — stack messages by DAG depth: earlier layer on top, later layer below (node 1 text above node 2, etc.) | High | Done |
| Node attribution — label or chrome per segment so users know which agent spoke | Medium | Done |
| Parallel reply bubbles — when two+ nodes at the same layer await input, show separate composer targets (one bubble per awaiting node) | High | Done |
| Route submit to correct node — `submit_user_input` keyed by target node id from the reply bubble, not canvas selection | High | Done |
| Optional canvas sync — selecting a node scrolls global chat to that node's segment (highlight only; pane stays unified) | Low | Done |
| Run start / entrypoint — global pane shows entrypoint user message at top before first node output | Medium | Done (segment headers) |
| Start run from idle composer — message the workflow composer before a run starts; header **Run** still starts without entrypoint | High | Done |

**Target:** One continuous chat pane for the whole run. As nodes complete and downstream nodes start, earlier conversation stays visible in layer order. When parallel nodes at the same depth pause for input, each gets its own reply bubble in the composer area so you can answer both without switching canvas selection.

**Shipped design:** Settled history (layer-ordered, non-live nodes) scrolls above a live strip of side-by-side columns (tmux-style). Each live column has its own transcript scroll, approval card, and composer. Overflow columns collapse into tabs in the last slot. Filter chips above settled history narrow by node without affecting canvas selection. Projection is UI-only (`projectChatLayout` in `crates/ui/src/lib/workflow.ts`); per-node `chatLogs` remain the backend source of truth.

**Gap — chat node bar status colors:** ~~Filter chips and live-node picker forced gray via `.chat-filter-status-dot { background: var(--text-muted) }`.~~ **Fixed** — dot class scopes size only; `.status-*` palette applies. Remaining polish: segment header pills and dark-mode contrast check (see table below).

| Item | Priority | Status |
| --- | --- | --- |
| Fix dot override — scope `.chat-filter-status-dot` size only; let `.status-*` set background (or use `.chat-filter-status-dot.status-queued`, etc.) | High | Done |
| Segment header pills — optional status tint on `.chat-segment-status` / `.chat-live-status-pill` for the active live column | Low | Planned |
| Dark mode check — verify contrast for all status hues on `--raised-surface` chip backgrounds | Medium | Planned |

**Target:** Node list in the chat bar shows the same status colors as the canvas — blue for thinking, amber for awaiting input, teal for running tool, green for done, red for failed, etc. — without re-selecting the node on the graph.

### Node completion

A workflow node is **incomplete** until the agent calls `openflow_submit_node_output` once with schema-conforming JSON. Plain assistant text does not finish the node or advance downstream scheduling. On success the host stores output, updates run projection, optionally surfaces a chat summary, and schedules the next execution layer when all siblings in the current layer have submitted.

| Layer | Role |
| --- | --- |
| `crates/engine/src/execution/node_invocation.rs` | `NODE_RUNTIME_PREAMBLE` — submit contract, schema placement, pause vs finish |
| `crates/providers/src/mapping.rs` | Parse submit tool args; `jsonrepair-rs`; auto-wrap flat fields under `output` |
| `crates/engine/src/execution/interactive_engine/completion.rs` | `on_ai_complete` → store output, malformed-submit retry, layer advance |
| `crates/engine/src/execution/interactive_engine/mod.rs` | `missing_upstream_outputs` — fail fast before scheduling downstream |
| `crates/orchestration/src/run/execution/events.rs` | `NodeCompleted` reducer — status, trace, chat summary bubble |
| `crates/ui/src/components/conversation/NodeCompletedBubble.tsx` | Dedicated “Node completed” row with success cue |

**Acceptance criteria**

| ID | Criterion | Verify | Status |
| --- | --- | --- | --- |
| NC-1 | Node stays incomplete until `openflow_submit_node_output` succeeds; assistant prose alone does not advance the layer | Engine poll never schedules downstream until `outputs` contains the node; preamble states contract | Done |
| NC-2 | Submit args validated against node `output_schema`; invalid shape → correction user message and retry (max 3) before node failure | `malformed_submit_output_retries_then_succeeds`; live provider mapping tests | Done |
| NC-3 | Flat schema fields auto-wrapped under `output` when the model omits the wrapper | `normalize_submit_output_arguments` unit tests in `mapping.rs` | Done |
| NC-4 | Successful submit stores JSON in engine `outputs`; direct downstream `AgentRequest.input.upstream` lists `{node_id, output}` sorted by node id | `build_node_input` tests; acceptance workflows | Done |
| NC-5 | Downstream node fails fast with `MissingUpstreamOutput` when any direct upstream lacks output — no silent empty upstream | Engine scheduling tests; error message lists missing node ids | Done |
| NC-6 | `ExecutionEvent::NodeCompleted` sets canvas status `completed`, records full output in `outputs` and run trace | `reducer_node_completed_*` in `execution/tests.rs` | Done |
| NC-7 | Chat log gets `messageKind: node_completed` with **summary text only** when output contains non-empty `summary`; skip chat row when `summary` absent | `reducer_node_completed_pushes_summary_completion_message`; `NodeCompletedBubble` | Done |
| NC-8 | `<tool_call>` / fenced tool markup stripped from assistant messages on stream finalize and submit path; leading human text preserved | `reducer_stream_finalize_strips_echoed_tool_call_markup`; `filter_tool_turn_assistant_message` tests | Done |
| NC-9 | Optional `assistant_message` on submit appended to transcript after markup strip; raw submit tool invocation never echoed as chat | `apply_completion` + provider adapter tests | Done |
| NC-10 | Overview and run trace show full structured output JSON for completed nodes (not chat-truncated) | `DockPanel` overview + trace detail `prettyJson(entry.output)` | Done |
| NC-11 | `on_ai_complete` rejects completions for a node not in `in_flight_ai` (T10) — terminal `MisroutedCompletion`, no output stored | Add engine test; wire in acceptance suite | Done |
| NC-12 | Misrouted completion validates node id exists in workflow graph before terminal error (ENG-1 audit) | `reject_misrouted_completion` + graph membership check | Done |
| NC-13 | Canvas status icon distinct for completed state (see [#8 Canvas run feedback](#canvas-run-feedback)) | Visual QA after status icons land | Planned |
| NC-14 | Auto-checkpoint includes node output on layer completion (see [#6 Run lifecycle](#run-lifecycle), [#24 Run checkpoint & replay](#run-checkpoint-history-and-replay)) | Checkpoint round-trip with `outputs` map | Planned |

**Target:** Agents finish nodes only via submit-output; users see a concise summary in chat, full JSON in trace/overview, and downstream nodes start only on valid upstream output — never on partial prose or missing parents.

**Reference:** Submit contract — [`node_invocation.rs`](crates/engine/src/execution/node_invocation.rs) (`NODE_RUNTIME_PREAMBLE`); completion path — [`completion.rs`](crates/engine/src/execution/interactive_engine/completion.rs); run projection — [`events.rs`](crates/orchestration/src/run/execution/events.rs); technical overview § “When a node is done” — [`technical-overview.md`](docs/architecture/technical-overview.md).

### Attachments & file references

Users can invoke skills with `/skill` tokens in the chat composer (`crates/ui/src/lib/chatCommands.ts`), but there is no attach affordance for project files or media. Agents must discover files via read-tier tools instead of receiving user-selected context up front.

| Layer | Gap |
| --- | --- |
| `crates/ui/src/lib/chatCommands.ts` | Resolves `/` skill tokens only; no `@` path tokens or referenced-file list |
| `crates/ui/src/components/conversation/` | No attach button, file picker combobox, reference pills, drag-drop target, or content preview above composer |
| `crates/ui/src/api.ts` / `crates/desktop/src/lib.rs` | `submit_user_input` and `start_run` accept plain `text` only — no structured file refs |
| `crates/orchestration/src/run/coordinator.rs` | No read-and-resolve step for referenced paths under execution cwd |
| `crates/engine/src/execution/interactive_engine.rs` | `on_user_input` records a single string; no `referenced_files` block in transcript or node input |
| `crates/engine/src/execution/node_invocation.rs` | `entrypoint` is `{ "text": "..." }` only — no attached file payloads |

| Item | Priority | Status |
| --- | --- | --- |
| Attach button — paperclip in composer opens file picker over linked-project tree | High | Planned |
| `@` token UX — combobox over linked-project files (reuse skill combobox pattern); optional browse dialog | High | Planned |
| Drag-and-drop — drop files onto composer to attach (paths resolved under execution cwd jail) | Medium | Planned |
| Reference resolution — read file content under execution cwd on submit; reject paths outside project jail | High | Planned |
| Structured submit payload — `referenced_files: [{ path, content \| excerpt }]` alongside message text | High | Planned |
| Transcript shape — persist references in `AgentTranscriptItem::UserMessage` and chat log projection | Medium | Planned |
| Composer chrome — pills for attached paths; expandable preview (path + line range + size cap); remove via × | Medium | Planned |
| Entrypoint attachments — same reference model on run start (with entrypoint wiring) | Medium | Planned |
| Image attachments — paste or pick images; encode for vision-capable providers when model supports it | Medium | Planned |
| Line-range refs — `@path:10-40` or selection-from-editor hook | Low | Planned |
| Reference budget — max files, max bytes, truncate with notice in formatted submit text | Low | Planned |

**Target:** Attach project files via button, `@` token, or drag-drop before send (or on run start). Resolved content is injected into the user message or entrypoint JSON so the agent sees explicit file context without an extra `read` tool round. Images attach when the selected model supports vision.

### Context used

Users cannot see what context was assembled for each agent turn. `WorkflowSettings.shared_context`, project rules, skills, attachments, upstream outputs, and read-file ledgers are merged in `build_node_input` / system prompts, but only fragments appear in chat (and raw `Context:` blocks are being removed from the conversation view). There is no structured breakdown of what the model actually received.

| Layer | Gap |
| --- | --- |
| `crates/engine/src/execution/node_invocation.rs` | `AgentRequest` assembly has no `context_used` snapshot alongside `input` |
| `crates/orchestration/src/run/execution/events.rs` | Run projection does not emit per-turn context breakdown to UI or trace |
| `crates/orchestration/src/run/state/` | No `contextUsedByNode` (or equivalent) in `WorkflowRunState` |
| `crates/ui/src/components/conversation/` | No "Context used" panel on composer or per assistant turn |
| `crates/ui/src/panels/` | Inspector / overview do not preview injected context for the selected node before run |

| Item | Priority | Status |
| --- | --- | --- |
| Context ledger — record sources per `CallAi`: `shared_context`, project rules, skills, attachments, upstream, `changed_files`, `read_files` | High | Planned |
| Structured payload — `context_used: [{ kind, label, bytes?, paths? }]` on run state and optional trace row | High | Planned |
| Composer panel — collapsible "Context used" above chat input during a run; lists what the active node will send on next turn | High | Planned |
| Per-turn attribution — expandable block on assistant messages showing context snapshot for that invocation | Medium | Planned |
| Pre-run preview — inspector or overview shows merged context for selected node before `start_run` | Medium | Planned |
| Token / byte estimate — optional size per source; warn when over workflow budget | Low | Planned |
| Link to source — click path/skill/rule row to open file or settings | Low | Planned |

**Target:** Before and during a run, open "Context used" and see exactly which workflow settings, rules, skills, attachments, and upstream artifacts were injected into the active node's next (or last) model call — without dumping raw prompt text into chat.

### Project rules

Linked projects should carry agent guidance (coding standards, architecture, naming) that applies during runs — analogous to Cursor `.cursor/rules/`, but scoped to the bound repo under `.flow/`.

| Layer | Gap |
| --- | --- |
| `{project}/.flow/` | No rules file or directory convention |
| `crates/orchestration/src/project/` | Project registry does not discover or load rules |
| `crates/engine/src/graph/workflow.rs` | `WorkflowSettings.shared_context` is manual; no auto-merge from project rules |
| `crates/orchestration/src/run/application/execution/` | Run start does not inject project rules into node system prompts |
| `crates/ui/src/` | No editor or picker for project rules in linked-project settings |

| Item | Priority | Status |
| --- | --- | --- |
| Rules storage — `.flow/rules/` (or single `.flow/rules.md`) under linked project | High | Planned |
| Discovery on project load — list rules files; surface in project settings | High | Planned |
| Run injection — merge project rules into `shared_context` (or per-node system prompt) at run start | High | Planned |
| Optional enable/disable per workflow — inherit project rules by default; workflow can opt out | Medium | Planned |
| Rules editor in UI — create/edit markdown rules from linked-project panel | Medium | Planned |
| Import from `.cursor/rules/` — one-click copy or symlink convention for Cursor users | Low | Planned |

**Target:** Bind a project folder; agents automatically follow that project's rules on every run without pasting them into workflow shared context by hand.

### File edit tooling

Agents read and mutate project files under the execution cwd via builtins in `crates/orchestration/src/tool/`. Each tool has a **risk tier** (`read`, `write`, or `exec`) that drives default approval behavior when the node uses `ApprovalMode::Write` (the default).

**Tier assignment** — persisted on each `ToolRef` in the node catalog (`agent.tools.catalog.tools`). Read builtins declare `"tier": "read"` explicitly; write builtins omit `tier` and resolve to `write` via `default_tier_for_tool_name` in `crates/engine/src/tools/config.rs`:

```json
{ "name": "read", "tier": "read" },
{ "name": "search", "tier": "read" },
{ "name": "find", "tier": "read" },
{ "name": "ast_grep", "tier": "read" },
{ "name": "write" },
{ "name": "edit" },
{ "name": "apply_patch" }
```

Under `ApprovalMode::Write`, **read** tier auto-allows; **write** and **exec** tier prompt before execution. Per-tool `overrides` (`allow` / `prompt` / `deny`) and node `approval_mode` (`always_ask`, `write`, `yolo`) override the default. See `ToolTier` and `requires_approval` in `crates/engine/src/tools/config.rs`.

| Layer | Role |
| --- | --- |
| `crates/orchestration/src/tool/registry.rs` | Builtin catalog — read tier: `read`, `search`, `find`, `ast_grep`; write tier: `write`, `edit`, `apply_patch`; exec tier: `bash`. **Adding a tool:** register here and update `NODE_RUNTIME_PREAMBLE` (`engine/src/execution/node_invocation.rs`) |
| `crates/orchestration/src/tool/runner.rs` | `ToolRunner` executes builtins under execution cwd; drains `FileChangeRecord` ledger after write-tier calls |
| `crates/engine/src/tools/config.rs` | `ToolTier`, `ToolRef.tier`, `ApprovalMode`, per-call tier resolution and approval policy |
| `crates/engine/src/execution/interactive_engine.rs` | Batches tool calls; pauses on write-tier approval via `AwaitToolApproval` |
| `crates/orchestration/src/run/state/` | `changed_files` / `changedFilesByNode` ledger; `EditBatch` snapshots for revert |
| `crates/ui/src/components/conversation/` | `ToolApprovalCard` diff preview; `FileChangesPanel` per-node changed files + git diff + batch revert |

| Item | Priority | Status |
| --- | --- | --- |
| `write` / `edit` / `apply_patch` builtins — create, overwrite, hashline edit, and unified-diff patch under execution cwd | High | Done |
| Path safety — `resolve_writable` jail; reject escapes outside execution cwd | High | Done |
| Tool approval — prompt before write-tier edits (`ToolTier` + `ApprovalMode` + overrides) | High | Done |
| Changed-files ledger — track paths touched per run; surface in run state and UI | Medium | Done |
| Diff preview in chat — dry-run hunks before approve; `FileChangesPanel` diff summaries | Medium | Done |
| Pass file-change context through node outputs and downstream agents | Medium | Done |
| Git diff integration — `git_diff_file` IPC; per-file diff in `FileChangesPanel` | Low | Done |
| Undo / revert last agent edit batch per node — `revert_edit_batch` IPC | Low | Done |
| Per-workflow path allowlist (beyond execution-cwd jail) | Low | Planned |
| Git stage / commit helpers from changed-files panel | Low | Planned |
| Full LSP language-server client (format-on-write via CLI exists) | Low | Planned |

**Target:** Agents propose file edits as write-tier tool calls; user approves when policy requires; changes apply under the linked project cwd and appear in chat as reviewable diffs. Read-tier discovery tools run without approval under default `write` approval mode.

### Upstream read-file context

Downstream nodes receive upstream `output` JSON and transitive `changed_files` (write-tier mutations), but not which files upstream agents **read** via `read`, `search`, `find`, or `ast_grep`. A reviewer or implementer node must re-discover the same paths instead of inheriting gathered context.

| Layer | Gap |
| --- | --- |
| `crates/orchestration/src/tool/runner.rs` | Read-tier tool results are not recorded in a per-node ledger (only write-tier drains `FileChangeRecord`) |
| `crates/engine/src/tools/` | No `ReadFileRecord` (or equivalent) — only `FileChangeRecord` for mutations |
| `crates/engine/src/execution/interactive_engine.rs` | No `read_files_by_node` map; `record_file_changes` is write-only |
| `crates/engine/src/execution/node_invocation.rs` | `build_node_input` injects `changed_files` but no `read_files` block for transitive upstream reads |
| `crates/orchestration/src/run/state/` | Run state has no `readFilesByNode` projection for UI or trace |
| `crates/ui/src/` | No panel or trace row showing files consulted upstream of the active node |

| Item | Priority | Status |
| --- | --- | --- |
| Read-file ledger — record paths (and optional line ranges) from read-tier tool calls per node | High | Planned |
| Transitive merge — dedupe by path; latest read wins (mirror `upstream_changed_files`) | High | Planned |
| Downstream input — add `read_files` to node input JSON alongside `upstream` and `changed_files` | High | Planned |
| Snapshot policy — path-only by default; optional excerpt/hashline tag when under byte budget | Medium | Planned |
| Run state projection — `readFilesByNode` in `WorkflowRunState` + run trace entries | Medium | Planned |
| Workflow setting — opt in/out per workflow (`pass_read_files_to_downstream`, default on) | Medium | Planned |
| UI — show upstream read files in inspector or overview when a downstream node is selected | Low | Planned |
| Include read files in node `output` on submit — optional explicit list from `openflow_submit_node_output` | Low | Planned |

**Target:** When node A reads `src/foo.rs` and hands off to node B, B's `AgentRequest.input` includes those paths (and optional excerpts) so B understands what A already inspected — without repeating read-tool rounds.

**Reference:** Write-path precedent — `upstream_changed_files` + `changed_files` in `crates/engine/src/execution/node_invocation.rs`; ledger drain in `crates/orchestration/src/tool/runner.rs`.

### Node handoff artifacts & output review

Planning nodes often produce markdown plans or specs that downstream implementer nodes must read. Today handoff is only structured JSON in `openflow_submit_node_output` — agents can `write` a plan anywhere under the execution cwd, but there is no canonical path, no stable reference in upstream input, and no human review gate before the next layer starts. The standalone [plan review tool](#interactive-plan-review-tool) covers offline review; this item brings the same workflow **in-app** at node completion.

| Layer | Gap |
| --- | --- |
| `crates/engine/src/graph/` | No `handoff` config on agent nodes (primary artifact filename, optional extra paths); no `require_output_review` flag |
| `crates/engine/src/execution/interactive_engine/` | Submit success immediately stores output and may schedule downstream — no `AwaitOutputReview` pause |
| `crates/engine/src/execution/node_invocation.rs` | `build_node_input` passes `upstream[].output` only — no `handoff_files` block with canonical paths |
| `crates/orchestration/src/run/execution/drive.rs` | No handoff dir materialization under the run artifact root |
| `crates/orchestration/src/adapters/storage/` | No `{project}/.flow/runs/{run_id}/handoffs/{node_id}/` layout (ties to [#24 Run checkpoint & replay](#run-checkpoint-history-and-replay)) |
| `crates/ui/src/forms/` | Inspector has no "Require output review" toggle or primary handoff filename |
| `crates/ui/src/components/conversation/` | No in-run plan review panel (markdown render, anchored comments, approve / request changes) |

**Decisions (resolve before coding):**

| ID | Question | Recommendation |
| --- | --- | --- |
| H1 | Review scope | **Per-node opt-in** — `require_output_review` on agent node config; default off; planning nodes enable explicitly |
| H2 | Where do handoff files live? | Run-scoped under linked project: `{project}/.flow/runs/{run_id}/handoffs/{node_id}/` (app-only workflows mirror in app data dir) |
| H3 | Canonical primary file | Default `plan.md` per node; overridable in inspector (e.g. `spec.md`, `design.md`) |
| H4 | Who writes the file? | Agent via `write`/`edit` under the handoff path **or** host copies `assistant_message` / submit `output.plan_text` when present — preamble documents the convention |
| H5 | When does review run? | After valid `openflow_submit_node_output`, **before** output is committed and downstream layer schedules — new `AwaitOutputReview` engine pause (parallel to `AwaitToolApproval`) |
| H6 | Review actions | **Approve** — commit output, write handoff manifest, advance layer. **Request changes** — reject submit, inject review comments into transcript, resume agent. **Block** — mark node failed or stopped per workflow policy |
| H7 | Downstream discovery | `AgentRequest.input` adds `handoff_files: [{ node_id, primary_path, paths[] }]` per upstream node that produced artifacts — downstream preamble: "read upstream handoff at …" |

| Item | Priority | Status |
| --- | --- | --- |
| Handoff path convention — document `{run_id}/handoffs/{node_id}/plan.md` in `NODE_RUNTIME_PREAMBLE` and glossary | High | Planned |
| Node schema — `handoff.primary_file` (default `plan.md`), optional `handoff.additional_paths`; `require_output_review: bool` | High | Planned |
| Inspector controls — primary filename + "Require output review before handoff" toggle per node | High | Planned |
| Handoff materialization — on submit, ensure handoff dir exists; copy or validate agent-written files; write `manifest.json` (paths, hashes, review status) | High | Planned |
| `AwaitOutputReview` — engine pause after schema-valid submit when `require_output_review`; hold output in pending until human decision | High | Planned |
| In-app review UI — render primary markdown in dock/chat; select-to-comment; verdict chips (`approve` / `block` / `question`); reuse patterns from `tools/plan-review.html` | High | Planned |
| Review IPC — `submit_output_review(node_id, decision, comments?)` → engine `on_output_review_decision` | High | Planned |
| Downstream input — `handoff_files` in `build_node_input` alongside `upstream` and `changed_files` | High | Planned |
| Run state projection — `pendingOutputReviews`, handoff paths in trace/overview, status icon `awaiting_output_review` | Medium | Planned |
| Export review — export anchored comments as markdown for the implementing agent (same format as standalone plan review tool) | Medium | Planned |
| Checkpoint round-trip — pending output review + handoff manifest included in resume snapshot ([#24](#run-checkpoint-history-and-replay)) | Medium | Planned |
| Optional workflow template — bundled plan→review→implement workflow using handoff + review toggles | Low | Planned |

**Target:** A planning node writes `plan.md` to a known path, pauses for human review when opted in, and only then hands off. The implementer node's input lists the exact path(s) — no guessing which file the planner meant. Review UX matches Cursor plan review: read markdown, comment on passages, approve or send back for revision.

**Depends on:** [#5 Node completion](#node-completion) (submit contract), [#6 Run lifecycle](#run-lifecycle) (artifact dirs), [#24 Run checkpoint & replay](#run-checkpoint-history-and-replay) (durable handoff paths). **Unlocks:** plan→implement workflow templates, audit trail for agent plans, LLM handoff via exported review markdown.

**Reference:** Submit contract — [`node_invocation.rs`](crates/engine/src/execution/node_invocation.rs); offline review UX — [`tools/plan-review.html`](tools/plan-review.html); tool-approval pause pattern — `AwaitToolApproval` in [`interactive_engine`](crates/engine/src/execution/interactive_engine/mod.rs).

---

## Refactor

Structural cleanup by workspace section. Keep domain logic in `domain`, transport in `providers`, runtime in `orchestration`, Tauri IPC in `desktop`, and frontend in `ui`. See `docs/architecture/contract.md`.

**Serde casing:** Engine persistence uses `snake_case`; IPC/UI DTOs use `camelCase`. Legacy `PascalCase` enum values and field aliases (`#[serde(alias = …)]`) remain for older saved workflows, run logs, and agent definitions. Unify on one convention (T16), then drop the old snake_case ↔ camelCase / PascalCase compatibility shims.

### Domain (`crates/domain`)

| Item | Status |
| --- | --- |
| Vocabulary-aligned module tree (`graph/`, `template/`, `execution/`, `conversation/`, `tools/`, `ports/`) | Done |
| Shared `node_invocation` for `WorkflowRunner` and `InteractiveEngine` | Done |
| `subagent_runtime`, `CallableAgent`, canonical `RunTelemetry` | Done |
| Remove unused port scaffolding; typed template errors; reduce `InteractiveEngine::poll` cloning | Done |
| Collapse `model::NodeTemplate` vs `template::Template` (T2) | Done |
| Node lookup index — `HashMap<NodeId, usize>` (T3) | Done |
| Make `HumanInputPort` / `ToolApprovalPort` load-bearing (T14) | Planned |
| Move `ScriptedAiAdapter` to outbound placement (T15) | Planned |
| Unify serde casing on wire types (T16) | Planned |
| Remove legacy snake_case ↔ camelCase / PascalCase serde aliases — `ChatRole`, `NodeKind`, `CallableAgent` fields, run report enums; after T16 | Planned |
| Trim blanket clippy allows — `clippy -- -D warnings` clean (T18) | Planned |

### Providers (`crates/providers`)

| Item | Status |
| --- | --- |
| Inline `create_provider` factory; remove unused adapter scaffolding | Done |
| `jsonrepair-rs` for tool args and plain JSON completions | Done |
| Per-provider module split audit — keep mapping shared, trim duplicate wire helpers | Planned |
| Provider error taxonomy aligned with domain `AgentError` (T1) | Planned |

### Orchestration (`crates/orchestration`)

| Item | Status |
| --- | --- |
| Thin `AppBackend` — catalog modules, `api.rs`, `error.rs` | Done |
| `execution/` split (`drive`, `events`, `headless`, `subagents`) | Done |
| Move `FileTemplateStore` from domain; alias `ExecutionEvent` → `RunTelemetry` | Done |
| Typed `BackendError`; `spawn_blocking` tool I/O; dead-code removal | Done |
| Unify on one Tokio runtime — `AppBackend` takes injected `Handle` | Done |
| Tool runner error taxonomy + retry loop (T19–T20) | In progress |
| `RunCoordinator` / session lifecycle — stop handle, channel cleanup | Done |
| Store catalog split audit — merge overlapping workflow/project helpers | Planned |

### Desktop (`crates/desktop`)

| Item | Status |
| --- | --- |
| Thin Tauri adapter — commands delegate to orchestration; event bridge only | Done |
| Remove unused port/adapter scaffolding | Done |
| Wire entrypoint through `start_run` IPC | Done |
| `stop_run` command + window-close abort | Done |
| Typed command DTOs — reduce inline structs in `lib.rs` | Planned |

### UI (`crates/ui`)

| Item | Status |
| --- | --- |
| Split shell — `context/`, `screens/`, `panels/`, `components/`, `forms/` | Done |
| `UiDesktopOutboundPort` in `port.ts` | Done |
| Reusable sidebar primitives; shared Agents screen list rows | Done |
| Run stop button + `stopRun` IPC wiring | Done |
| Slim `AppProvider` — extract run listeners, zoom, dock resize into hooks/modules | Planned |
| Typed run-state selectors — reduce `AppContext` surface | Planned |
| Canvas host boundary — keep React Flow isolated from Solid app state | Planned |
| Component tests colocated with `conversation/`, `sidebar/` modules | Done |

**Target:** Each crate has one obvious entry point; cross-crate seams match `AGENTS.md` boundary table; no dead modules or duplicate DTOs between orchestration and UI.

---

## Domain engine hardening

Remediation for modeled-but-unwired behavior and correctness gaps in `crates/domain`. Full task specs (files, acceptance, guardrails) lived in the prior remediation plan; phases below are the execution order. These tasks are sequenced into the queue above (items #3–#5, #30–#31).

### Decisions (resolve before coding)

| ID | Question | Recommendation |
| --- | --- | --- |
| D1 Templates | `model::NodeTemplate` vs `template::Template` — which is canonical? | Keep `template::Template`; persist it in `FileTemplateStore` |
| D2 `available_tools` | Domain resolves tool names, or adapter owns registry? | Confirm against provider crate; document if adapter-owned |
| D3 Parallelism | Concurrent sibling nodes in same execution layer? | Stretch; skip unless needed for demo |
| D4 Max tool rounds | Cap tool-calling rounds per node? | Removed — agents call tools until `openflow_submit_node_output` |
| D5 Tool failure | Retry then feed error to model, or fail node/run immediately? | Default: retry transient tools per policy, then `is_error` result; never abort run for one tool call |

### Phase 1 — Foundations

| Task | Severity | Summary |
| --- | --- | --- |
| T1 Error taxonomy on `AgentError` | P0 | `Transient` / `Permanent` / `Failed`; `is_retryable()` for retry logic — Done |
| T2 Collapse template systems | P0 | Single canonical type per D1 — Done |
| T3 Node lookup index | P1 | `HashMap<NodeId, usize>` in engine; drop O(n²) scans — Done |

### Phase 2 — Functional gaps

| Task | Severity | Summary |
| --- | --- | --- |
| T4 Wire tool-approval policy | P0 | Honor `ApprovalMode`, `ToolTier`, `ToolPolicy` in engine — Done |
| T5 Tool deny / decision resume | P0 | `on_tool_decision`, `approval_id` on `AwaitToolApproval` — Done |
| T6 Implement `retry_policy` | P0 | Retry transient **AI** failures per node with exponential backoff (default 3 attempts) — Done |
| T19 Tool error taxonomy | P0 | `Transient` / `Permanent` on `ToolError` / `ToolRunnerError`; `is_retryable()` — Done |
| T20 Tool invocation retry | P0 | Honor `retry_policy` (or tool-specific override) in `drive.rs` before `ToolCompleted` error |
| T21 Resilient tool failure path | P0 | Failed tools → transcript → `CallAi`; no `ExecutionEvent::Error` / drive exit for tool failures |
| T7 Node-local max-tool-rounds failure | Optional | Only if D4 says so |
| T8 Resolve `available_tools` | P1 | Populate or document per D2 |
| T9 Apply `filter_tool_turn_assistant_message` | P1 | Strip redundant tool-call XML from transcripts — Done |

**High-value P0 path:** T1 → T2 → T5 → T6 → T19 → T20 → T21 → T9 → T10.

### Phase 3 — Correctness and consistency

| Task | Severity | Summary |
| --- | --- | --- |
| T10 Validate node id in `on_ai_complete` | P1 | Reject misrouted completions — Done |
| T11 Fix run-event semantics | P2 | Emit `Started` at `CallAi`; remove provider branding — Done |
| T12 Surface template store persistence errors | P1 | Return `Result` from store mutations — Done |
| T13 Engine input error enum | P2 | Replace `Result<(), String>` with `EngineInputError` |
| T14 Make inbound ports load-bearing | P2 | Implement `HumanInputPort` / `ToolApprovalPort` on engine |

### Phase 4 — Cleanup

| Task | Severity | Summary |
| --- | --- | --- |
| T15 Fix hexagonal file placement | P2 | Move `ScriptedAiAdapter` to outbound |
| T16 Unify serde casing + typo fix | P2 | Wire-format change; keep back-compat aliases |
| T16b Remove legacy casing shims | P2 | Drop `#[serde(alias = …)]` and PascalCase enum accepts after T16 migration |
| T17 Concurrent layer siblings (runner only) | Stretch | Per D3; after phases 1–3 |
| T18 Trim blanket clippy allows | P2 | Last; `clippy -- -D warnings` clean |
