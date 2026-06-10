# Roadmap

Prioritized work across product features, orchestration lifecycle, and domain engine hardening.

**Status:** Done · In progress · Planned

---

## Near term

### Wire entrypoint text through the desktop run path

Domain supports entrypoint injection (`run_with_entrypoint` → `InteractiveEngine` → `build_node_input`), but the desktop app never passes it.

| Layer | Gap |
| --- | --- |
| `crates/desktop/src/lib.rs` | `start_run` hardcodes `None` for entrypoint |
| `crates/ui/src/api.ts` | `startRun` does not accept or send entrypoint |
| `crates/orchestration/src/backend.rs` | `start_run` does not accept or forward entrypoint text |

**Target:** User entrypoint text from the UI reaches root agent nodes as `{ "entrypoint": { "text": "..." }, "upstream": [] }` in `AgentRequest.input`.

**Reference:** Domain test `injects_entrypoint_into_root_node_input_only` in `crates/domain/src/runner.rs`.

### Run lifecycle

| Item | Priority | Status |
| --- | --- | --- |
| `stop_run` command — abort execution handle, clear channels, mark run inactive | High | Done |
| Wire stop/cancel to UI (stop button during active runs) | High | Done |
| Handle window close (`CloseRequested`) — abort active run before exit | High | Done |
| Graceful shutdown — cancel in-flight AI calls and tool subprocesses on close | Medium | Done |
| User stop shows `stopped` on canvas, overview, and trace (not `failed`) | Medium | Done |
| Clean up temp artifact dirs (`openflow-run-*`) on completion or abort | Medium | Planned |
| Store event bridge task handle for independent cancellation | Medium | Planned |
| Unify on one Tokio runtime — see `docs/architecture/threading-concurrency.md` | Medium | Planned |
| Decide and document run persistence policy (dies with app vs. resume after restart) | Medium | Planned |
| Warn on close when workflows have unsaved changes | Low | Planned |
| Warn on close when a run is still active | Low | Planned |

**Deferred** until cron, retry loops, and repo workflows land: background job start/stop/resume, multi-run orchestration.

### Provider API key storage — plaintext in settings

| Item | Priority | Status |
| --- | --- | --- |
| Persist keys in `settings.json` (`ProviderProfile.api_key`) | High | Done |
| Settings UI plaintext risk notice | High | Done |
| Env var fallback unchanged | High | Done |

### Tool invocation retry and resilience

Today a failed tool call becomes a single `is_error: true` [`ToolResult`](crates/domain/src/tools/config.rs) fed back to the model. [`retry_policy`](crates/domain/src/graph/workflow.rs) (T6) applies only to transient **AI** [`AgentError`](crates/domain/src/ports/outbound.rs), not tool-runner failures. The drive loop can still **exit the run** on orchestration/engine mismatches (`on_tool_results` error → `ExecutionEvent::Error`) or on AI invoke failure after retries.

| Layer | Gap |
| --- | --- |
| `crates/orchestration/src/tools/runner.rs` | `ToolRunnerError` has no transient/permanent classification; no retry/backoff |
| `crates/orchestration/src/execution/drive.rs` | Tool execute fails once → `denied()` result; no retry loop; handler errors abort drive |
| `crates/domain/src/graph/workflow.rs` | `RetryPolicy` is AI-oriented; no tool-specific retry knobs |
| `crates/domain/src/execution/interactive_engine.rs` | Tool errors in transcript do not increment AI retry counters; run should continue |

**Target behavior:**

1. Classify tool failures as retryable (timeout, rate limit, transient I/O) vs permanent (bad args, missing file, policy deny).
2. Retry retryable tool invocations per workflow/node policy (`max_attempts`, `backoff_ms`) **before** surfacing an error result to the model.
3. On exhausted retries or permanent failure, append `is_error: true` tool result and **resume the agent loop** (`CallAi`) — do not terminate the run or crash the host.
4. Reserve run-level failure for unrecoverable host errors (engine state corruption, cancelled run), not individual tool calls.

**Depends on:** T1 (error taxonomy pattern), T6 (retry policy wiring). See T19–T21 in domain hardening.

### Chat presentation — thinking bubbles & tool cleanup

Assistant token streaming is wired (`ChatMessageDelta` → chat log). Next chat polish: show provider reasoning as first-class thinking bubbles and replace always-expanded tool panes with compact, expandable rows.

