# Using the app

Task-oriented map of the OpenFlow desktop UI after install and provider setup. For install and keys, see [`../getting-started/README.md`](../getting-started/README.md). For a first workflow build, see [`first-workflow.md`](first-workflow.md).

## Sidebar

| Control | Purpose |
| --- | --- |
| **New chat** | Start a saved, direct AI conversation without creating or opening a workflow. Existing chats appear under **Chats**. |
| **Agents** | Create and edit saved agent definitions (prompt, model, tools, Markdown or JSON output). |
| **Schedule** | Enable timed or interval schedules on workflows that already exist in the catalog. |
| **Workflows** | App-level workflows stored under the OpenFlow data directory. **New workflow** creates one; **Build with AI** opens the authoring chat. Use a workflow row's options menu to rename or delete it. |
| **Projects** | Bind a repository folder, create or assign workflows under `.flow/workflows/`, and open project workflows in the editor. |
| **Help** | Replay the guided UI tour. |
| **Settings** | Appearance, providers ([`provider-setup.md`](provider-setup.md)), web search keys, MCP servers, diagnostics, and about. |

On first launch, OpenFlow explains chats versus workflows, then highlights the real canvas, Inspector, Workflow Settings, run control, and composer. Use **Next** and **Back** to move through the tour, **Skip tour** or Escape to dismiss it, then **Help** to replay it later.

The **Chats**, **Workflows**, and **Projects** headings keep their `+` actions visible. Use them to create a chat or workflow, or select a repository folder to add as a project.

