
# Tools reference

Agents call **tools** during a workflow run. Built-in repository tools are registered in `crates/orchestration/src/tool/registry.rs`. **Harness** tools (`openflow_submit_node_output`, `openflow_request_user_input`, `openflow_ask_user_question`) control node completion and human pauses. **MCP** tools are added at run start from **Settings → MCP Servers** and appear as `mcp/<server>/<tool>`.

Paths are relative to the run **execution folder** (the selected project's configured folder, or an isolated app-managed workspace when no project is selected). Approval behavior depends on the node **Approval mode** in the inspector; see [`../guides/using-the-app.md#tools-and-approval`](../guides/using-the-app.md#tools-and-approval).

Many tools accept optional `_i` (intent): short text shown in the chat UI; the implementation ignores it.

| Tier | Typical approval (Write mode) |
| --- | --- |
| Read | Auto-allow |
| Write | Prompt in chat |

In **Read only** approval mode, write-tier and `bash` are not offered to the model. In **Plan → Execute** planning phase, `bash`, MCP, subagent calls, and most writes are blocked; see [`openflow_write_plan_artifact`](#openflow_write_plan_artifact).

## Quick index

| Tool | Chat label | Tier |
| --- | --- | --- |
| [`read`](#read) | Read File | Read |
| [`search`](#search) | Search Files | Read |
| [`find`](#find) | Search Folders | Read |
| [`ast_grep`](#ast_grep) | AST Search | Read |
| [`ast_edit`](#ast_edit) | AST Edit | Write |
| [`web_search`](#web_search) | (web) | Read |
| [`openflow_update_todo_list`](#openflow_update_todo_list) | Update Progress | Read |
| [`write`](#write) | Write File | Write |
| [`edit`](#edit) | Edit File | Write |
| [`apply_patch`](#apply_patch) | Apply Patch | Write |
| [`bash`](#bash) | Run Command | Write |
| [`openflow_write_plan_artifact`](#openflow_write_plan_artifact) | Seal Plan | Write |
| [`openflow_declare_subagents`](#openflow_declare_subagents) | Declare Subagents | Write |
| [`openflow_call_subagent`](#openflow_call_subagent) | Call Subagent | Write |
| [`openflow_submit_node_output`](#openflow_submit_node_output) | Submit Output | Harness |
| [`openflow_request_user_input`](#openflow_request_user_input) | Request Input | Harness |
| [`openflow_ask_user_question`](#openflow_ask_user_question) | Ask Question | Harness |
| [MCP tools](#mcp-tools) | varies | Write |
| [Authoring tools](#workflow-authoring-tools-build-with-ai) | varies | Authoring session only |

---

## `read`

**Purpose:** Load file contents, list a directory, fetch HTTP(S), or read spilled tool output.

**When to use:** Before edits; to inspect upstream artifacts; after truncation (`artifact:{id}`).

**Key arguments:** `path` — repository-relative path, URL, or `artifact:{id}`. Append `:start-end` for a line range (e.g. `src/lib.rs:10-20`) or `:raw` for unnumbered full content.

**Limits:** Default output is numbered lines, capped at 3000 lines.

**Approval:** Read tier.

---

## `search`

**Purpose:** Ripgrep-style regex search over file contents (Rust regex syntax; no backreferences or lookaround).

**When to use:** Find symbols or strings across the tree. Prefer [`ast_grep`](#ast_grep) for syntax-shaped queries.

**Key arguments:** `pattern` (required), `paths` (required, string or array), optional `i` (case insensitive), optional `gitignore` (default true).

**Limits:** At most 500 matches — narrow `pattern` or `paths` if capped.

**Approval:** Read tier.

---

## `find`

**Purpose:** Glob files and directories (e.g. `**/*.rs`, `src/**/*.ts`).

**When to use:** Discover paths before `read` or `search`.

**Key arguments:** `paths` (required) — glob or array of globs.

**Limits:** At most 200 paths.

**Approval:** Read tier.

---

## `ast_grep`

**Purpose:** Structural code search via ast-grep (`$VAR` metavariables).

**When to use:** Match syntax trees instead of raw text.

**Key arguments:** `pat` (required), `paths` (required array).

**Approval:** Read tier.

---

## `ast_edit`

**Purpose:** Rewrite code structurally through the `ast-grep` CLI.

**When to use:** Codemods and structural multi-file rewrites. Prefer [`edit`](#edit) for one-off local changes.

**Key arguments:** `ops` (required, non-empty array of `{ "pat", "out" }`) and `paths` (required, non-empty array of files, directories, or globs under the execution folder). An empty `out` deletes each match.

**Behavior:** Ops run sequentially. Language is inferred from file extensions. Requires the `ast-grep` binary on `PATH`.

**Approval:** Write tier. File mutations are exclusive. Not available during Plan → Execute planning.

---

## `web_search`

**Purpose:** Web search through bundled **search-cli**, merging configured providers into rank-fused JSON.

**When to use:** External facts not in the repo. Not the same as [`search`](#search) (local grep).

**Availability:** Registered only when **Settings → Search** has at least one configured key (or equivalent env vars). See [`../guides/using-the-app.md#settings-beyond-providers`](../guides/using-the-app.md#settings-beyond-providers).

**Key arguments:** `query` (required), optional `mode` (`general`, `news`, `academic`, `scholar`, `deep`, `people`, `social`, `patents`, `images`, `places`, `extract`, `similar`), optional `count` (default 10).

**Approval:** Read tier.

---

## `openflow_update_todo_list`

**Purpose:** Show the current agent node's phase checklist in chat.

**When to use:** Work with multiple distinct phases. Skip simple tasks. Replace the whole checklist whenever a phase starts or completes; stale updates collapse to the latest valid state.

**Key arguments:** `todos` — 1–12 ordered items with `content` and `status` (`pending`, `in_progress`, or `completed`). At most one item may be `in_progress`.

**Approval:** Read tier. Available in Read only and Plan → Execute planning modes.

---

## `write`

**Purpose:** Create or overwrite a file under the execution folder.

**When to use:** New files, or full replacement. Prefer [`edit`](#edit) for incremental changes on existing files.

**Key arguments:** `path` and `content` (both required). Never call with `path` only.

**Notes:** For large docs, write a small stub first, then grow with `edit` in ~40-line chunks.

**Approval:** Write tier.

---

## `edit`

**Purpose:** Change existing files.

**Modes:**

1. **Replace mode** — `path` + `edits[]` with `old_text`, `new_text`, optional `all` (replace every match).
2. **Hashline mode** — `input` string with `¶path#TAG` sections copied from `read` output.

**Approval:** Write tier. File mutations are serialized with other exclusive write tools on the run.

---

## `apply_patch`

**Purpose:** Apply a Codex-style patch envelope (`*** Begin Patch` … `*** End Patch`).

**When to use:** Multi-file patches. Usually prefer [`edit`](#edit) for small changes.

**Key arguments:** `input` (required) — full envelope text.

**Approval:** Write tier.

---

## `bash`

**Purpose:** Run a shell command under the execution folder.

**When to use:** Git, builds, or commands that do not map to `read` / `search` / `edit`. Use `cwd` for working directory instead of `cd dir && …`.

**Key arguments:** `command` (required), optional `timeout` seconds (default 300, max 3600), optional `env`, optional `cwd`.

**Limits:** Merged stdout/stderr over 50KB spills to an artifact; read via `artifact:{id}`.

**Approval:** Write tier. **Not available during Plan → Execute planning.**

**Concurrency:** At most one `bash` per node at a time.

---

## `openflow_write_plan_artifact`

**Purpose:** Seal the run-local plan draft at `run://PLAN.md` into an immutable plan artifact after human approval.

**When to use:** Plan → Execute workflows only, on the configured evidence-source node. Draft with `write`, refine with replace-mode `edit`, then call this tool with **no arguments**.

**Approval:** Always prompts for explicit human approval. Denial leaves the draft mutable.

**Planning:** Only on nodes allowed to manage the plan during the planning phase.

---

## `openflow_declare_subagents`

**Purpose:** Register ad hoc subagents (name + purpose) for the current node during this run. Shown on the canvas.

**When to use:** Dynamic delegation without pre-saving agents. Omitted when the node **Approval mode** is **Read only**.

**Key arguments:** `subagents` — array of `{ "name", "purpose" }`.

**Approval:** Write tier (no separate prompt when mode allows the tool).

---

## `openflow_call_subagent`

**Purpose:** Run a subagent by ID with a task string. IDs come from [`openflow_declare_subagents`](#openflow_declare_subagents) and/or **callable agents** attached in the inspector (saved agent snapshots). The live tool schema lists invocable IDs.

**When to use:** Delegate a sub-task and return output to the parent agent. Not offered in **Read only** mode or to nested subagent contexts (no recursive `call_subagent`).

**Key arguments:** `subagent_id`, `input`.

**Approval:** Write tier.

See [`../architecture/callable-agents.md`](../architecture/callable-agents.md) for saved-agent snapshots.

---

## `openflow_submit_node_output`

**Purpose:** Finish the agent node with structured output. Plain assistant text does **not** complete the node.

**When to use:** Task done, output matches the node JSON schema, no further human input needed.

**Key arguments:**

```json
{
  "output": { },
  "assistant_message": "optional short message for chat"
}
```

Schema fields belong under `output`, not at the top level. Invalid submits may trigger an automatic repair pass (overseer model); see [`../architecture/output-repair.md`](../architecture/output-repair.md).

**Rules:** Call **alone** in a model turn — never in the same batch as executable tools.

**Approval:** Harness (no file approval).

---

## `openflow_request_user_input`

**Purpose:** Pause a workflow node with one required free-text question.

**When to use:** Clarification is required, choices do not cover the likely answers, and the answer cannot be resolved with tools or upstream input. Normal direct-chat replies do not call this tool; plain assistant text ends the turn.

**Availability:** Only when the node has **request user input** and free-text input enabled. Direct chat disables this tool because its plain-message lifecycle already accepts the next user message.

**Key arguments:**

- `{ "assistant_message": "<one direct human-facing question>" }`

**Rules:** Call **alone** in a model turn (like submit). Plain provider text does not pause ordinary workflow nodes.

**Approval:** Harness.

---

## `openflow_ask_user_question`

**Purpose:** Pause with 1-3 structured multiple-choice questions.

**When to use:** A human answer is required and 2-3 clear choices make the decision easier. Do not use it merely to keep a conversation open.

**Availability:** Only when the node has **request user input** and structured input enabled. Direct chat offers this tool as an optional alternative to a normal plain reply.

**Key arguments:**

- `{ "questions": [{ "id": "<snake_case id>", "header": "<12 chars max>", "question": "<question>", "options": [{ "label": "<choice>", "description": "<tradeoff>" }] }] }`

Each call accepts 1-3 questions with 2-3 options each. The normal composer remains available for a free-text answer.

**Rules:** Call **alone** in a model turn. Do not mix it with submit, free-text input, or executable tools.

**Approval:** Harness.

---

## MCP tools

**Purpose:** External tools exposed by MCP servers you enable in settings.

**Names:** `mcp/<server-id>/<tool-name>` (exact names appear in the model catalog at run start).

**When to use:** Integrations (issue trackers, browsers, custom servers) beyond built-ins.

**Setup:** [`../guides/using-the-app.md#settings-beyond-providers`](../guides/using-the-app.md#settings-beyond-providers).

An unavailable server is skipped during run setup. Other MCP servers and the chat continue; OpenFlow adds a system chat message naming the skipped server.

**Approval:** Treated as write tier. **Not available during Plan → Execute planning.**

---

## Workflow authoring tools (Build with AI)

Used only in the **Build with AI** session, not in normal workflow runs:

| Tool | Purpose |
| --- | --- |
| `openflow_set_workflow_meta` | Name and workflow-level metadata on the draft |
| `openflow_add_node` | Add a node (short system and task prompts) |
| `openflow_update_node` | Change node fields |
| `openflow_add_edge` | Connect nodes |
| `openflow_remove_node` | Remove a node |
| `openflow_remove_edge` | Remove an edge |
| `openflow_submit_node_output` | Finish authoring with `assistantMessage` only (no workflow `output` payload) |

Do not mix `openflow_submit_node_output` with other authoring tools in the same model batch. See [`../guides/using-the-app.md#build-with-ai`](../guides/using-the-app.md#build-with-ai).

## Related

- [`../guides/using-the-app.md`](../guides/using-the-app.md) — approval modes and chat/tool UI
- [`../concepts/workflows-and-runs.md`](../concepts/workflows-and-runs.md) — runs, Plan → Execute
- [`../guides/for-new-users.md`](../guides/for-new-users.md) — first-hour path
