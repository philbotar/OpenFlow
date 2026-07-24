# Roadmap

A single prioritized queue. Work top to bottom — each numbered item is meant to be a self-contained chunk you can finish before moving on. Detailed specs for the larger items live in [Detailed specs](#detailed-specs) below; engine task IDs (T1–T21) are specced in [Engine hardening](#engine-hardening).

**Status:** Done · In progress · Planned

---

## The queue

### Tier 1 — Finish what's started

| # | Item | Status | Details |
| --- | --- | --- | --- |
| 1 | **Chat presentation** — thinking bubbles, collapsible tool rows, tool intent summaries, live tool updates, pretty tool names, args one-liners | In progress | [Chat presentation](#chat-presentation--thinking-bubbles--tool-cleanup) — bubbles, intent, live updates, and chat pretty-names **Done**; run-trace pretty-names, legacy thinking-line cleanup, and OpenAI-only provider thinking **remain** |
| 2 | **Entrypoint wiring** — pass entrypoint text from UI through `start_run` to root node input | Done | [Entrypoint wiring](#wire-entrypoint-text-through-the-desktop-run-path) |

Entrypoint wiring is small but blocks attachments (#15) and any "kick off a run with instructions" flow — do it early.

### Tier 2 — Reliability core

Make runs survivable before adding features on top. A failed tool call or transient provider error should never kill a run.

| # | Item | Status | Details |
| --- | --- | --- | --- |
| 3 | **Error taxonomy + AI retry** — T1 (`AgentError` transient/permanent), T2 (collapse templates), T3 (node lookup index), T5 (tool deny/decision resume), T6 (`retry_policy` with exponential backoff, default 3 attempts) | Done | [Phase 1–2](#phase-1--foundations) |
| 4 | **Tool retry, hooks & resilient failure** — T19 (tool error taxonomy), T20 (tool invocation retry), T21 (failed tools feed transcript and resume `CallAi`; never abort the run), before/after tool hooks for approval/audit/guards | In progress | [Tool retry](#tool-invocation-retry-and-resilience) — T19–T21 **Done**; hook **seam** **Done**; hook **registration** (approval/audit/guards) **Planned** |
| 5 | **Transcript & event correctness** — T9 (strip redundant tool-call XML), T10 (validate node id in `on_ai_complete`), T11 (run-event semantics), T12 (template store persistence errors); [node completion](#node-completion) acceptance | In progress | [Phase 2–3](#phase-2--functional-gaps) · [Node completion](#node-completion) — T9–T12 and NC-1–NC-10 **Done**; NC-13–NC-14 **remain** |
| 6 | **Run lifecycle leftovers** — clean up `openflow-run-*` temp dirs, store event-bridge task handle, decide checkpoint/persistence policy and durable artifact layout (in-memory only vs. disk checkpoints vs. resume after restart) | Planned | [Run lifecycle](#run-lifecycle) |
| 7 | **Secure key storage** — move provider API keys from plaintext `settings.json` to macOS Keychain (keep env-var fallback); migrate existing keys on first launch | Planned | *New* |

### Tier 3 — Daily-driver UX

The things you hit every single run.

| # | Item | Status | Details |
| --- | --- | --- | --- |
| 8 | **Canvas run feedback** — colored status icons per agent state; scrollable in-node subagent list (drop `+N more`); chat node chips use same status colors as canvas | Planned | [Canvas run feedback](#canvas-run-feedback) |
| 9 | **Thinking levels** — `reasoning_effort` schema (node + provider default), gear-panel + inspector controls, provider reasoning param wiring, thinking transcript items | In progress | [Thinking & chat presentation](#thinking--chat-presentation) — schema, UI controls, OpenAI-compat wiring, and `ThinkingBubble` **Done**; Anthropic reasoning/thinking **Planned**; per-run override **Planned** |
| 10 | **Pre-run workflow validation** — validate before `start_run`: dangling edges, cycles, missing provider/model/key, empty prompts; surface as canvas badges + blocking dialog | Planned | *New* |
| 36 | **Workflow insights** — continuous design-time advisory panel: graph smells, config gaps, and best-practice suggestions; non-blocking; jump-to-node fixes | Planned | [Workflow insights](#workflow-insights) |
| 11 | **Project rules** — `.flow/rules/` under linked projects; discovered on load, merged into shared context at run start | Planned | [Project rules](#project-rules) |
| 12 | **Input queue + structured questions** — type ahead during active runs (buffer per node, drain on `AwaitInput`); option-card questions via extended `openflow_request_user_input` | Planned | [Agent questions & todos](#agent-questions--todos) |
| 13 | **Token & cost tracking** — per-turn usage from provider responses; per-node and per-run totals in trace and overview; rough cost estimate per model | Planned | *New* |
| 14 | **Project terminal & jobs** — interactive shell tab in the bottom dock; cwd follows linked project / active run execution root; background job handles for long-running commands | In progress | [Project terminal](#project-terminal) — interactive terminal tab **Done**; job manager + async bash job ids **Planned** |

### Tier 4 — Context & attachments

Getting the right context into and out of agents.

| # | Item | Status | Details |
| --- | --- | --- | --- |
| 15 | **Attachments & file references** — attach button, `@` token combobox, drag-drop; resolved content in submit payload and entrypoint | In progress | [Attachments](#attachments--file-references) — `@` combobox + resolve-on-submit (inlined text) **Done**; attach button, drag-drop, structured payload, pills, images **Planned** |
| 37 | **Agent prompt skill references** — `/skill` tokens in saved-agent and node system/task prompts; slash combobox + preview; expand skill bodies at run-start (or per turn) instead of pasting full instructions | Planned | [Agent prompt skill references](#agent-prompt-skill-references) |
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
| 23 | **Cron / scheduled runs + workflow retry loop** — execute the schedule/retry schema fields that already exist | In progress | [Cron / scheduled runs](#cron--scheduled-runs) — cron schedule + Schedule screen + due-run loop **Done**; workflow retry loop **Planned** |
| 24 | **Run checkpoint, history, and replay** — persist run checkpoints to disk; browse run history; resume paused runs or replay from a checkpoint (read-only trace or forked re-execution); depends on persistence policy (#6) | Planned | [Run checkpoint & replay](#run-checkpoint-history-and-replay) |
| 25 | **Programmatic / non-AI nodes** — code/script, API-call, and transform nodes between agent nodes; deterministic execution without LLM turns | Planned | [Programmatic nodes](#programmatic--non-ai-nodes) |
| 26 | **External connectors** — Composio / n8n-style integration nodes | Planned | |
| 27 | **Run insights & self-learning** — extract durable lessons from completed runs; surface patterns, failures, and optimization hints; inject approved insights into future runs | Planned | [Run insights & self-learning](#run-insights--self-learning) |
| 35 | **Workflow orchestration & reinvoke** — child workflow runs, foreach/batch over repo files, in-app scripting to start runs; partial re-run of a node or subgraph | Planned | [Workflow orchestration & reinvoke](#workflow-orchestration--reinvoke) |
| 38 | **In-app file viewer from node output** — agents and nodes surface clickable file references in chat, canvas, and handoff output; open paths in an in-app reader (syntax highlight, markdown, line ranges) without leaving the app | Planned | [In-app file viewer](#in-app-file-viewer-from-node-output) |

### Tier 6 — Polish & distribution

| # | Item | Status | Details |
| --- | --- | --- | --- |
| 28 | **Canvas editing QoL** — undo/redo for graph edits, duplicate node, copy/paste between workflows | Planned | *New* |
| 29 | **Accessibility & keyboard shortcuts** — panel toggles, focus management, shortcut reference overlay | In progress | [Accessibility](#accessibility) — shortcut reference overlay **Done**; panel toggles and focus management **Planned** |
| 30 | **Onboarding & templates** — first-run empty state, 2–3 bundled example workflows, "new from template" | Planned | *New* |
| 31 | **macOS distribution** — code signing, notarization, auto-update (Tauri updater); bundle already builds | Planned | *New (expands packaging)* |
| 32 | **Serde casing unification** — T16 (one wire convention) then T16b (drop legacy aliases/shims) | Planned | [Phase 4](#phase-4--cleanup) |
| 33 | **Cleanup pass** — T13–T15, T18 (clippy `-D warnings`), refactor polish: slim `AppProvider`, typed desktop DTOs, store catalog audit, provider module audit | Planned | [Refactor](#refactor) |

### Dev & agent tooling

| # | Item | Status | Details |
| --- | --- | --- | --- |
| 34 | **Interactive plan review tool** — standalone HTML+JS for markdown plan review | Done | [Plan review tool](#interactive-plan-review-tool) |

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
- Error logging stored locally (**backend slice Done**; UI + agent auto-fix loop follow-up) — persistent error reporting plan (`docs/superpowers/plans/2026-06-15-persistent-error-reporting.md`)
- Workflow version control (per-change revert)
- Natural language workflow definition (partial: **Build with AI** authoring screen shipped — [`WorkflowAuthoringScreen.tsx`](../crates/ui/src/screens/WorkflowAuthoringScreen.tsx); full NL builder still backlog)
- Workflow authoring polish — inspector apply UX, template library integration (validation banner → [#36 Workflow insights](#workflow-insights))
- T7 node-local max-tool-rounds (only if D4 changes), T17 concurrent layer siblings in headless runner (stretch)

**Deferred** until workflow retry loop ([#23](#cron--scheduled-runs)) and [#35 Workflow orchestration](#workflow-orchestration--reinvoke) land: background job start/stop/resume at the process level (distinct from in-workflow child runs). Cron scheduling while the app is open is **Done** — see [#23](#cron--scheduled-runs).

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

**Reference:** Domain test `blank_entrypoint_is_not_injected_into_root_input` in `crates/engine/src/execution/node_invocation.rs`.

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
| `crates/orchestration/src/run/coordinator/mod.rs` | `RunSession` is in-memory only; no run store; `start_run` always creates a fresh engine |
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

**Depends on:** #6 (persistence policy). **Unlocks:** #23 (scheduled runs need durable run records), #17 (handoff artifact paths), #27 (run insights need durable run records), [#35 Workflow orchestration](#workflow-orchestration--reinvoke), audit/compliance use cases.

**Reference:** Live projection — `WorkflowRunState` in `crates/orchestration/src/run/state/mod.rs`; headless snapshot — `WorkflowRunSnapshot` in `crates/orchestration/src/run/execution/mod.rs`; artifact temp dirs — `drive.rs` (`openflow-run-{uuid}`).

### Workflow insights

Workflow authors today get **errors only at run time** or when the Build-with-AI authoring screen validates a draft. The editor has no standing **advisory panel** that explains how to improve graph design, node configuration, or run readiness while you edit. [`validate_authoring_workflow`](../crates/orchestration/src/workflow/authoring/validate.rs) already checks empty prompts, output schema shape, and DAG legality for AI-authored drafts, but those checks are not surfaced continuously in the canvas editor. Queue item [#10 Pre-run validation](#tier-3--daily-driver-ux) covers **blocking** gates before `start_run`; this section covers **non-blocking insights** — ranked suggestions with severity, evidence, and jump-to-fix affordances.

Distinct from [#27 Run insights & self-learning](#run-insights--self-learning): **Workflow insights** analyze the **workflow definition** (and optional linked project/settings context). **Run insights** analyze **completed run telemetry** and promote lessons into future context. The two may cross-link later ("3 recent runs failed at node X — add retry policy") but ship independently.

| Layer | Gap |
| --- | --- |
| `crates/engine/src/graph/validation.rs` | DAG errors only — no graph-smell heuristics (orphans, depth, fan-out) |
| `crates/orchestration/src/workflow/authoring/validate.rs` | Authoring-only semantic checks; not invoked on every editor save |
| `crates/orchestration/src/settings/provider.rs` | Provider readiness exists; not projected as per-node insight rows |
| `crates/orchestration/src/workflow/catalog.rs` | No `analyze_workflow` use-case or insight DTO |
| `crates/desktop/src/lib.rs` | No `list_workflow_insights` IPC |
| `crates/ui/src/panels/` | No Insights panel in gear sidebar or inspector |
| `crates/ui/src/canvas/` | No per-node insight badges (distinct from blocking validation badges in #10) |

**Decisions (resolve before coding):**

| ID | Question | Recommendation |
| --- | --- | --- |
| WI1 | What is a workflow insight? | A **structured suggestion** with `kind`, `severity` (`info` / `warning` / `hint`), `summary`, `detail`, optional `node_ids[]`, `fix_action` (`select_node`, `open_inspector`, `open_settings`), and `status` (`open` / `dismissed` / `resolved`) |
| WI2 | Blocking vs advisory? | **Never block** `start_run` from insights alone — blocking rules stay in #10. Insights may **recommend** running validation or fixing config before run |
| WI3 | When to recompute? | On workflow graph/config change (debounced); on linked project or settings change; optional manual refresh. Cache last result keyed by workflow content hash |
| WI4 | Where in UI? | **Gear panel → Insights tab** (workflow-scoped list) + optional compact **editor header chip** ("3 suggestions"); per-node rows in inspector when a node is selected |
| WI5 | Dismiss semantics? | Per-workflow dismiss stored in app data (`openflow/insight-dismissals.json` or embedded in workflow metadata); dismissed kinds do not reappear until workflow changes materially |
| WI6 | Run-informed hints (v2)? | When durable run history exists ([#24](#run-checkpoint-history-and-replay)), optional insights cite recent failures — thin bridge to [#27](#run-insights--self-learning), not duplicate extractors |

**Insight kinds (v1 taxonomy):**

| Kind | Source signal | Example |
| --- | --- | --- |
| `graph_orphan` | Node with zero incoming and zero outgoing edges | "Node 'Debug' is disconnected — wire it or delete it" |
| `graph_depth` | Longest path exceeds threshold | "7-node linear chain — consider parallel layers or a Code node for batch prep" |
| `graph_fan_out` | Layer with many siblings, no merge/join semantics ([#20](#tier-5--power-features)) | "4 nodes at layer 2 run in parallel with no join — downstream may race" |
| `config_empty_prompt` | Empty `system_prompt` or `task_prompt` | "Node 'Research' has an empty task prompt" |
| `config_missing_model` | Node model unset and no provider default | "Node 'Writer' has no model and no provider default is configured" |
| `config_no_tools` | Agent node with empty tool catalog on a task that typically needs I/O | "Implementer has no file tools — add read/write or confirm read-only intent" |
| `config_write_without_read` | Write-tier tools enabled, no read-tier tools | "Node can edit files but cannot read them first" |
| `config_approval_risk` | `ApprovalMode::Yolo` or `always_ask` mismatch with tool tiers | "YOLO mode on a node with bash — consider write approval" |
| `config_missing_summary` | Output schema lacks `summary` field | "Add a `summary` string to output schema for chat handoff ([#5 Node completion](#node-completion))" |
| `workflow_no_shared_context` | Multi-node workflow, empty `shared_context` | "3 agents share no workflow context — add standards in gear panel" |
| `workflow_duplicate_prompt` | Identical system prompts on 2+ nodes | "Planner and Reviewer use the same system prompt — differentiate roles" |
| `run_readiness_provider` | Active provider missing API key / model unavailable | "OpenAI provider has no API key — add in Settings before run" |
| `run_readiness_project` | Workflow linked to missing project path | "Linked project folder not found — re-bind in sidebar" |
| `schedule_disabled` | Cron expression set but `enabled: false` | "Schedule is configured but disabled" |

| Item | Priority | Status |
| --- | --- | --- |
| Insight schema — `WorkflowInsight`, kinds, severity, evidence, dismiss id; serde round-trip tests | High | Planned |
| `analyze_workflow` use-case — merge engine DAG validation + semantic checks from authoring validator + settings/provider readiness | High | Planned |
| Graph heuristics — orphan detection, max depth, fan-out per layer, duplicate prompt hash | Medium | Planned |
| Config heuristics — empty prompts, tool/approval mismatches, missing `summary` in output schema | High | Planned |
| IPC — `list_workflow_insights(workflow_id)` returns ranked list; optional `dismiss_workflow_insight` | High | Planned |
| Insights panel — gear sidebar tab: grouped by severity; click row selects node / opens inspector field | High | Planned |
| Header summary chip — "N suggestions" opens Insights tab; hide when zero | Medium | Planned |
| Inspector inline hints — top 1–3 insights for selected node with jump links | Medium | Planned |
| Debounced recompute — hook editor mutations + project/settings listeners | High | Planned |
| Dismiss persistence — per-workflow dismissed insight keys survive reload | Medium | Planned |
| Share validator with authoring — single source for semantic checks in `workflow/authoring/validate.rs` and insights analyzer | High | Planned |
| Canvas badges (optional) — non-blocking amber dot on nodes with open warnings (distinct from red blocking badges in #10) | Low | Planned |
| Run-informed hints — surface recurring node failures from run history when #24 lands | Low | Planned |
| Export insights — markdown checklist for workflow review PRs | Low | Planned |

**Target:** Open a workflow → gear panel **Insights** lists actionable improvements ("Implementer has write tools but no read tools", "Provider key missing"). Click a row → canvas selects the node and inspector scrolls to the relevant field. Dismiss noise you accept intentionally. Run still works when only `info`-level hints remain; **Run** stays blocked only when #10 validation fails.

**Depends on:** existing `validate_workflow` + `validate_authoring_workflow` checks. **Complements:** [#10 Pre-run validation](#tier-3--daily-driver-ux) (blocking), [#27 Run insights](#run-insights--self-learning) (post-run). **Unlocks:** better onboarding templates ([#30](#tier-6--polish--distribution)), workflow authoring polish, fewer failed first runs.

**Reference:** DAG validation — [`validation.rs`](../crates/engine/src/graph/validation.rs); authoring semantic checks — [`validate.rs`](../crates/orchestration/src/workflow/authoring/validate.rs); provider readiness — [`settings/provider.rs`](../crates/orchestration/src/settings/provider.rs); Build-with-AI validation UI — [`AuthoringDraftPreview.tsx`](../crates/ui/src/components/workflowAuthoring/AuthoringDraftPreview.tsx).

### Run insights & self-learning

Completed runs today produce rich telemetry — transcripts, tool results, retries, approvals, node outputs, changed files — but none of it compounds across attempts. Every `start_run` begins with the same workflow `shared_context` and node prompts. Users must manually notice recurring failures (same tool error three runs in a row), copy lessons into workflow settings, or re-explain constraints in the entrypoint. There is no post-run retrospective, no workflow-scoped memory, and no way to promote a run-time discovery into durable guidance for the next execution.

This section covers **insights** (human- and machine-readable observations extracted from run records) and **self-learning** (injecting approved insights back into future runs at workflow, project, or node scope). It deliberately does **not** copy oh-my-pi's opaque memory tools wholesale — insights map to Step-through's existing context surfaces (`shared_context`, project rules, per-node preamble, upstream handoffs) so learning stays inspectable and editable.

| Layer | Gap |
| --- | --- |
| `crates/orchestration/src/run/state/` | `WorkflowRunState` is live projection only; no post-run analysis artifacts |
| `crates/orchestration/src/run/execution/events.rs` | Telemetry is consumed for UI/trace; not aggregated into insight records |
| `crates/orchestration/src/adapters/storage/` | No insight store — no `{project}/.flow/insights/` or per-workflow insight index |
| `crates/orchestration/src/run/coordinator/mod.rs` | `start_run` does not merge prior-run insights into context assembly |
| `crates/engine/src/execution/node_invocation.rs` | `build_node_input` / preamble have no `insights` or `run_history_summary` block |
| `crates/engine/src/graph/workflow.rs` | `WorkflowSettings` has no insight policy (auto-inject vs suggest-only vs off) |
| `crates/orchestration/src/project/` | Project registry does not discover workflow insights alongside rules |
| `crates/ui/src/` | No insights panel on completed runs; no "promote to workflow" affordance; no cross-run comparison |
| `crates/desktop/src/lib.rs` | No IPC for list/generate/approve/dismiss insights |

**Decisions (resolve before coding):**

| ID | Question | Recommendation |
| --- | --- | --- |
| I1 | What is an insight? | A **structured record** with `kind`, `summary`, `evidence` (run id, node id, trace refs), `confidence`, `status` (`suggested` / `approved` / `dismissed`), and optional `scope` (`workflow`, `project`, `node:{id}`) |
| I2 | Where do insights live? | Project-scoped index: `{project}/.flow/insights/index.json` + `{insight_id}.json`; app-only workflows mirror under app data dir. Per-run **raw analysis** stays inside the run record: `{project}/.flow/runs/{run_id}/insights.json` |
| I3 | Who generates insights? | **v1:** deterministic extractors over run telemetry (retries, tool errors, approval denials, node failures, duration outliers) + optional **post-run LLM retrospective** (single summarizer node or headless `AiPort` call) producing suggested insights only — never auto-approved |
| I4 | Human gate before injection? | **Yes.** Suggested insights appear in UI; user **approves** (promotes to scope) or **dismisses**. Approved insights merge at run start; dismissed insights are retained for audit but not injected |
| I5 | Injection surface | Approved insights append to **`WorkflowSettings.shared_context`** (workflow scope), **project rules** (project scope), or **node system prompt suffix** (node scope) — same merge order as [#11 Project rules](#project-rules) and [#18 Context used](#context-used); show in context ledger |
| I6 | Cross-run comparison? | **v2:** diff two runs of the same workflow (status, per-node outcomes, tool error sets, token totals when queue item #13 lands) — read-only analytics, not learning injection |
| I7 | Scheduled / retry-loop learning? | Cron and workflow retry fields (queue item #23) may attach `insight_policy: refresh_on_failure` — re-run extractors when a scheduled attempt fails; surface "same failure as last Tuesday" in schedule UI |
| I8 | Privacy / retention | Insights may quote file paths and error snippets; respect run prune policy ([#24](#run-checkpoint-history-and-replay)); optional redact paths in exported insight bundles |

**Insight kinds (v1 taxonomy):**

| Kind | Source signal | Example |
| --- | --- | --- |
| `tool_failure_pattern` | Repeated `is_error` tool results with same tool + error class | "bash exits 127 when `./scripts/verify.sh` run outside project root" |
| `retry_friction` | AI or tool retries exhausted per node | "Research node hits transient provider errors — consider higher `retry_policy.max_attempts`" |
| `approval_bottleneck` | Frequent `AwaitToolApproval` denials or long approval latency | "Edit tool on Implementer node denied 4× — tighten path allowlist or switch approval mode" |
| `node_failure` | Terminal node `failed` with structured output or trace error | "Planner submit missing `summary` field — schema mismatch" |
| `context_gap` | Downstream re-reads files upstream already read (after [#16 Upstream read-file context](#upstream-read-file-context)) | "Reviewer re-read `src/api.ts` — upstream read ledger not wired yet" |
| `handoff_miss` | Downstream agent searches for plan file not in `handoff_files` (after [#17](#node-handoff-artifacts--output-review)) | "Implementer used `read` on ad-hoc path — enable handoff review on Planner" |
| `cost_outlier` | Per-node token total > workflow median × N (after queue item #13) | "Debug node consumed 80% of run tokens" |
| `retrospective` | Post-run LLM summary over trace + outputs | "Run succeeded but tests were never executed — add verification node" |

| Item | Priority | Status |
| --- | --- | --- |
| Insight schema — `InsightRecord`, kinds, status, scope, evidence refs; serde round-trip tests | High | Planned |
| Run-record analysis artifact — write `insights.json` (suggested + extractor metadata) on terminal outcomes | High | Planned |
| Deterministic extractors — tool failure patterns, retry/approval counts, node failure messages, run duration | High | Planned |
| `InsightStore` adapter — CRUD approved/dismissed insights; list by workflow/project/node scope | High | Planned |
| Post-run retrospective — optional headless `AiPort` call or dedicated summarizer workflow template | Medium | Planned |
| Approve / dismiss IPC — `list_insights`, `approve_insight`, `dismiss_insight`, `generate_run_insights(run_id)` | High | Planned |
| Run-start merge — inject approved insights into `shared_context` / project rules / node preamble per scope | High | Planned |
| Context ledger attribution — `context_used` rows for injected insights ([#18 Context used](#context-used)) | Medium | Planned |
| Insights UI — completed-run panel: suggested cards with evidence links to trace rows; approve/dismiss actions | High | Planned |
| Promote to workflow — one-click approve at workflow scope from run insights panel | High | Planned |
| Workflow insight policy — `WorkflowSettings.insight_policy`: `off` / `suggest_only` / `inject_approved` (default `inject_approved`) | Medium | Planned |
| Inspector preview — show approved insights that will apply on next run for selected node | Medium | Planned |
| Export insights — markdown bundle for a workflow or run (audit, PR attachment) | Low | Planned |
| Cross-run diff UI — pick two runs; compare outcomes, errors, tokens (depends on #13, #24) | Low | Planned |
| Insight decay — optional `expires_at` or max age; auto-archive stale suggestions | Low | Planned |
| Schedule failure linkage — surface recurring insight on schedule screen when same `tool_failure_pattern` repeats | Low | Planned |

**Target:** Finish a run → open **Insights** on the run history row → see suggested lessons with trace evidence ("bash failed 3× with exit 127 on node Implementer"). Approve the ones you trust; they merge into the next run's context automatically and appear in **Context used** before the first `CallAi`. Dismiss noise without deleting audit history. Over time the workflow accumulates durable, human-curated guidance — self-learning without a black-box memory store.

**Depends on:** [#24 Run checkpoint & replay](#run-checkpoint-history-and-replay) (durable run records and history UI). **Strongly benefits from:** [#11 Project rules](#project-rules) (project-scoped injection), [#18 Context used](#context-used) (attribution), queue item #13 (cost outliers), [#16 Upstream read-file context](#upstream-read-file-context) and [#17 Node handoff artifacts](#node-handoff-artifacts--output-review) (richer extractors). **Unlocks:** smarter scheduled/retry loops (queue item #23), workflow authoring suggestions ("your last 5 runs failed at the same node"), compliance exports, reduced repeated user entrypoint explanations.

**Reference:** Run telemetry — `RunTelemetry` in `crates/engine/src/execution/telemetry.rs`; live projection — `WorkflowRunState` in `crates/orchestration/src/run/state/`; context assembly — `node_invocation.rs`; OMP memory stance — OMP tool parity harness (`docs/superpowers/plans/2026-06-14-omp-tool-parity-harness.md`) (map memory to context ledger / project rules, not opaque recall).

### Provider API key storage

| Item | Priority | Status |
| --- | --- | --- |
| Persist keys in `settings.json` (`ProviderProfile.api_key`) | High | Done |
| Settings UI plaintext risk notice | High | Done |
| Env var fallback unchanged | High | Done |
| macOS Keychain storage — keys out of plaintext; migrate on first launch (#7) | High | Planned |

### Tool invocation retry and resilience

Today a failed tool call becomes a single `is_error: true` [`ToolResult`](../crates/engine/src/tools/config.rs) fed back to the model. [`retry_policy`](../crates/engine/src/graph/workflow.rs) (T6) applies only to transient **AI** [`AgentError`](../crates/engine/src/ports/outbound.rs), not tool-runner failures. The drive loop can still **exit the run** on orchestration/engine mismatches (`on_tool_results` error → `ExecutionEvent::Error`) or on AI invoke failure after retries.

| Layer | Gap |
| --- | --- |
| `crates/orchestration/src/tool/runner.rs` | `ToolRunnerError::is_retryable()`; retry via `tool/retry.rs` + `tool_port.rs` | **Done** |
| `crates/orchestration/src/run/execution/tool_port.rs` | Retry loop with backoff; `ToolRetrying` telemetry | **Done** |
| `crates/engine/src/execution/interactive_engine/tools.rs` | Partial tool batches filled with error results; run continues | **Done** |

**Target behavior:**

1. Classify tool failures as retryable (timeout, rate limit, transient I/O) vs permanent (bad args, missing file, policy deny). **Done** — `ToolError::is_retryable()` in `orchestration/src/tool/errors.rs`.
2. Retry retryable tool invocations per workflow/node policy (`max_attempts`, `backoff_ms`) **before** surfacing an error result to the model. **Done** — `execute_with_retry` in `orchestration/src/tool/retry.rs`, wired from `tool_port.rs`.
3. On exhausted retries or permanent failure, append `is_error: true` tool result and **resume the agent loop** (`CallAi`) — do not terminate the run or crash the host. **Done**
4. Reserve run-level failure for unrecoverable host errors (engine state corruption, cancelled run), not individual tool calls. **Done**

**oh-my-pi imports to keep:**

| Pattern | Step-through shape |
| --- | --- |
| Catch all expected tool failures | `ToolRunnerError` becomes a structured `ToolResult { is_error: true }` whenever the engine can continue |
| Tool retry classification | Keep `ToolError::is_retryable`; add retry/backoff in orchestration before `ToolCompleted` error projection |
| Before/after tool hooks | Hook **seam** Done (`ToolHooks` around `ToolRunner::execute`); registering approval/audit/guard hooks in composition root **Planned** |
| Abort/cancel distinction | Preserve user stop / node interrupt as cancellation, not model-visible tool failure |
| Structured output metadata | Never truncate silently; preserve `ToolOutputMeta` and artifact references in run history |

**Depends on:** T1 (error taxonomy pattern), T6 (retry policy wiring). See T19–T21 in domain hardening.

### Chat presentation — thinking bubbles & tool cleanup

Assistant token streaming is wired (`ChatMessageDelta` → chat log). Collapsible tool rows, thinking bubbles, intent summaries, and live tool tails are shipped in chat. Remaining polish: run-trace pretty-names, legacy thinking-line cleanup, and Anthropic provider thinking.

| Item | Priority | Status |
| --- | --- | --- |
| Collapsible tool bubbles — collapsed row shows tool name + one-line outcome; expand for args and full output | High | Done |
| Thinking bubble UI — collapsible reasoning block in chat; distinct from assistant messages; collapsed by default | High | Done |
| Provider thinking in transcript — parse reasoning blocks from provider responses; project to chat (not legacy `ChatRole::Thinking` tool lines) | High | In progress — OpenAI-compat (`reasoning_content` / `ThinkingDelta`) **Done**; Anthropic thinking blocks **Planned** |
| Tool intent field — add optional `_i` / `intent` text to tool-call schema; show it as the collapsed tool-row summary when present | High | Done |
| Pretty tool names — human-readable labels in chat (`ToolBubble`, `ToolApprovalCard`, `FileChangesPanel`) | Medium | Done |
| Pretty tool names — same mapping in run trace rows (`events.rs` still uses raw ids) | Medium | Planned |
| Tool row chrome — drop `Tool Invocation:` header; status chip (running / completed / failed); chevron expand | Medium | Done |
| Args summary — one-line path/query preview when collapsed; full formatted JSON only when expanded | Medium | Done |
| Live tool updates — emit `ToolUpdated` / tail events for long-running tools; stream current tail into the expanded row while preserving final full output or artifact | High | Done |
| Streaming thinking — append reasoning tokens into the thinking bubble during active turns | Medium | Done |
| Hide legacy thinking tool lines — stop grouping provider reasoning with legacy tool I/O prose | Medium | Planned |

**Reference:** [`ToolBubble.tsx`](../crates/ui/src/components/conversation/ToolBubble.tsx); full spec in [Thinking & chat presentation](#thinking--chat-presentation).

### Canvas run feedback

During a run, agent nodes show a status row and optional subagent rows. Subagents are capped at three visible entries with a `+N more` overflow line; status is a colored dot plus text label (`WorkflowNode.react.tsx`, `agentStatus.ts`).

| Layer | Gap |
| --- | --- |
| `crates/ui/src/canvas/WorkflowNode.react.tsx` | `MAX_VISIBLE_SUBAGENTS = 3` truncates the list; no scroll container |
| `crates/ui/src/styles/index.css` | `.node-subagent-list` is static; no max-height / overflow-y |
| `crates/ui/src/canvas/WorkflowNode.react.tsx` | Status is dot + text only — no distinct icon per `AgentStatus` |
| `crates/ui/src/lib/agentStatus.ts` | Labels only; no icon or color token mapping for canvas chrome |
| `crates/ui/src/components/conversation/ConversationMessages.tsx` | ~~Filter chips forced gray via `.chat-filter-status-dot`~~ **Fixed** — dots inherit `.status-*` palette |
| `crates/ui/src/components/conversation/ChatPanel.tsx` | Live-node picker chips use status palette (same fix as filter chips) |
| `crates/ui/src/styles/index.css` | Canvas `.status-*` palette exists; chat chips inherit it after dot override fix |

| Item | Priority | Status |
| --- | --- | --- |
| Chat status color parity — filter chips and live-node picker dots use the same `.status-*` colors as canvas nodes | High | Done |
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
| Lifecycle — kill PTY on app close; multi-tab sessions | Medium | Done |
| Lifecycle — warn if shell still running on close (ties to warn-on-close backlog) | Medium | Planned |
| New terminal / split — additional PTY tabs (v2 polish) | Low | Done |
| Inject command from chat — "Run in terminal" on bash tool rows (optional; depends on live bash output) | Low | Planned |

**Target:** Open the bottom dock → Terminal → get a project-scoped shell immediately. Run `cargo nextest run`, `git status`, or `./scripts/verify.sh` while a workflow run is paused or in progress. Cwd matches where agents execute when a run is active.

**Not in v1:** Remote SSH shells, root/sudo elevation UI, or replaying agent bash invocations as read-only panes (chat tool rows remain the audit trail).

**Reference:** Dock tabs — [`DockPanel.tsx`](../crates/ui/src/panels/DockPanel.tsx); execution cwd — `orchestration/src/run/execution/`; bash tool — [`bash.rs`](../crates/orchestration/src/adapters/tool_impl/bash.rs).

### Cron / scheduled runs

Workflow-level cron schedules (`WorkflowSettings.schedule`) persist on the workflow. While the desktop app is open, a background poll claims due runs and starts them through the normal run harness. Schedule status appears on the Schedule screen and in bootstrap payloads.

| Layer | Status |
| --- | --- |
| `crates/engine/src/graph/workflow.rs` | `WorkflowSchedule` schema (`cron`, `enabled`, `timezone`) — **Done** |
| `crates/orchestration/src/schedule/` | `ScheduleService` — refresh, statuses, `claim_due_run` — **Done** |
| `crates/ui/src/screens/ScheduleScreen.tsx` | Schedule sidebar UI — **Done** |
| `crates/desktop/src/lib.rs` | `spawn_schedule_loop` → `start_due_scheduled_run` — **Done** |

| Item | Priority | Status |
| --- | --- | --- |
| Schedule schema on `WorkflowSettings` | High | Done |
| Schedule screen — enable/disable cron, pick preset or custom expression, timezone | High | Done |
| Due-run loop — poll while app open; skip when manual run active; emit schedule status events | High | Done |
| Workflow retry loop — re-run workflow on terminal failure per schema (distinct from per-turn `retry_policy`) | High | Planned |
| Durable run records for scheduled attempts (history, failure streaks) | Medium | Planned — depends on [#24](#run-checkpoint-history-and-replay) |

**Target:** Set a cron on a workflow; the app starts it automatically when due while open. Retry-loop semantics (automatic re-run after failed runs) remain future work.

**Reference:** Schedule sidebar plan (`docs/superpowers/plans/2026-06-16-schedule-sidebar.md`).

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

Per-node and workflow-level `reasoning_effort` / `reasoning_budget_tokens` are in the schema. Settings, gear panel, and inspector expose controls. OpenAI-compatible providers forward reasoning params and stream `ThinkingDelta` into `ThinkingBubble` rows. Anthropic thinking lives in `rig_adapter/` (`claude_thinking.rs`, `reasoning_convert.rs`) — keep that path current when extending Anthropic reasoning. Legacy `ChatRole::Thinking` tool prose still appears alongside structured bubbles until cleanup lands.

| Layer | Gap |
| --- | --- |
| `crates/providers/src/rig_adapter/` (Anthropic) | Confirm thinking/budget mapping and block parsing stay complete for all Anthropic models |
| `crates/providers/src/rig_adapter/` (OpenAI-compat) | Forwards reasoning params; streams reasoning as `ThinkingDelta` — **Done** |
| `crates/ui/src/lib/parseLegacyToolMessages.ts` | Legacy tool I/O lines still reuse `ChatRole::Thinking`; provider reasoning is distinguished via `isProviderThinkingMessage` |
| `crates/orchestration/src/run/execution/events.rs` | Run trace tool rows still use raw tool ids (pretty-names chat-only today) |
| `crates/ui/src/components/conversation/` | No per-run thinking override in chat chrome |

| Item | Priority | Status |
| --- | --- | --- |
| Thinking level schema — `reasoning_effort` + `reasoning_budget_tokens` on agent node + saved agent | High | Done |
| Provider default — pick default reasoning effort in Settings → Reasoning (applied at run start when node unset) | High | Done |
| Workflow settings control — pick default thinking level in gear panel (off / low / medium / high or provider-aligned presets) | High | Done |
| Inspector control — pick thinking level per node; inherit provider default when unset | High | Done |
| Provider wiring (OpenAI-compat) — map level to reasoning params; parse `reasoning_content` into `ThinkingDelta` | High | Done |
| Provider wiring (Anthropic) — map level to Anthropic thinking/reasoning params; parse thinking blocks from responses | High | Planned |
| Thinking transcript items — stream reasoning into chat as `ThinkingBubble` rows (distinct from legacy `ChatRole::Thinking` tool lines) | High | Done (OpenAI-compat); Planned (Anthropic) |
| Collapsible tool bubbles — collapsed row shows tool name + one-line outcome; expand for args and full output | High | Done |
| Tool intent field — support optional `_i` / `intent` in tool-call args; collapsed tool row prefers intent over raw-arg summaries | High | Done |
| Live tool updates — add `ToolUpdated` event and tail-buffer UI for bash/eval/job output while a tool is still running | High | Done |
| Pretty tool names — map builtin/subagent ids to short human labels in `ToolBubble`, `ToolApprovalCard`, and `FileChangesPanel` | Medium | Done |
| Pretty tool names — same mapping in run trace rows | Medium | Planned |
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
| `crates/engine/src/execution/interactive_engine/mod.rs` | No in-run todo state; questions resume as plain user messages |
| `crates/orchestration/src/run/coordinator/mod.rs` | `submit_user_input` rejects unless `awaiting_node_id` matches |
| `crates/orchestration/src/run/execution/drive.rs` | `ProvideInput` ignored during tool approval; no input buffer |
| `crates/orchestration/src/run/state/mod.rs` | Run state has no todo or pending-question projection; no input queue |
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

**Shipped:** One run-wide chat pane via `projectChatLayout` (`crates/ui/src/lib/workflow.ts`). Settled history is layer-ordered; live nodes render in side-by-side columns with per-node composers and approval cards. Filter chips narrow by node; canvas selection can scroll/highlight a segment. Idle composer can start a run with entrypoint text. Backend remains per-node `chatLogs`; projection is UI-only.

| Layer | Role |
| --- | --- |
| `crates/ui/src/lib/workflow.ts` | `projectChatLayout` — layer order, settled vs live columns, overflow tabs |
| `crates/ui/src/context/appProvider/useRunSession.ts` + `useChatComposer.ts` | Merged layout state, kickoff/flush, per-node draft + submit routing |
| `crates/ui/src/components/conversation/ChatPanel.tsx` | Settled segments + live column strip |
| `crates/orchestration/src/run/state/` | `chatLogs: Record<NodeId, ChatMessage[]>` — source of truth |

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

**Reference:** Submit contract — [`node_invocation.rs`](../crates/engine/src/execution/node_invocation.rs) (`NODE_RUNTIME_PREAMBLE`); completion path — [`completion.rs`](../crates/engine/src/execution/interactive_engine/completion.rs); run projection — [`events.rs`](../crates/orchestration/src/run/execution/events.rs); technical overview § “When a node is done” — [`technical-overview.md`](architecture/technical-overview.md).

### Attachments & file references

Users can invoke skills with `/skill` tokens and attach project context with `@{path}` tokens in the chat composer. On submit (including idle run kickoff), referenced paths are read under the execution cwd and inlined into the message text. Structured `referenced_files` payloads, attach button, drag-drop, pills, and vision images are not shipped yet.

| Layer | Role / gap |
| --- | --- |
| `crates/ui/src/lib/fileReferences.ts` | `@` / `@{path}` token parsing, completion, path extraction, inline formatting — **Done** |
| `crates/ui/src/components/conversation/FileReferenceCombobox.tsx` | Project file combobox in composer — **Done** |
| `crates/orchestration/src/project/file_refs.rs` | List + read referenced paths under execution cwd jail — **Done** |
| `crates/ui/src/context/appProvider/useChatComposer.ts` | `resolveChatSubmission` reads refs before `submit_user_input` / run kickoff — **Done** |
| `crates/ui/src/components/conversation/` | No attach button, drag-drop target, reference pills, or preview chrome |
| `crates/engine/src/execution/` | User input is a single string; no structured `referenced_files` in transcript or node input |

| Item | Priority | Status |
| --- | --- | --- |
| `@` token UX — combobox over linked-project files; `@{path}` completion | High | Done |
| Reference resolution — read file content under execution cwd on submit; reject paths outside project jail | High | Done |
| Entrypoint attachments — resolve `@{path}` tokens when starting a run from idle composer | Medium | Done (inlined into entrypoint text) |
| Reference budget — max files, max bytes, truncate with notice in formatted submit text | Low | Done (65536-byte cap in `file_refs.rs`) |
| Attach button — paperclip in composer opens file picker over linked-project tree | High | Planned |
| Drag-and-drop — drop files onto composer to attach (paths resolved under execution cwd jail) | Medium | Planned |
| Structured submit payload — `referenced_files: [{ path, content \| excerpt }]` alongside message text | High | Planned |
| Transcript shape — persist references in `AgentTranscriptItem::UserMessage` and chat log projection | Medium | Planned |
| Composer chrome — pills for attached paths; expandable preview (path + line range + size cap); remove via × | Medium | Planned |
| Image attachments — paste or pick images; encode for vision-capable providers when model supports it | Medium | Planned |
| Line-range refs — `@path:10-40` or selection-from-editor hook | Low | Planned |

**Target:** Attach project files via button, `@` token, or drag-drop before send (or on run start). Resolved content is injected into the user message or entrypoint JSON so the agent sees explicit file context without an extra `read` tool round. Images attach when the selected model supports vision.

### Agent prompt skill references

Today `/skill` works in the **chat composer** (`resolveChatSubmission` in [`chatCommands/index.ts`](../crates/ui/src/lib/chatCommands/index.ts)): type `/ponytail` (or pick from the combobox), see a description preview, and on send the skill id is recorded in formatted submit text. **Saved agents** and **node inspector** system/task prompts are plain textareas — authors must paste full skill instructions or duplicate prose by hand.

| Layer | Role / gap |
| --- | --- |
| `crates/ui/src/forms/AgentConfigForm.tsx` | System + task prompt fields — no slash combobox or skill preview |
| `crates/ui/src/screens/AgentsScreen.tsx` | Saved-agent editor — same gap |
| `crates/ui/src/lib/chatCommands.ts` | Slash token parse, combobox match, submit resolution — **Done** for composer only |
| `crates/ui/src/components/conversation/SkillCommandCombobox.tsx` | Reusable slash UI — composer-only today |
| `crates/orchestration/src/adapters/storage/skill_store.rs` | Skill discovery + `SkillSummary` — read skill file body for expansion |
| `crates/engine/src/execution/node_invocation.rs` | `AgentRequest` assembly — prompts passed verbatim; no `/skill` expansion |

**Intent:** Write `/ponytail` (or `/openflow-engine-change`) in a system or task prompt instead of copying the whole skill markdown. At **run start** (or first `CallAi` for that node), expand each referenced skill into the assembled system prompt — same discovery roots as Settings skill scan. Persist **tokens** in `agents.json` / workflow JSON; store expanded text only in run snapshots if needed for replay ([#24](#run-checkpoint-history-and-replay)).

| Item | Priority | Status |
| --- | --- | --- |
| Slash UX in prompt fields — reuse combobox + description preview on system/task textareas (Agents screen + node inspector) | High | Planned |
| Token syntax — `/skillId` at line start or inline; unknown ids left literal with editor warning | High | Planned |
| Expansion — resolve skill file content from `skill_store` into prompt assembly; dedupe repeated ids | High | Planned |
| Snapshot policy — expand at run-start into frozen `CallableAgent` / node config snapshot (deterministic reruns) | Medium | Planned |
| Context ledger — list expanded skills in [#18 Context used](#context-used) per turn | Medium | Planned |
| Optional `@` in prompts — same jail as [#15](#attachments--file-references) for `{path}` in static prompts (stretch) | Low | Planned |

**Target:** On the Agents screen, system prompt is `You follow /ponytail and /openflow-engine-change.` — no pasted skill bodies. Start a run → the model receives expanded skill instructions in its system prompt. Inspector shows which skills resolved and which ids were missing.

**Depends on:** skill discovery (`list_skills` / `skill_store`). **Complements:** [#15 Attachments](#attachments--file-references) (composer `/skill`), [#18 Context used](#context-used). **Unlocks:** shorter saved-agent library, composable agent personas without duplication.

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
| `crates/orchestration/src/run/execution/` | Run start does not inject project rules into node system prompts |
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

**Reference:** Submit contract — [`node_invocation.rs`](../crates/engine/src/execution/node_invocation.rs); offline review UX — `tools/plan-review.html`; tool-approval pause pattern — `AwaitToolApproval` in [`interactive_engine`](../crates/engine/src/execution/interactive_engine/mod.rs).

### In-app file viewer from node output

Agents and nodes already read and write files under the execution cwd, and the changed-files panel shows diffs after write-tier tools run. There is no general way for a node to **present** a file in its output — assistant text, submit payload, or handoff manifest — and let the user **click to open** that path inside OpenFlow. Users must open the linked project in an external editor or hunt paths in raw chat text.

This item adds **clickable file references** surfaced by nodes during and after a run, plus an **in-app viewer** (dock tab or overlay panel) for reading those files without leaving the app.

| Layer | Gap |
| --- | --- |
| `crates/ui/src/components/conversation/` | Assistant markdown renders paths as plain text — no file-link detection or click handler |
| `crates/ui/src/panels/` | No file viewer tab/panel; `FileChangesPanel` is diff-only for write-tier mutations |
| `crates/orchestration/src/project/file_refs.rs` | Read-under-jail exists for composer `@` refs — not wired for on-demand viewer open from run output |
| `crates/engine/src/execution/` | Node `output` and transcript have no structured `presented_files: [{ path, label?, line_range? }]` |
| `crates/desktop/src/lib.rs` | No `open_file_in_viewer` IPC (read path under execution cwd, return content + mime hint) |

**Decisions (resolve before coding):**

| ID | Question | Recommendation |
| --- | --- | --- |
| V1 | How are files referenced? | **Dual path:** (1) structured `presented_files` on submit/output when agent uses a builtin or schema field; (2) **auto-link** well-formed repo-relative paths in assistant markdown (`src/foo.rs`, `path:10-40`) under execution cwd jail |
| V2 | Where does the viewer live? | Dock tab **Files** (or reuse Overview sub-panel) — list node-presented files + click history; primary pane shows content with syntax highlight for code, render for markdown |
| V3 | Line ranges | Optional `#L10-L40` or `:10-40` suffix on links; viewer scrolls/highlights range |
| V4 | Scope | Read-only in v1; "Open in editor" external action is stretch. Images: inline preview when path is image mime |
| V5 | Security | Same execution-cwd jail as [#15](#attachments--file-references); reject escapes; cap file size for inline load |

| Item | Priority | Status |
| --- | --- | --- |
| File-link detection — parse repo-relative paths in assistant markdown; render as clickable chips/links | High | Planned |
| Viewer IPC — read file under execution cwd; return content, size, truncated flag | High | Planned |
| In-app viewer panel — dock tab or split pane: path breadcrumb, content, line numbers, markdown/code modes | High | Planned |
| Structured `presented_files` — optional field on node submit/output; merge into viewer file list per node | Medium | Planned |
| Line-range navigation — scroll + highlight when link includes range | Medium | Planned |
| Canvas / node card — show count or primary presented file on completed nodes; click opens viewer | Medium | Planned |
| Handoff integration — primary handoff file from [#17](#node-handoff-artifacts--output-review) opens in same viewer | Medium | Planned |
| Image preview — inline render for png/jpg/gif/webp under size cap | Low | Planned |
| Open externally — reveal in Finder / default app (stretch) | Low | Planned |

**Target:** A planning node finishes and its assistant message says "See `docs/plan.md` for the full spec." You click the link → the file opens in the dock viewer with markdown rendering. An implementer cites `src/lib.rs:42-80` → viewer jumps to that range. No copy-paste into another editor for read-only review.

**Depends on:** execution cwd jail ([#15](#attachments--file-references)), optional [#17](#node-handoff-artifacts--output-review) for handoff paths. **Complements:** [File edit tooling](#file-edit-tooling) (changed-files diffs), [#18 Context used](#context-used) (click path rows to open). **Unlocks:** faster human review of agent-produced artifacts, less context switching during runs.

### Programmatic / non-AI nodes

Workflows today are agent-only: every `NodeKind` is `Agent`, and every node turn goes through `AiPort`. There is no way to run deterministic logic — HTTP calls, JSON transforms, repo file enumeration, or small scripts — as a first-class graph step. Users work around this by asking an LLM to call `bash` or write throwaway tools, which is slow, non-deterministic, and hard to validate on the canvas.

This section covers **programmatic nodes**: graph steps that execute without an LLM turn and emit structured output for downstream agent nodes (or other programmatic nodes).

| Layer | Gap |
| --- | --- |
| `crates/engine/src/graph/workflow.rs` | `NodeKind` is only `Agent`; no `Code`, `Http`, or `Transform` config types |
| `crates/engine/src/execution/interactive_engine/` | Engine assumes every ready node → `CallAi`; no `CallProgrammatic` poll branch |
| `crates/orchestration/src/run/execution/drive.rs` | Drive loop has no programmatic executor hook |
| `crates/orchestration/src/tool/` | Bash exists as an agent tool, not as a sandboxed node runtime with declared inputs/outputs |
| `crates/ui/src/forms/` | Inspector only edits `AgentNodeConfig`; no code editor or HTTP/transform panels |
| `crates/ui/src/canvas/` | Canvas renders all nodes as agent chips — no distinct programmatic node chrome |

**Decisions (resolve before coding):**

| ID | Question | Recommendation |
| --- | --- | --- |
| P1 | Which node kinds in v1? | **Code** (script), **Transform** (JSONata/JQ-style map on upstream output), **Http** (request/response) — ship Code + Transform first; Http can follow or land with [#26 External connectors](#tier-5--power-features) |
| P2 | Code runtime? | **v1:** embedded JS (QuickJS or Deno core) with injected `upstream`, `entrypoint`, `env` (cwd, project path), and a narrow `invoke_workflow` stub (see [#35](#workflow-orchestration--reinvoke)); no raw filesystem/network unless explicitly allowlisted on the node |
| P3 | Output contract? | Same as agent nodes: programmatic nodes **submit output** JSON via engine API; downstream `build_node_input` receives it in `upstream` like any other node completion |
| P4 | Errors? | Structured node failure (permanent) vs retryable (transient HTTP); honor workflow `retry_policy` for retryable kinds; surface in trace like tool errors |
| P5 | Validation? | Pre-run validation ([#10](#tier-3--daily-driver-ux)) checks script syntax, required upstream parents, and declared output schema when present |

**Node kinds (v1 taxonomy):**

| Kind | Executes | Typical use |
| --- | --- | --- |
| **Code** | User script in node config | Enumerate repo files, build entrypoint payload, branch on upstream JSON, call `invoke_workflow` in a loop |
| **Transform** | Declarative map (no Turing-complete script required) | Pick fields from upstream, merge arrays, template strings into entrypoint shape |
| **Http** | Outbound HTTP with typed response mapping | Webhooks, REST fetches, callback to external orchestrator |

| Item | Priority | Status |
| --- | --- | --- |
| `NodeKind` + config structs — `CodeNodeConfig`, `TransformNodeConfig` in engine; serde + validation | High | Planned |
| Engine poll branch — `CallProgrammatic` instead of `CallAi` for non-agent nodes; submit-output path shared with agents | High | Planned |
| Programmatic executor port — orchestration adapter runs script/transform with timeout, memory cap, cwd jail | High | Planned |
| Canvas + inspector — distinct node shape, Monaco (or embedded) editor for Code, transform field picker | High | Planned |
| Pre-run validation hooks — syntax check, upstream completeness, output schema | Medium | Planned |
| Http node kind — request builder, response → JSON output, retry on 5xx | Medium | Planned |
| Headless acceptance — workflow with Code → Agent chain in `workflow_acceptance.rs` | High | Planned |

**Target:** Place a **Code** node before an agent node. The script lists `src/**/*.rs`, builds `{ "files": [...] }`, and downstream agents receive that JSON in `upstream` without an LLM call. Complex control flow (loops, conditionals) lives in code nodes instead of prompt hacks.

**Depends on:** [#5 Node completion](#node-completion) (submit contract). **Unlocks:** [#35 Workflow orchestration](#workflow-orchestration--reinvoke) (scripts call `invoke_workflow`), batch repo processing, cheaper DAG segments, [#26 External connectors](#tier-5--power-features) (Http node overlap).

**Reference:** Submit contract — [`node_invocation.rs`](../crates/engine/src/execution/node_invocation.rs); eval-tool stdin RPC pattern — eval tool plan (`docs/superpowers/plans/2026-06-15-eval-tool.md`); `NodeKind` — [`workflow.rs`](../crates/engine/src/graph/workflow.rs).

### Workflow orchestration & reinvoke

Runs today are flat: one workflow, one `start_run`, one engine instance. A user cannot **re-run part of a graph** (a single node or a subgraph) without restarting the whole workflow, nor **spawn child workflow runs** from inside a parent (e.g. "for each file in the repo, run the review workflow"). There is no **in-app scripting surface** to start runs programmatically — only UI and IPC `start_run`. Checkpoint replay ([#24](#run-checkpoint-history-and-replay)) covers fork-from-checkpoint for humans, not dynamic in-run orchestration or batch fan-out.

| Layer | Gap |
| --- | --- |
| `crates/orchestration/src/run/coordinator/mod.rs` | Single active run per session focus; no parent/child run registry or `invoke_workflow(workflow_id, entrypoint)` API |
| `crates/engine/src/execution/interactive_engine/` | No "jump to node" or "re-execute subgraph" without resetting completed upstream state |
| `crates/orchestration/src/run/execution/drive.rs` | No wait-for-child-runs poll step; no aggregation of child outputs into parent node output |
| `crates/desktop/src/lib.rs` | No batch `start_runs` or scripting IPC; no `reinvoke_from_node` |
| `crates/ui/src/` | No child-run monitor, foreach progress, or "re-run from here" on canvas during/after a run |
| Project scripts | No `.flow/scripts/` discovery or REPL to chain workflows |

**Decisions (resolve before coding):**

| ID | Question | Recommendation |
| --- | --- | --- |
| O1 | Child run model? | Each `invoke_workflow` creates a **new run record** (child `run_id`) with `parent_run_id` + `parent_node_id`; child gets its own engine instance and artifact dir under `{project}/.flow/runs/{child_run_id}/` |
| O2 | Parent wait semantics? | **Sync (v1):** programmatic node blocks until all child runs terminal (completed/failed/stopped); emits aggregated output `{ "children": [ { "run_id", "status", "output" } ] }`. **Async (v2):** fire-and-forget with polling builtin |
| O3 | Foreach / batch pattern? | **Dedicated `ForEach` programmatic node** *or* Code node calling `invoke_workflow` in a loop — start with Code + API; add `ForEach` node when UX demands canvas-native glob + concurrency controls |
| O4 | Concurrency cap? | Workflow setting `max_concurrent_child_runs` (default 3); queue excess children; surface in run trace |
| O5 | Partial reinvoke scope? | **In-run retry node** — re-execute node N and downstream only, preserving upstream outputs (extends checkpoint "replay from node" to live runs). **Subgraph reinvoke** — select node set on canvas, "Run selection" with fresh entrypoint. Distinct from full workflow restart |
| O6 | In-app scripting? | **v1:** project scripts at `{project}/.flow/scripts/*.ts` (or `.js`) calling desktop seam commands (`startRun`, `waitForRun`, `invokeWorkflow`); run from Schedule sidebar or a Scripts panel. **v2:** workflow-embedded script library |
| O7 | Entrypoint per child? | Child entrypoint is JSON merged from parent: `{ "item": <foreach element>, "parent": <parent node output>, "entrypoint": <root entrypoint text> }` |

**Orchestration patterns (v1):**

| Pattern | Mechanism | Example |
| --- | --- | --- |
| **Batch over repo files** | Code node globs paths → loop `invoke_workflow(review_wf_id, { item: { path } })` | Lint every `*.rs` file with a small review workflow |
| **Map-reduce** | ForEach or Code spawns N children; downstream Agent node reads aggregated `children` output | Parallel doc generation per module |
| **Partial re-run** | `reinvoke_from_node(node_id)` after editing workflow or fixing env | Re-run implementer only after plan approval |
| **External driver** | `.flow/scripts/nightly.ts` starts runs on a timer (complements [#23 Cron](#tier-5--power-features)) | CI-style regression workflow battery |

| Item | Priority | Status |
| --- | --- | --- |
| `invoke_workflow` API — orchestration + desktop IPC; child run metadata (`parent_run_id`, `parent_node_id`) | High | Planned |
| Child run registry — list/watch children from parent; terminal aggregation into parent programmatic node output | High | Planned |
| `reinvoke_from_node` — reset node N + descendants, preserve upstream outputs; engine + coordinator support | High | Planned |
| Code node `invoke_workflow` binding — sandboxed callable from [#25](#programmatic--non-ai-nodes) scripts | High | Planned |
| Concurrency limit + queue — `max_concurrent_child_runs` on `WorkflowSettings` | Medium | Planned |
| ForEach node (optional) — glob/JSON-array input, native progress UI, concurrency slider | Medium | Planned |
| `.flow/scripts/` — discover, edit, run project scripts; `startRun` / `waitForRun` / `invokeWorkflow` in script host | Medium | Planned |
| Canvas "Re-run from here" — context menu on completed node; subgraph selection run | Medium | Planned |
| Child run UI — nested run list, per-child status, jump to child trace | Medium | Planned |
| Headless acceptance — parent Code node spawns two child runs, parent Agent reads aggregate | High | Planned |

**Target:** A workflow with a **Code** node iterates `git ls-files '*.md'`, calls `invoke_workflow` for each path, and a downstream agent summarizes all child outputs. A project script in `.flow/scripts/` can start the same workflow ten times with different entrypoints without clicking Run. After a failed node, choose **Re-run from here** to re-execute that node and downstream without replaying the whole DAG.

**Depends on:** [#24 Run checkpoint & replay](#run-checkpoint-history-and-replay) (durable child run records), [#25 Programmatic nodes](#programmatic--non-ai-nodes) (Code node + `invoke_workflow`), [#6 Run lifecycle](#run-lifecycle) (artifact layout). **Unlocks:** repo-wide refactors, workflow test batteries, CI-style automation inside the app, promoted background multi-run orchestration.

**Reference:** Run coordinator — [`coordinator.rs`](../crates/orchestration/src/run/coordinator/mod.rs); checkpoint fork — [#24](#run-checkpoint-history-and-replay) "Replay from node"; schedule loop — schedule sidebar plan (`docs/superpowers/plans/2026-06-16-schedule-sidebar.md`).

---

## Refactor

Structural cleanup by workspace section. Keep engine semantics in `engine`, provider transport in `providers`, runtime and persistence in `orchestration`, Tauri IPC in `desktop`, and frontend interaction in `ui`. See `docs/architecture/contract.md`.

**Serde casing:** Engine persistence uses `snake_case`; IPC/UI DTOs use `camelCase`. Legacy `PascalCase` enum values and field aliases (`#[serde(alias = …)]`) remain for older saved workflows, run logs, and agent definitions. Unify on one convention (T16), then drop the old snake_case ↔ camelCase / PascalCase compatibility shims.

### Engine (`crates/engine`)

| Item | Status |
| --- | --- |
| Vocabulary-aligned module tree (`graph/`, `template/`, `execution/`, `conversation/`, `tools/`, `ports/`) | Done |
| Shared `node_invocation` for desktop and headless runs | Done |
| `subagent_runtime`, `CallableAgent`, canonical `RunTelemetry` | Done |
| Remove unused port scaffolding; typed template errors; remove `InteractiveEngine::poll` | Done |
| Collapse `model::NodeTemplate` vs `template::Template` (T2) | Done |
| Node lookup index — `HashMap<NodeId, usize>` (T3) | Done |
| Remove unused inbound port scaffolding; use inherent resume methods (T14) | Done |
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
| Provider error taxonomy aligned with engine `AgentError` (T1) | Planned |

### Orchestration (`crates/orchestration`)

| Item | Status |
| --- | --- |
| Thin `AppBackend` — catalog modules, `api.rs`, `error.rs` | Done |
| `run/execution/` split (`drive`, `events`, `headless`, `subagents`) | Done |
| Move `FileTemplateStore` to orchestration; alias `ExecutionEvent` → `RunTelemetry` | Done |
| Typed `BackendError`; `spawn_blocking` tool I/O; dead-code removal | Done |
| Unify on one Tokio runtime — `AppBackend` takes injected `Handle` | Done |
| Tool runner error taxonomy + retry loop (T19–T20) | Done |
| Tool hook registration in composition root (T19 follow-up) | Planned |
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
| UI desktop wrappers in `api.ts` | Done |
| Reusable sidebar primitives; shared Agents screen list rows | Done |
| Run stop button + `stopRun` IPC wiring | Done |
| Slim `AppProvider` — extract run listeners, zoom, dock resize into hooks/modules | Planned |
| Typed run-state selectors — reduce `AppContext` surface | Planned |
| Canvas host boundary — keep React Flow isolated from Solid app state | Planned |
| Component tests colocated with `conversation/`, `sidebar/` modules | Done |

**Target:** Each crate has one obvious entry point; cross-crate seams match `AGENTS.md` boundary table; no dead modules or duplicate DTOs between orchestration and UI.

---

## Engine hardening

Remediation for modeled-but-unwired behavior and correctness gaps in `crates/engine`. Full task specs (files, acceptance, guardrails) lived in the prior remediation plan; phases below are the execution order. These tasks are sequenced into the queue above (items #3–#5, #31–#32).

### Decisions (resolve before coding)

| ID | Question | Recommendation |
| --- | --- | --- |
| D1 Templates | `model::NodeTemplate` vs `template::Template` — which is canonical? | Keep `template::Template`; persist it in `FileTemplateStore` |
| D2 `available_tools` | Engine resolves tool names, or adapter owns registry? | Confirm against provider crate; document if adapter-owned |
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
| T20 Tool invocation retry | P0 | Honor `retry_policy` in `tool_port.rs` with backoff before surfacing tool errors — Done |
| T21 Resilient tool failure path | P0 | Failed tools → transcript → `CallAi`; no `ExecutionEvent::Error` / drive exit for tool failures — Done |
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
| T14 Remove inbound port scaffolding | P2 | Done — use `InteractiveEngine::on_human_input` / `on_tool_decision`; add traits only if a real consumer is typed on them |

### Phase 4 — Cleanup

| Task | Severity | Summary |
| --- | --- | --- |
| T15 Fix hexagonal file placement | P2 | Move `ScriptedAiAdapter` to outbound |
| T16 Unify serde casing + typo fix | P2 | Wire-format change; keep back-compat aliases |
| T16b Remove legacy casing shims | P2 | Drop `#[serde(alias = …)]` and PascalCase enum accepts after T16 migration |
| T17 Concurrent layer siblings (runner only) | Stretch | Per D3; after phases 1–3 |
| T18 Trim blanket clippy allows | P2 | Last; `clippy -- -D warnings` clean |
