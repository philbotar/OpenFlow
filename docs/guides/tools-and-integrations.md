# Tools and integrations

Choose each node's **Approval mode** to control which tools it can use and when it asks permission. Nodes can use built-in tools, saved agents, discovered skills, web search, and MCP servers. The [Tools reference](../reference/tools.md) lists their arguments and limits.

## Approval modes

Set **Approval mode** in a node's Inspector:

| Mode | Behavior |
| --- | --- |
| **Read only** | Exposes read-tier tools. |
| **Read auto-approve, write prompt** | Runs read-tier tools without prompting and asks before write-tier tools. |
| **Always ask** | Prompts for each tool call. |
| **Auto-approve all** | Skips prompts for tools allowed to that node. |

Read-only `ls` and read-only `find` commands through Bash can remain auto-approved in the middle mode. Shell operators and mutating `find` predicates are write-tier. MCP tools are write-tier by default unless the user explicitly reviews a server policy.

## Skills

Type `/` in a composer or task prompt to search discovered `SKILL.md` files. In a run, OpenFlow resolves each recognized skill to its exact file and loads it into context. Skills are read-only catalog entries and are not copied into OpenFlow storage. An unknown leading command blocks a run; an unknown inline token remains literal.

## Saved and callable agents

The **Agents** screen stores reusable agent definitions. Add a saved agent to a node as a callable agent when that node should delegate work. The callable-agent definition is snapshotted at run start, so later library edits do not change an in-flight run. [Callable agents](../architecture/callable-agents.md) describes the snapshot lifecycle.

## Web search

Configure search credentials in **Settings → Search**. Workflows can then use the bundled web-search tool. Store keys in Settings or use the provider's environment fallback; do not place secrets in prompts or workflow files.

## MCP servers

Open **Settings → MCP Servers** to add Stdio, Streamable HTTP, or legacy SSE servers, import `mcpServers` JSON, test, trust, disable, or remove a connection. Have the server command, URL, or imported configuration ready. Enter credentials in Settings, not workflow text. Review the server policy before enabling it; OpenFlow does not automatically trust a server's read-only annotation. [Security and data](../security-and-data.md#mcp-and-external-services) explains what external services can receive.

MCP resources and prompts are not injected just because a server is connected. In a node Inspector, open **MCP context** and select each resource or prompt explicitly. OpenFlow snapshots the selected content at run start; resume and replay use that immutable snapshot. Selected context is untrusted provider input and is bounded by the limits in [Configuration](../reference/configuration.md#mcp).

Enable server-to-client capabilities deliberately: **Expose selected project root**, **Allow approved sampling reqs**, and **Allow approved form/URL elicitation** are off by default. Sampling and elicitation requests appear in the originating conversation for one-time approval. Stopping the run cancels pending callbacks.

Use **Approve & Test** after import or a security-relevant change. Trust is revoked when source, install, connection, or policy changes. If one enabled server fails setup or listing during a run, OpenFlow skips it and continues with other servers.

For startup, authentication, or listing failures, use [MCP troubleshooting](../troubleshooting/README.md#mcp-servers).