| Item | Priority | Status |
| --- | --- | --- |
| Thinking bubble UI — collapsible reasoning block in chat; distinct from assistant messages; collapsed by default | High | Planned |
| Provider thinking in transcript — parse reasoning blocks from Anthropic/OpenAI responses; project to chat (not legacy `ChatRole::Thinking` tool lines) | High | Planned |
| Collapsible tool bubbles — collapsed row shows tool name + one-line outcome; expand for args and full output | High | Planned |
| Pretty tool names — human-readable labels in chat (e.g. Read, Search, Edit file) instead of raw builtin ids (`read`, `ast_grep`, `openflow_call_subagent`) | Medium | Planned |
| Tool row chrome — drop `Tool Invocation:` header; status chip (running / completed / failed); chevron expand | Medium | Planned |
| Args summary — one-line path/query preview when collapsed; full formatted JSON only when expanded | Medium | Planned |
| Streaming thinking — append reasoning tokens into the thinking bubble during active turns | Medium | Planned |

**Reference:** [`ToolBubble.tsx`](crates/ui/src/components/conversation/ToolBubble.tsx) (always expanded today); full spec in [Thinking & chat presentation](#thinking--chat-presentation).

---

## Product features

| Feature | Status | Notes |
| --- | --- | --- |
| Workflow settings (`shared_context`, schedule/retry/provider schema) | Done | Gear panel in editor |
| Subagent integration — list on agent node, node settings picker | Done | Saved + ad-hoc subagents |
| Canvas subagent list — scrollable in-node list (no truncate) | Planned | See [Canvas run feedback](#canvas-run-feedback) |
| Canvas node status icons — colored icons per state (thinking, done, etc.) | Planned | See [Canvas run feedback](#canvas-run-feedback) |
| Callable agents (`openflow_call_subagent`) | Done | Snapshotted at run start |
| Project-backed workflows (`.flow/workflows/`) | Done | Sidebar project groups |
| Skill invocation | Done | Invoke path works |
| Skill discovery settings — unified skills section in Settings | Planned | Currently scans Cursor, Codex, Claude roots |
| Show skill description above invoke UI | Done | `SkillDescriptionPreview` above composer when `/skill` tokens are present |
| File references — attach project files to chat and entrypoint input | Planned | See [File references](#file-references) |
| Project rules — per-linked-project agent guidance | Planned | See [Project rules](#project-rules) |
| Branching — nodes wait for all upstream outputs before continuing | Planned | |
| MCP integration | Planned | |
| Remove `Context:` / `Task:` labels from chat | Planned | |
| Cron / scheduled workflow runs | Planned | Schema field exists; execution TBD |
| Retry loop (workflow-level automation) | Planned | Schema field exists; execution TBD |
| Tool invocation retry | Planned | Retry transient tool failures before surfacing to model; see near-term section |
| Resilient tool failure handling | Planned | Failed tool calls feed transcript and continue agent loop; no run abort/crash |
| Error logging stored locally; agent loop to propose fixes | Planned | |
| File edit tooling — read/write-tier builtins, approval, diff preview, changed-files ledger | Done | See [File edit tooling](#file-edit-tooling) |
| Remove per-node JSON output schema editing | Planned | Overkill for current scope; keep internal defaults, drop inspector/agents UI |
| Pass read files to downstream nodes | Planned | See [Upstream read-file context](#upstream-read-file-context) |
| Natural language workflow definition | Planned | |
| Standalone macOS app packaging | Planned | |
| Workflow version control (per-change revert) | Planned | |
| Run persistence, history, and replay | Planned | |
| Programmatic / non-AI nodes (API nodes) | Planned | |
| Thinking level per node | Planned | See [Thinking & chat presentation](#thinking--chat-presentation) |
| Thinking bubbles in chat UI | Planned | Collapsible provider reasoning; near-term [Chat presentation](#chat-presentation--thinking-bubbles--tool-cleanup) |
| Tool invocation display cleanup | Planned | Compact collapsed rows; expand for args/output; near-term [Chat presentation](#chat-presentation--thinking-bubbles--tool-cleanup) |
| Pretty tool names in chat | Planned | Human-readable labels for builtins and subagents; near-term [Chat presentation](#chat-presentation--thinking-bubbles--tool-cleanup) |
| Terminal tab in bottom dock panel | Planned | Interactive shell alongside Overview, Chat, Run trace |
| Chat bar markdown rendering | Planned | |
| System-level notifications | Planned | |
| Agent questions & todos — in-run UI | Planned | See [Agent questions & todos](#agent-questions--todos) |
| Queued chat input during active runs | Planned | See [Agent questions & todos](#agent-questions--todos) |
| Global chat — unified pane across node progression | Planned | See [Global chat](#global-chat) |
| Composio / n8n-style external node connectors | Planned | |
| Accessibility & keyboard shortcuts | Planned | See [Accessibility](#accessibility) |

### Canvas run feedback

During a run, agent nodes show a status row and optional subagent rows. Subagents are capped at three visible entries with a `+N more` overflow line; status is a colored dot plus text label (`WorkflowNode.react.tsx`, `agentStatus.ts`).

| Layer | Gap |
| --- | --- |
| `crates/ui/src/canvas/WorkflowNode.react.tsx` | `MAX_VISIBLE_SUBAGENTS = 3` truncates the list; no scroll container |
| `crates/ui/src/styles/index.css` | `.node-subagent-list` is static; no max-height / overflow-y |
| `crates/ui/src/canvas/WorkflowNode.react.tsx` | Status is dot + text only — no distinct icon per `AgentStatus` |
| `crates/ui/src/lib/agentStatus.ts` | Labels only; no icon or color token mapping for canvas chrome |

| Item | Priority | Status |
| --- | --- | --- |
| Scrollable subagent list — show all in-run subagents inside the node; max-height + `overflow-y: auto`; drop `+N more` truncation | High | Planned |
| Subagent row polish — keep status dot + name; optional purpose tooltip; readable at small node widths | Medium | Planned |
| Status icons — replace or augment the dot with a distinct colored icon per state (thinking, waiting for input, awaiting approval, running tool, done, failed, stopped) | High | Planned |
| Icon + label pairing — icon at a glance; text label on hover or when node is selected / zoomed in | Medium | Planned |
| Handle chrome — match icon color on left/right handles for quick scan across the graph | Low | Planned |

**Target:** Glance at the canvas and tell what each node is doing from icon color and shape. Open a busy agent node and scroll its full subagent roster without losing entries behind a `+N more` line.

### Accessibility

Keyboard QoL exists for run, save, delete, and zoom (`AppProvider` global handler; see README). Panel chrome (sidebar hide, dock max/collapse) is mouse/drag only.

| Item | Priority | Status |
| --- | --- | --- |
| Sidebar hide/show — macOS hide control + keyboard shortcut | Medium | Planned |
| Bottom dock — maximize height / collapse + keyboard shortcut | Medium | Planned |
| Inspector and workflow settings — toggle via keyboard | Low | Planned |
| Shortcut reference — Settings panel or `?` overlay | Low | Planned |
| Focus management — modals, dock tabs, sidebar nav | Medium | Planned |
| Canvas and run status — screen-reader labels | Low | Planned |

**Target:** Every primary panel toggle (sidebar, dock, inspector) has a documented shortcut; shortcuts skip when focus is in a text field.

### Thinking & chat presentation

Providers expose extended reasoning (Anthropic thinking blocks, OpenAI reasoning effort, etc.), but the app has no per-node knob and no first-class UI for model reasoning. `ChatRole::Thinking` today is reused for legacy tool-line parsing and pause context — not provider reasoning. Tool bubbles always show full output in a fixed-height scroll region.

| Layer | Gap |
| --- | --- |
| `crates/domain/src/graph/workflow.rs` | No `thinking_level` (or budget) on `AgentNodeConfig` / `CallableAgent` |
| `crates/domain/src/ports/outbound.rs` | `AgentRequest` has no thinking/reasoning field for adapters |
| `crates/providers/src/` | Wire payloads omit provider-specific reasoning params; responses do not parse thinking blocks into transcript items |
| `crates/domain/src/conversation/mod.rs` | No dedicated transcript item for provider reasoning (distinct from `ChatRole::Thinking` log lines) |
| `crates/orchestration/src/execution/events.rs` | Run projection does not emit structured thinking events to chat |
| `crates/ui/src/forms/` | Inspector has no thinking-level control (off / low / medium / high or provider-aligned presets) |
| `crates/ui/src/components/conversation/` | No collapsible thinking block component; `PlainMessage` renders thinking role like assistant text |
| `crates/ui/src/components/conversation/ToolBubble.tsx` | Always expanded fixed-height scroll pane; `Tool Invocation:` header; raw builtin ids (`read`, `openflow_call_subagent`) with no display-name mapping |
| `crates/ui/src/components/conversation/ConversationMessages.tsx` | No `ThinkingBubble`; tool markers and legacy thinking lines share the same bubble path |
| `crates/ui/src/lib/parseLegacyToolMessages.ts` | Legacy `ChatRole::Thinking` grouped as tool bubbles — conflates provider reasoning with tool I/O |

| Item | Priority | Status |
| --- | --- | --- |
| Thinking level schema — `thinking_level` on agent node + saved agent; optional workflow default | High | Planned |
| Inspector control — pick thinking level per node; inherit workflow default when unset | High | Planned |
| Provider wiring — map level to Anthropic/OpenAI-compat reasoning params; parse thinking blocks from responses | High | Planned |
| Thinking transcript items — `AgentTranscriptItem::ReasoningBlock` (or equivalent) in domain + run projection | High | Planned |
| `ThinkingBubble` component — collapsible reasoning bubble; muted styling; collapsed by default | High | Planned |
| Collapsible tool bubbles — collapsed row shows tool name + one-line outcome; expand for args and full output | High | Planned |
| Pretty tool names — map builtin/subagent ids to short human labels in `ToolBubble`, `ToolApprovalCard`, and trace rows | Medium | Planned |
| Tool row chrome — icon + name + status chip; remove `Tool Invocation:` label; chevron toggle | Medium | Planned |
| Args one-liner — path/query/file summary when collapsed; `prettyJson` args only when expanded | Medium | Planned |
| Streaming thinking — append reasoning tokens into the thinking bubble during active turns | Medium | Planned |
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
| Unified transcript — merge all node `chatLogs` into one scrollable pane for the active run | High | Planned |
| Persist on progression — keep showing prior nodes' messages as the run advances; do not clear or hide when focus moves | High | Planned |
| Execution-layer ordering — stack messages by DAG depth: earlier layer on top, later layer below (node 1 text above node 2, etc.) | High | Planned |
| Node attribution — label or chrome per segment so users know which agent spoke | Medium | Planned |
| Parallel reply bubbles — when two+ nodes at the same layer await input, show separate composer targets (one bubble per awaiting node) | High | Planned |
| Route submit to correct node — `submit_user_input` keyed by target node id from the reply bubble, not canvas selection | High | Planned |
| Optional canvas sync — selecting a node scrolls global chat to that node's segment (highlight only; pane stays unified) | Low | Planned |
| Run start / entrypoint — global pane shows entrypoint user message at top before first node output | Medium | Planned |

**Target:** One continuous chat pane for the whole run. As nodes complete and downstream nodes start, earlier conversation stays visible in layer order. When parallel nodes at the same depth pause for input, each gets its own reply bubble in the composer area so you can answer both without switching canvas selection.

### File references

Users can invoke skills with `/skill` tokens in the chat composer (`crates/ui/src/lib/chatCommands.ts`), but there is no way to attach project files to a message. Agents must discover files via read-tier tools instead of receiving user-selected context up front.

| Layer | Gap |
| --- | --- |
| `crates/ui/src/lib/chatCommands.ts` | Resolves `/` skill tokens only; no `@` path tokens or referenced-file list |
| `crates/ui/src/components/conversation/` | No file picker combobox, reference pills, or content preview above composer |
| `crates/ui/src/api.ts` / `crates/desktop/src/lib.rs` | `submit_user_input` and `start_run` accept plain `text` only — no structured file refs |
| `crates/orchestration/src/run/coordinator.rs` | No read-and-resolve step for referenced paths under execution cwd |
| `crates/engine/src/execution/interactive_engine.rs` | `on_user_input` records a single string; no `referenced_files` block in transcript or node input |
| `crates/engine/src/execution/node_invocation.rs` | `entrypoint` is `{ "text": "..." }` only — no attached file payloads |

| Item | Priority | Status |
| --- | --- | --- |
| `@` token UX — combobox over linked-project files (reuse skill combobox pattern); optional browse dialog | High | Planned |
| Reference resolution — read file content under execution cwd on submit; reject paths outside project jail | High | Planned |
| Structured submit payload — `referenced_files: [{ path, content \| excerpt }]` alongside message text | High | Planned |
| Transcript shape — persist references in `AgentTranscriptItem::UserMessage` and chat log projection | Medium | Planned |
| Composer chrome — pills for attached paths; expandable preview (path + line range + size cap) | Medium | Planned |
| Entrypoint attachments — same reference model on run start (with entrypoint wiring) | Medium | Planned |
| Line-range refs — `@path:10-40` or selection-from-editor hook | Low | Planned |
| Reference budget — max files, max bytes, truncate with notice in formatted submit text | Low | Planned |

**Target:** Type `@` in the composer (or pick files before run) to attach project paths. Resolved content is injected into the user message or entrypoint JSON so the agent sees explicit file context without an extra `read` tool round.

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
| `crates/orchestration/src/tool/registry.rs` | Builtin catalog — read tier: `read`, `search`, `find`, `ast_grep`; write tier: `write`, `edit`, `apply_patch` |
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
| Collapse `model::NodeTemplate` vs `template::Template` (T2) | Planned |
| Node lookup index — `HashMap<NodeId, usize>` (T3) | Planned |
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
| Unify on one Tokio runtime — see near-term run lifecycle | Planned |
| Tool runner error taxonomy + retry loop (T19–T20) | Planned |
| `RunCoordinator` / session lifecycle — stop handle, channel cleanup | Done |
| Store catalog split audit — merge overlapping workflow/project helpers | Planned |

### Desktop (`crates/desktop`)

| Item | Status |
| --- | --- |
| Thin Tauri adapter — commands delegate to orchestration; event bridge only | Done |
| Remove unused port/adapter scaffolding | Done |
| Wire entrypoint through `start_run` IPC | Planned |
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
| Component tests colocated with `conversation/`, `sidebar/` modules | Planned |

**Target:** Each crate has one obvious entry point; cross-crate seams match `AGENTS.md` boundary table; no dead modules or duplicate DTOs between orchestration and UI.

---

## Domain engine hardening

Remediation for modeled-but-unwired behavior and correctness gaps in `crates/domain`. Full task specs (files, acceptance, guardrails) lived in the prior remediation plan; phases below are the execution order.

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
| T1 Error taxonomy on `AgentError` | P0 | `Transient` / `Permanent` / `Failed`; `is_retryable()` for retry logic |
| T2 Collapse template systems | P0 | Single canonical type per D1 |
| T3 Node lookup index | P1 | `HashMap<NodeId, usize>` in engine; drop O(n²) scans |

### Phase 2 — Functional gaps

| Task | Severity | Summary |
| --- | --- | --- |
| T4 Wire tool-approval policy | P0 | Honor `ApprovalMode`, `ToolTier`, `ToolPolicy` in engine — Done |
| T5 Tool deny / decision resume | P0 | `on_tool_decision`, `approval_id` on `AwaitToolApproval` |
| T6 Implement `retry_policy` | P0 | Retry transient **AI** failures per node; needs T1 |
| T19 Tool error taxonomy | P0 | `Transient` / `Permanent` on `ToolError` / `ToolRunnerError`; `is_retryable()` |
| T20 Tool invocation retry | P0 | Honor `retry_policy` (or tool-specific override) in `drive.rs` before `ToolCompleted` error |
| T21 Resilient tool failure path | P0 | Failed tools → transcript → `CallAi`; no `ExecutionEvent::Error` / drive exit for tool failures |
| T7 Node-local max-tool-rounds failure | Optional | Only if D4 says so |
| T8 Resolve `available_tools` | P1 | Populate or document per D2 |
| T9 Apply `filter_tool_turn_assistant_message` | P1 | Strip redundant tool-call XML from transcripts |

**High-value P0 path:** T1 → T2 → T4 → T5 → T6 → T19 → T20 → T21 → T9 → T10.

### Phase 3 — Correctness and consistency

| Task | Severity | Summary |
| --- | --- | --- |
| T10 Validate node id in `on_ai_complete` | P1 | Reject misrouted completions |
| T11 Fix run-event semantics | P2 | Emit `Started` at `CallAi`; remove provider branding |
| T12 Surface template store persistence errors | P1 | Return `Result` from store mutations |
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

---

## Suggested execution order

1. ~~Entrypoint wiring + run lifecycle (stop/cancel/shutdown)~~ — stop/cancel/shutdown done; entrypoint wiring remains
2. Chat presentation — thinking bubbles + collapsible tool rows (near-term section)
3. Domain P0 path (T1–T6, T19–T21, T9, T10) — includes tool retry and resilient failure handling
4. Product: branching join semantics, MCP, cron/retry execution
5. Domain polish (T11–T18) and remaining product features
6. Refactor polish — `AppProvider` slim-down, desktop IPC DTOs, orchestration catalog audit
