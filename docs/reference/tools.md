# Tools reference

Agents call three kinds of tools:

- **Built-in** tools for files, shell commands, and search, registered in `crates/orchestration/src/tool/registry.rs`.
- **Harness** tools (`openflow_submit_node_output`, `openflow_request_user_input`, `openflow_ask_user_question`) complete the node or pause for a human.
- **MCP** tools, added at run start from **Settings → MCP Servers** under a provider-safe `mcp_…` namespace.

Paths are relative to the run **execution folder**: the selected project's configured folder, or an isolated app-managed workspace when no project is selected. The node's [**Approval mode**](../guides/tools-and-integrations.md#approval-modes) in the Inspector controls confirmation prompts. Many tools accept an optional `_i` (intent): short text shown in the chat UI and ignored by the implementation.

| Tier | Typical approval (Write mode) |
| --- | --- |
| Read | Auto-allow |
| Write | Prompt in chat |

**Read only** approval mode does not offer write-tier tools or `bash`. The **Plan → Execute** planning phase blocks `bash`, MCP, subagent calls, and most writes; see [`openflow_write_plan_artifact`](#openflow_write_plan_artifact).

## Quick index

| Tool | Chat label | Tier |
| --- | --- | --- |
| [`read`](#read) | Read File | Read |
| [`ls`](#ls) | List Directory | Read |
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
| [Authoring tools](#workflow-authoring-tools-create-with-ai) | varies | Authoring session only |

## `read`

Read a file or directory, fetch HTTP(S), or read spilled tool output. Use it before editing, to inspect upstream artifacts, or to retrieve truncated output through `artifact:{id}`.

**Key arguments:** `path`: repository-relative path, URL, or `artifact:{id}`. Append `:start-end` for a line range (e.g. `src/lib.rs:10-20`) or `:raw` for unnumbered full content.

**Limits:** Default output is numbered lines, capped at 3000 lines.

**Approval:** Read tier.

## `ls`

List a local directory under the execution folder without a shell command.

**Key arguments:** `path`: repository-relative directory; `.` is the execution folder.

**Approval:** Read tier.

## `search`

Search file contents for symbols or strings, like ripgrep. Uses Rust regex syntax, so no backreferences or lookaround. For syntax-tree queries, use [`ast_grep`](#ast_grep).

**Key arguments:** `pattern` (required), `paths` (required, string or array), optional `i` (case insensitive), optional `gitignore` (default true).

**Limits:** 500 matches. If capped, narrow `pattern` or `paths`.

**Approval:** Read tier.

## `find`

Find file and directory paths by glob, such as `**/*.rs` or `src/**/*.ts`. Inspect results with `read` or `search`.

**Key arguments:** `paths` (required): glob or array of globs.

**Limits:** 200 paths.

**Approval:** Read tier.

## `ast_grep`

Search code by syntax-tree structure with ast-grep and `$VAR` metavariables.

**Key arguments:** `pat` (required), `paths` (required array).

**Approval:** Read tier.

## `ast_edit`

Rewrite code by syntax-tree structure through the `ast-grep` CLI, for codemods and multi-file changes. For individual local changes, use [`edit`](#edit).

**Key arguments:** `ops` (required, non-empty array of `{ "pat", "out" }`) and `paths` (required, non-empty array of files, directories, or globs under the execution folder). An empty `out` deletes each match.

Operations run in order. The language is inferred from file extensions. Requires the `ast-grep` binary on `PATH`.

**Approval:** Write tier. File mutations are exclusive. Not available during Plan → Execute planning.

## `web_search`

Search the web through bundled **search-cli**, which merges configured providers' results into rank-fused JSON. For repository content, use [`search`](#search).

Requires at least one key in **Settings → Search** or an equivalent environment variable. The [web search guide](../guides/tools-and-integrations.md#web-search) covers credential setup.

**Key arguments:** `query` (required), optional `mode` (`general`, `news`, `academic`, `scholar`, `deep`, `people`, `social`, `patents`, `images`, `places`, `extract`, `similar`), optional `count` (default 10).

**Approval:** Read tier.

## `openflow_update_todo_list`

Show a phase checklist in the node's chat for multi-phase work; skip it for simple tasks. Send the whole checklist again when a phase starts or completes. Older updates collapse to the latest valid state.

**Key arguments:** `todos`: 1 to 12 ordered items with `content` and `status` (`pending`, `in_progress`, or `completed`). At most one item may be `in_progress`.

**Approval:** Read tier. Available in Read only and Plan → Execute planning modes.

## `write`

Create or fully overwrite a file under the execution folder. For incremental changes, use [`edit`](#edit).

**Key arguments:** `path` and `content`, both required. Never call with `path` only. For large documents, write a small stub, then expand it with `edit` in roughly 40-line chunks.

**Approval:** Write tier.

## `edit`

Change an existing file in one of two modes:

1. **Replace mode**: `path` + `edits[]` with `old_text`, `new_text`, optional `all` (replace every match).
2. **Hashline mode**: `input` string with `¶path#TAG` sections copied from `read` output.

**Approval:** Write tier. File mutations are serialized with other exclusive write tools on the run.

## `apply_patch`

Apply a Codex-style patch envelope (`*** Begin Patch` … `*** End Patch`) across one or more files. For small changes, use [`edit`](#edit).

**Key arguments:** `input` (required): full envelope text.

**Approval:** Write tier.

## `bash`

Run a shell command under the execution folder for Git, builds, or work that `read`, `search`, and `edit` don't cover. Set the working directory with `cwd`, not `cd dir && …`.

**Key arguments:** `command` (required), optional `timeout` seconds (default 300, max 3600), optional `env`, optional `cwd`.

**Limits:** Merged stdout/stderr over 50KB spills to an artifact; read it via `artifact:{id}`.

**Approval:** Write tier. In Write mode, plain `ls` and read-only `find` commands count as read-tier; shell operators and mutating `find` predicates stay write-tier. Not available during Plan → Execute planning.

**Concurrency:** At most one `bash` per node at a time.

## `openflow_write_plan_artifact`

Seal `run://PLAN.md` into an immutable plan artifact after human approval. Only the configured evidence-source node in a Plan → Execute workflow gets this tool. Draft with `write`, refine with replace-mode `edit`, then call the sealer with **no arguments**.

**Approval:** Always prompts for explicit human approval. Denial leaves the draft mutable.

## `openflow_declare_subagents`

Register ad hoc subagents by name and purpose for the current node and run. They appear on the canvas and need no saved agent definition. Omitted when **Approval mode** is **Read only**.

**Key arguments:** `subagents`: array of `{ "name", "purpose" }`.

**Approval:** Write tier (no separate prompt when the mode allows the tool).

## `openflow_call_subagent`

Run a subagent by ID with a task string. The subagent does the task and returns output to its parent. IDs come from [`openflow_declare_subagents`](#openflow_declare_subagents) and/or **callable agents** attached in the inspector (saved agent snapshots); the live tool schema lists invocable IDs.

Unavailable in **Read only** mode and inside subagents, which prevents recursive `call_subagent` calls.

**Key arguments:** `subagent_id`, `input`.

**Approval:** Write tier.

[Callable agents](../architecture/callable-agents.md) describes saved-agent snapshots.

## `openflow_submit_node_output`

Finish the agent node when its task is done, its output matches the node JSON schema, and it needs no more human input. Plain assistant text does not complete the node.

**Key arguments:**

```json
{
  "output": { },
  "assistant_message": "optional short message for chat"
}
```

Schema fields go under `output`, not at the top level. An invalid submit may trigger an automatic repair pass (overseer model); see [Output repair](../architecture/output-repair.md).

**Rules:** Call **alone** in a model turn, never batched with executable tools.

**Approval:** Harness (no file approval).

## `openflow_request_user_input`

Pause a workflow node with one required free-text question when tools and upstream input can't resolve the answer and fixed choices won't do. In direct chat, a normal reply ends the turn with plain assistant text.

**Availability:** Only when the node has **request user input** and free-text input enabled. Direct chat disables this tool because its plain-message lifecycle already accepts the next user message.

**Key arguments:**

- `{ "assistant_message": "<one direct human-facing question>" }`

**Rules:** Call **alone** in a model turn, like submit. Plain provider text does not pause ordinary workflow nodes.

**Approval:** Harness.

## `openflow_ask_user_question`

Pause for a required human answer with 1-3 structured multiple-choice questions. Use it when 2-3 clear choices help the decision, not just to keep a conversation open.

**Availability:** Only when the node has **request user input** and structured input enabled. In direct chat, it is an optional alternative to a plain reply.

**Key arguments:**

- `{ "questions": [{ "id": "<snake_case id>", "header": "<12 chars max>", "question": "<question>", "options": [{ "label": "<choice>", "description": "<tradeoff>" }] }] }`

Each call takes 1-3 questions with 2-3 options each. The normal composer stays available for a free-text answer.

**Rules:** Call **alone** in a model turn. Do not mix it with submit, free-text input, or executable tools.

**Approval:** Harness.

## MCP tools

MCP servers enabled in Settings expose external tools, such as issue trackers, browsers, and custom services.

- **Names:** provider-safe `mcp_…` identifiers of ASCII letters, digits, `_`, and `-`. They reversibly encode the server ID and original MCP tool name; OpenFlow dispatches model calls back to the original names.
- **Setup:** in **Settings → MCP Servers**, discover or import a supported config, install a registry package, or configure stdio, Streamable HTTP, or legacy SSE manually. The [MCP guide](../guides/tools-and-integrations.md#mcp-servers) covers setup and trust.
- **Credentials:** remote servers support secret-backed headers and OAuth. Credential values stay out of settings payloads and exported configs.
- **Trust:** bound to security-relevant config; changing that config requires review again.
- **Roots, sampling, and elicitation:** denied by default. Per-server policy may expose only the selected project root. Each sampling or elicitation request needs a one-time chat decision and carries its originating node, tool-call ID, and tool name. Sampling cannot request MCP tools, other tools, tasks, or cross-server context.
- **Budgets:** app ceilings bound configured budgets: 64 sampling requests per run, 65,536 output tokens per request, 262,144 requested sampling tokens per run, and 64 elicitation requests per run.
- **Unavailable servers:** skipped during run setup. Other MCP servers and the chat continue; a system chat message names the skipped server.

**Approval:** Treated as write tier. **Not available during Plan → Execute planning.**

## Workflow authoring tools (Create with AI)

Only in the **Create with AI** session, not in normal workflow runs:

| Tool | Purpose |
| --- | --- |
| `openflow_set_workflow_meta` | Name and workflow-level metadata on the draft |
| `openflow_add_node` | Add a node (short system and task prompts) |
| `openflow_update_node` | Change node fields |
| `openflow_add_edge` | Connect nodes |
| `openflow_remove_node` | Remove a node |
| `openflow_remove_edge` | Remove an edge |
| `openflow_submit_node_output` | Finish authoring with `assistantMessage` only (no workflow `output` payload) |

Do not mix `openflow_submit_node_output` with other authoring tools in the same model batch. To review and save the result, follow [Create with AI](../guides/workflow-authoring.md#create-with-ai-safely).

[Using the app](../guides/using-the-app.md) locates the chat and tool controls. [Workflows and runs](../concepts/workflows-and-runs.md) explains run states and Plan → Execute. The [OpenFlow overview](../README.md) gives a first workflow path.
