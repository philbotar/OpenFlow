# Using the app

Task-oriented map of the OpenFlow desktop UI after install and provider setup. For install and keys, see [`../getting-started/README.md`](../getting-started/README.md). For a first workflow build, see [`first-workflow.md`](first-workflow.md).

## Sidebar

| Control | Purpose |
| --- | --- |
| **Agents** | Create and edit saved agent definitions (prompt, model, tools, output schema). |
| **Schedule** | Enable timed or interval schedules on workflows that already exist in the catalog. |
| **Workflows** | App-level workflows stored under the OpenFlow data directory. **New workflow** creates one; **Build with AI** opens the authoring chat. |
| **Projects** | Bind a repository folder, create or assign workflows under `.flow/workflows/`, and open project workflows in the editor. |
| **Settings** | Appearance, providers ([`provider-setup.md`](provider-setup.md)), web search keys, MCP servers, diagnostics, and about. |

Project workflow files override app workflows when both share the same workflow ID. Paths are listed in [`../reference/README.md#runtime-and-persistence-paths`](../reference/README.md#runtime-and-persistence-paths).

## Editor layout

- **Canvas** — nodes, edges, validation errors, and per-node run status.
- **Inspector** — selected node configuration (instruction, model, tools, callable agents).
- **Workflow settings** — shared context, execution cwd, provider overrides, optional Plan → Execute gate, and schedule metadata.
- **Bottom dock** — **Chat**, **Terminal**, **Run trace**, and **History** tabs.

## Run controls

Use the top bar while a workflow is open in the editor:

- **Run** — start a new run when the provider is ready and no continuable run exists. Optional starter text goes through the chat composer when the workflow expects entrypoint input.
- **Continue** — resume a paused run that still has pending input or approvals.
- **Stop** — cancel the active run.

Provider readiness failures are covered in [`../troubleshooting/README.md#provider-not-ready`](../troubleshooting/README.md#provider-not-ready).

## Chat composer

During a run, each active node has a chat thread in the dock.

- Type `/` to attach a **skill** from discovered `SKILL.md` files (Cursor, Claude, and Agents skill directories on the machine). Skills are read-only catalog entries, not stored inside OpenFlow.
- Type `@` to reference project files when the workflow is bound to a project.

Tool calls that require approval appear in the thread; approve or deny before execution continues.

## Tools and approval

Agent nodes can invoke built-in tools (read and edit files, run shell commands, search, and others) according to engine tool policy. In the inspector **Tools** section, **Approval mode** controls how often you are prompted:

| Approval mode | Behavior |
| --- | --- |
| **Read only** | Only read-tier tools (for example read, search, find) are offered to the model; write and shell tools are omitted from the catalog. |
| **Read auto-approve, write prompt** | Read-tier tools run without a prompt; write-tier tools (edit, bash, MCP, …) require approval in chat. |
| **Always ask** | Prompt before each tool call. |
| **Auto-approve all** | No approval prompts for tools this node is allowed to use. |

Denied tools return an error result to the model; the run can continue unless the agent cannot recover. Callable **Subagents** in the same inspector delegate work to saved agents without redrawing the graph.

## Run trace, history, and replay

- **Run trace** — structured timeline of node and tool events for the current or replayed run.
- **History** — durable runs for the active workflow. **Open replay** loads a read-only view; **Resume run** is available when status is `paused`, `stopped`, or `failed`.

See [`../concepts/workflows-and-runs.md`](../concepts/workflows-and-runs.md) for run vocabulary.

## Post-run review

When a run completes successfully, the chat may show **Run review** suggestions: evidence-backed improvements to prompts, tools, workflow structure, models, or coordination. The review uses a separate model pass and does not change whether the run succeeded. If review fails, the UI shows an error message instead of suggestions.

## Build with AI

**Build with AI** starts a workflow authoring session: describe the goal in chat, iterate on the draft graph in the preview, then apply the draft to a new or project-scoped workflow. Provider readiness is required, same as running a workflow.

## Schedule

Open **Schedule** in the sidebar to attach a schedule to an existing workflow: choose presets (for example daily time or interval), save, or remove. Scheduled runs use the same runtime as manual **Run**; configure entrypoint behavior in the workflow itself.

## Saved agents and callable agents

Saved agents on the **Agents** screen are library entries. To reuse one inside a workflow, add it as a **callable agent** on a node that should delegate work. Snapshots are frozen at run start; editing the library agent later does not change an in-flight run. See [`../architecture/callable-agents.md`](../architecture/callable-agents.md).

## Settings beyond providers

| Section | Use when |
| --- | --- |
| **Search** | Workflows call web search through bundled search-cli; store per-provider API keys here or export keys in the shell environment. |
| **MCP Servers** | Add MCP server commands, probe connectivity, enable or disable discovered servers from external config, and control whether external discovery runs. |
| **Diagnostics** | Local debug output and related developer options. |

MCP tools become available to nodes that advertise MCP access during a run.