Project workflow files override app workflows when both share the same workflow ID. Paths are listed in [`../reference/README.md#runtime-and-persistence-paths`](../reference/README.md#runtime-and-persistence-paths).

## Direct chats

Select **New chat**, then send a message in the full-page, single-pane composer. OpenFlow saves the Chat separately from the workflow catalog and names it from the first message. Select an existing entry under the separate **Chats** sidebar heading to restore its flat transcript; sending another message resumes its durable run. Open a chat row's options menu to remove it from chat history. Stop its run first when that chat is active.

Use the paperclip, paste an image, or drop files onto the composer to attach up to four files per
message. A single file can be at most 10 MiB; the message total is 25 MiB. Supported formats:
JPEG, PNG, GIF, WebP, PDF, plain text, Markdown, CSV, JSON, HTML, CSS, JavaScript, and Python.
Attachment-only messages are valid.

OpenFlow validates each file, copies it into the durable run, then sends the managed copy to the
provider. Moving or deleting the original does not break chat replay. Image previews are bounded
local derivatives; documents render as metadata cards and never as active HTML or scripts. Deleting
a saved chat also deletes its run-owned copies. If cleanup cannot finish immediately, the chat is
deleted and the app reports that cleanup remains pending.

Provider serializers support these attachment shapes, but the selected model can still reject an
image or document. Keep the pending card when import fails. For a model capability error, select a
media-capable model and retry.

Use the controls below the composer to choose:

- **Project** — scopes file references, execution cwd, and durable run storage to that project. Choose before sending the first message. Select **Add Project…** in this menu to add a repository folder and select it for the chat.
- **Model** — selects the model for the next assistant turn.
- **Speed** — selects Standard or Fast for OpenAI and ChatGPT (Codex). Fast changes service priority, not reasoning effort.
- **Approval mode** — controls which tools require confirmation.
- **Reasoning effort** — selects a provider-supported effort level and, when required, its token budget.

Without a project, OpenFlow runs the chat in an isolated app-managed workspace. With a project,
OpenFlow uses that project's configured execution folder and stores run metadata under `.flow/runs/`.
Adding a project or starting its run checks `.flow` write access first.

Direct chat ends each assistant turn after a normal reply; it does not require a closing question.
When your answer is required and clear choices help, the assistant can show a multiple-choice card.
You can always answer that card through the normal composer instead.

The saved Chat contains only chat metadata and its durable run ID. At run start, the backend privately adapts that Chat to the workflow execution engine. This execution detail is not returned in the Chat DTO or written to `chats.json`: direct chats do not expose nodes or appear in **Workflows**, the canvas, project assignments, or `workflows.json`.

## Editor layout

- **Canvas** — nodes, edges, validation errors, and per-node run status.
- **Inspector** — selected node configuration (instruction, optional provider override, model, handoff, tools, callable agents).
- **Workflow settings** — shared provider, speed, reasoning effort, shared context, optional Plan → Execute gate, and schedule metadata.
- **Bottom dock** — **Chat**, **Terminal**, **Run trace**, and **History** tabs.

## Node handoffs

Use the Inspector **Handoff** section to choose what a completed node gives its downstream nodes:

- **Markdown** — default for new nodes. Edit the heading template to match the work. The node must preserve and fill every heading. OpenFlow validates the result, stores `HANDOFF.md`, and passes its `run://` URI plus hash and media type downstream.
- **JSON** — define a JSON output schema for typed machine data. OpenFlow validates the object, stores `HANDOFF.json`, and passes both the structured output and artifact reference downstream.

Saved workflows created before handoff formats existed load as JSON, preserving their existing output schemas. During execution, downstream nodes receive each direct upstream node's compact output and optional `handoff` manifest. The runtime instructs them to read the immutable `run://` artifact before using its contents.

The **Agents** screen exposes the same format editor. OpenFlow saves the choice with the agent and copies it to workflow nodes created from that agent. Existing saved agents without a format continue to load as JSON.

## Run controls

Use the top bar while a workflow is open in the editor:

- **Run** — start a new run when the provider is ready and no continuable run exists. Optional starter text goes through the chat composer when the workflow expects entrypoint input.
- **Continue** — resume a paused run that still has pending input or approvals.
- **Stop** — cancel the selected run. Other active chats/workflows continue.

Nodes inherit the workflow's shared provider unless their Inspector selects an override. A workflow without a shared provider inherits the active Settings provider. Run start validates every provider referenced by the workflow.

Provider readiness failures are covered in [`../troubleshooting/README.md#provider-not-ready`](../troubleshooting/README.md#provider-not-ready).

## Chat composer

During a run, each active node has a chat thread in the dock.

- Type `/` anywhere in the composer to attach a **skill** from discovered `SKILL.md` files (Cursor, Claude, and Agents skill directories on the machine). The skill bubble shows the recognized invocation; OpenFlow removes the command from the user message, resolves the exact file, and loads it into the run context. Skills are read-only catalog entries, not stored inside OpenFlow.
- Type `@` to reference project files when the workflow is bound to a project.

Tool calls that require approval appear in the thread; approve or deny before execution continues.

After a workflow node changes files, its thread shows a collapsed file-change summary. Expand it,
then select **View diff** on an edit to load the exact diff. OpenFlow keeps repeated edits to the
same path in execution order and makes exact diffs available in live runs and replay. Older runs
show their stored summaries. A warning appears when the node used Bash because shell, external
tool, and MCP writes cannot always be attributed to that node.

## Tools and approval

Agent nodes can invoke built-in tools (read and edit files, run shell commands, search, and others) according to engine tool policy. In the inspector **Tools** section, **Approval mode** controls how often you are prompted:

| Approval mode | Behavior |
| --- | --- |
| **Read only** | Only read-tier tools (for example read, search, find) are offered to the model; write and shell tools are omitted from the catalog. |
| **Read auto-approve, write prompt** | Read-tier tools run without a prompt; write-tier tools (edit, bash, MCP, …) require approval in chat. |
| **Always ask** | Prompt before each tool call. |
| **Auto-approve all** | No approval prompts for tools this node is allowed to use. |

Denied tools return an error result to the model; the run can continue unless the agent cannot recover. Callable **Subagents** in the same inspector delegate work to saved agents without redrawing the graph.

Per-tool names, arguments, and limits: [`../reference/tools.md`](../reference/tools.md).

## Run trace, history, and replay

- **Run trace** — structured timeline of node and tool events for the current or replayed run.
- **History** — durable runs for the active workflow. **Open replay** loads a read-only view; **Resume run** is available when status is `paused`, `stopped`, or `failed`.

See [`../concepts/workflows-and-runs.md`](../concepts/workflows-and-runs.md) for run vocabulary.

## Post-run review

When a run completes successfully, the chat may show **Run review** suggestions: evidence-backed improvements to prompts, tools, workflow structure, models, or coordination. The review uses a separate model pass and does not change whether the run succeeded. If review fails, the UI shows an error message instead of suggestions.

## Build with AI

**Build with AI** starts a workflow authoring session. Ask questions or discuss the goal normally; the assistant proposes graph changes only after you explicitly ask to create or edit the workflow. Review the proposed graph, then select **Create Workflow** or **Apply Changes** to save it. Provider readiness is required, same as running a workflow.

## Schedule

Open **Schedule** in the sidebar to attach a schedule to an existing workflow: choose presets (for example daily time or interval), save, or remove. Scheduled runs use the same runtime as manual **Run**; configure entrypoint behavior in the workflow itself.

## Saved agents and callable agents

Saved agents on the **Agents** screen are library entries. To reuse one inside a workflow, add it as a **callable agent** on a node that should delegate work. Snapshots are frozen at run start; editing the library agent later does not change an in-flight run. See [`../architecture/callable-agents.md`](../architecture/callable-agents.md).

To invoke installed skills from a workflow node or saved agent, type one or more `/skill-id` tokens anywhere in its **Task prompt**:

```text
Implement the approved ticket with /tdd and /code-review
```

The task-prompt editor lists matching skills after you type `/`, using the same discovered-skill catalog as the bottom composer. Recognized tokens show the same skill name and description bubble as chat. At run start, OpenFlow resolves each installed token to its exact `SKILL.md`, loads the file contents into system context before other work, and freezes the resolved paths for that run. An unknown leading command blocks the run with the node or callable-agent name; unknown inline tokens stay literal.

## Settings beyond providers

| Section | Use when |
| --- | --- |
| **Search** | Workflows call web search through bundled search-cli; store per-provider API keys here or export keys in the shell environment. |
| **MCP Servers** | Discover supported configs, import `mcpServers` JSON, install registry packages, or add stdio/remote servers manually. Review trust before enablement. Test, edit, export, delete, or disable connections. **Disable all** turns off configured servers plus external discovery. |
| **Diagnostics** | Local debug output and related developer options. |

OpenFlow supports local stdio, Streamable HTTP, and legacy SSE MCP transports. Remote auth supports static secret-backed headers plus OAuth discovery, PKCE, callback validation, token refresh, and disconnect. MCP inputs and OAuth tokens stay in `{data_local}/openflow/mcp-secrets.json`; the file is plaintext with mode `0600` on Unix. Settings and exports contain opaque refs, never credential values. Remote URLs pass HTTPS, redirect, DNS/IP, and localhost policy checks before connection.

Server-to-client capabilities default to off per server. Enable **Expose selected project root**, **Allow approved sampling reqs**, or **Allow approved form/URL elicitation**, then run **Approve & Test** again. OpenFlow exposes only the selected project's canonical folder as an MCP root; app-managed workspaces stay hidden. Sampling and elicitation stay bound to the originating node and MCP tool call.

Every sampling or elicitation req appears in chat for one-time approval. Sampling uses the originating node's effective provider without tools, human-input tools, or recursive MCP access. Per-req and per-run request/token budgets apply, plus hard app ceilings. Form replies must match the server's primitive JSON schema. URL elicitation accepts credential-free HTTPS URLs, opens them in the system browser, then reports the user's decision. Stopping the run cancels pending callbacks.

Use **Approve & Test** after import or any security-relevant change. Each row shows its current lifecycle state, last stage/error, check time, and attempt count. Use **Retry**, **Restart & Test**, **Disable**, **Copy diagnostics**, or **Open source** as available. OpenFlow reports a missing executable directly and stops waiting after 15 seconds when a server does not start or list tools. During run setup and shutdown, it handles up to four MCP servers concurrently, so one stalled server does not serialize the full server list. MCP tool calls stop waiting after 120 seconds and send the protocol cancellation notification. OpenFlow does not automatically retry a timed-out call because the server may already have completed it.

Per-server policy defaults every tool to **Write** and serializes calls as **Exclusive**. After a successful test, you can allowlist individual discovered tools, classify the whole server as user-reviewed **Read**, or opt into **Shared** concurrency. These are user decisions: OpenFlow never trusts a server's own read-only annotation. Every policy change revokes trust and requires another test.

MCP tools become available to nodes that advertise MCP access during a run. If an enabled server cannot connect or list tools, OpenFlow skips that server, shows a system message in chat, and continues the run without its tools.

To add MCP resources or prompts to a node, open its Inspector, expand **MCP context**, choose an enabled server, then select each resource or prompt explicitly. OpenFlow does not inject catalog content merely because you loaded it. Preview shows source provenance, included size, and truncation. Each selection has a byte cap; the combined node cap is 1 MiB.

At run start, OpenFlow reads selected resources and renders selected prompts once, stores that bounded snapshot with the durable run, then labels it as untrusted data in provider context. Resume and replay use the same immutable snapshot. Resource subscriptions remain attached to the run client until shutdown but do not mutate the frozen provider context.
