# Tools reference

Agents use built-in tools for file operations, shell commands, and search. Built-in repository tools are registered in `crates/orchestration/src/tool/registry.rs`. **Harness** tools (`openflow_submit_node_output`, `openflow_request_user_input`, `openflow_ask_user_question`) control node completion and human pauses. **MCP** tools are added at run start from **Settings → MCP Servers** and use a provider-safe `mcp_…` namespace.

Paths are relative to the run **execution folder** (the selected project's configured folder, or an isolated app-managed workspace when no project is selected). The node's [**Approval mode**](../guides/tools-and-integrations.md#approval-modes) in the Inspector controls confirmation prompts.

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

Load file contents, list a directory, fetch HTTP(S), or read spilled tool output. Use `read` before editing, to inspect upstream artifacts, or to retrieve truncated output through `artifact:{id}`.

**Key arguments:** `path`: repository-relative path, URL, or `artifact:{id}`. Append `:start-end` for a line range (e.g. `src/lib.rs:10-20`) or `:raw` for unnumbered full content.

**Limits:** Default output is numbered lines, capped at 3000 lines.

**Approval:** Read tier.

## `ls`

List a local directory under the execution folder without running a shell command.

**Key arguments:** `path`: repository-relative directory path. Use `.` for the execution folder.

**Approval:** Read tier.

## `search`

Search file contents for symbols or strings with Rust regex syntax, similar to ripgrep. Backreferences and lookaround are unsupported. For syntax-tree queries, use [`ast_grep`](#ast_grep).

**Key arguments:** `pattern` (required), `paths` (required, string or array), optional `i` (case insensitive), optional `gitignore` (default true).

**Limits:** At most 500 matches: narrow `pattern` or `paths` if capped.

**Approval:** Read tier.

## `find`

Find file and directory paths with globs such as `**/*.rs` or `src/**/*.ts`, then inspect them with `read` or `search`.

**Key arguments:** `paths` (required): glob or array of globs.

**Limits:** At most 200 paths.

**Approval:** Read tier.

## `ast_grep`

Search code by syntax-tree structure with ast-grep and `$VAR` metavariables.

**Key arguments:** `pat` (required), `paths` (required array).

**Approval:** Read tier.

## `ast_edit`

Rewrite code by syntax-tree structure through the `ast-grep` CLI. Use it for codemods and changes across multiple files; [`edit`](#edit) suits individual local changes.

**Key arguments:** `ops` (required, non-empty array of `{ "pat", "out" }`) and `paths` (required, non-empty array of files, directories, or globs under the execution folder). An empty `out` deletes each match.

Operations run sequentially. The tool infers the language from file extensions and requires the `ast-grep` binary on `PATH`.

**Approval:** Write tier. File mutations are exclusive. Not available during Plan → Execute planning.

## `web_search`

Search the web through bundled **search-cli**, which merges configured providers' results into rank-fused JSON. For content in the repository, use [`search`](#search).

Available only when **Settings → Search** has at least one configured key or an equivalent environment variable. The [web search guide](../guides/tools-and-integrations.md#web-search) explains credential setup.

**Key arguments:** `query` (required), optional `mode` (`general`, `news`, `academic`, `scholar`, `deep`, `people`, `social`, `patents`, `images`, `places`, `extract`, `similar`), optional `count` (default 10).

**Approval:** Read tier.

## `openflow_update_todo_list`

Show a phase checklist in the current node's chat for work with several distinct phases. Skip it for simple tasks. Replace the whole checklist when a phase starts or completes; older updates collapse to the latest valid state.

**Key arguments:** `todos`: 1 to 12 ordered items with `content` and `status` (`pending`, `in_progress`, or `completed`). At most one item may be `in_progress`.

**Approval:** Read tier. Available in Read only and Plan → Execute planning modes.

## `write`

Create or fully overwrite a file under the execution folder. Use [`edit`](#edit) for incremental changes to an existing file.

**Key arguments:** `path` and `content` (both required). Never call with `path` only.

For large documents, write a small stub first, then expand it with `edit` in roughly 40-line chunks.

**Approval:** Write tier.

## `edit`

Change an existing file using either mode:

1. **Replace mode**: `path` + `edits[]` with `old_text`, `new_text`, optional `all` (replace every match).
2. **Hashline mode**: `input` string with `¶path#TAG` sections copied from `read` output.

**Approval:** Write tier. File mutations are serialized with other exclusive write tools on the run.

## `apply_patch`

Apply a Codex-style patch envelope (`*** Begin Patch` … `*** End Patch`) across one or more files. Use [`edit`](#edit) for small changes.

**Key arguments:** `input` (required): full envelope text.

**Approval:** Write tier.

## `bash`

Run a shell command under the execution folder for Git, builds, or work that `read`, `search`, and `edit` do not cover. Set the working directory with `cwd` rather than `cd dir && …`.

**Key arguments:** `command` (required), optional `timeout` seconds (default 300, max 3600), optional `env`, optional `cwd`.

**Limits:** Merged stdout/stderr over 50KB spills to an artifact; read via `artifact:{id}`.

**Approval:** Write tier. Plain `ls` and read-only `find` commands are treated as read-tier in
Write mode. Shell operators and mutating `find` predicates remain write-tier. Bash is not available
during Plan → Execute planning.

**Concurrency:** At most one `bash` per node at a time.

## `openflow_write_plan_artifact`

Seal `run://PLAN.md` into an immutable plan artifact after human approval. This tool is available only to the configured evidence-source node in a Plan → Execute workflow. Draft with `write`, refine with replace-mode `edit`, then call the sealer with **no arguments**.

**Approval:** Always prompts for explicit human approval. Denial leaves the draft mutable.

## `openflow_declare_subagents`

Register ad hoc subagents by name and purpose for the current node and run. They appear on the canvas and do not need a saved agent definition. The tool is omitted when **Approval mode** is **Read only**.

**Key arguments:** `subagents`: array of `{ "name", "purpose" }`.

**Approval:** Write tier (no separate prompt when mode allows the tool).

## `openflow_call_subagent`

Run a subagent by ID with a task string. IDs come from [`openflow_declare_subagents`](#openflow_declare_subagents) and/or **callable agents** attached in the inspector (saved agent snapshots). The live tool schema lists invocable IDs.

The subagent performs the task and returns output to its parent. The tool is unavailable in **Read only** mode and inside subagents, which prevents recursive `call_subagent` calls.

**Key arguments:** `subagent_id`, `input`.

**Approval:** Write tier.

[Callable agents](../architecture/callable-agents.md) describes saved-agent snapshots.

## `openflow_submit_node_output`

Finish the agent node when its task is done, its output matches the node JSON schema, and it needs no further human input. Plain assistant text does not complete the node.

**Key arguments:**

```json
{
  "output": { },
  "assistant_message": "optional short message for chat"
}
```

Schema fields belong under `output`, not at the top level. Invalid submits may trigger an automatic repair pass (overseer model); see [Output repair](../architecture/output-repair.md).

**Rules:** Call **alone** in a model turn: never in the same batch as executable tools.

**Approval:** Harness (no file approval).

## `openflow_request_user_input`

Pause a workflow node with one required free-text question when tools and upstream input cannot resolve the answer and fixed choices would be insufficient. Normal direct-chat replies use plain assistant text to end the turn.

**Availability:** Only when the node has **request user input** and free-text input enabled. Direct chat disables this tool because its plain-message lifecycle already accepts the next user message.

**Key arguments:**

- `{ "assistant_message": "<one direct human-facing question>" }`

**Rules:** Call **alone** in a model turn (like submit). Plain provider text does not pause ordinary workflow nodes.

**Approval:** Harness.

## `openflow_ask_user_question`

Pause for a required human answer with 1-3 structured multiple-choice questions. Use it when 2-3 clear choices help the decision, not merely to keep a conversation open.

**Availability:** Only when the node has **request user input** and structured input enabled. Direct chat offers this tool as an optional alternative to a normal plain reply.

**Key arguments:**

- `{ "questions": [{ "id": "<snake_case id>", "header": "<12 chars max>", "question": "<question>", "options": [{ "label": "<choice>", "description": "<tradeoff>" }] }] }`

Each call accepts 1-3 questions with 2-3 options each. The normal composer remains available for a free-text answer.

**Rules:** Call **alone** in a model turn. Do not mix it with submit, free-text input, or executable tools.

**Approval:** Harness.

## MCP tools

MCP servers enabled in Settings expose external tools for integrations such as issue trackers, browsers, and custom services.

**Names:** provider-safe `mcp_…` identifiers containing only ASCII letters, digits, `_`, and `-`. OpenFlow encodes the server ID and original MCP tool name reversibly, then dispatches model calls back to the original names.

In **Settings → MCP Servers**, discover or import a supported config, install a registry package, or configure stdio, Streamable HTTP, or legacy SSE manually. The [MCP guide](../guides/tools-and-integrations.md#mcp-servers) explains setup and trust.

Remote servers support secret-backed headers and OAuth. OpenFlow keeps credential values out of settings payloads and exported configs. Server trust is bound to security-relevant config; changing that config requires review again.

MCP roots, sampling, and elicitation default to denied. Per-server policy may expose only the selected project root. Each sampling or elicitation request requires a one-time chat decision and carries its originating node, tool-call ID, and tool name. Sampling cannot request MCP tools, other tools, tasks, or cross-server context. Configured budgets are bounded by app ceilings: 64 sampling requests per run, 65,536 output tokens per request, 262,144 requested sampling tokens per run, and 64 elicitation requests per run.

An unavailable server is skipped during run setup. Other MCP servers and the chat continue; OpenFlow adds a system chat message naming the skipped server.

**Approval:** Treated as write tier. **Not available during Plan → Execute planning.**

## Workflow authoring tools (Create with AI)

Used only in the **Create with AI** session, not in normal workflow runs:

| Tool | Purpose |
| --- | --- |
| `openflow_set_workflow_meta` | Name and workflow-level metadata on the draft |
| `openflow_add_node` | Add a node (short system and task prompts) |
| `openflow_update_node` | Change node fields |
| `openflow_add_edge` | Connect nodes |
| `openflow_remove_node` | Remove a node |
| `openflow_remove_edge` | Remove an edge |
| `openflow_submit_node_output` | Finish authoring with `assistantMessage` only (no workflow `output` payload) |

Do not mix `openflow_submit_node_output` with other authoring tools in the same model batch. Follow [Create with AI](../guides/workflow-authoring.md#create-with-ai-safely) to review and save the result.

[Using the app](../guides/using-the-app.md) locates the chat and tool controls. [Workflows and runs](../concepts/workflows-and-runs.md) explains run states and Plan → Execute; the [OpenFlow overview](../README.md) gives a first workflow path.
