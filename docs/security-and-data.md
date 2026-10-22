# Security and data

OpenFlow runs locally, but model providers, web search, MCP servers, and tools can send data outside the app. Before running sensitive work, review the workflow and choose what it may access: [project files](guides/projects-and-persistence.md), providers, and [external tools](guides/tools-and-integrations.md).

## Credentials

The [persistence reference](reference/README.md#runtime-and-persistence-paths) lists credential and data locations.

Saved provider API keys go in local `settings.json`; ChatGPT (Codex) OAuth sessions are also stored locally. MCP inputs and OAuth tokens go in `mcp-secrets.json` (plaintext, mode `0600` on Unix), and settings and MCP exports reference them opaquely. Do not commit these files or put credentials in prompts, workflow JSON, diagnostics, or screenshots.

API keys resolve as transient run input → stored provider profile key → provider environment variable. ChatGPT (Codex) requires its stored OAuth session and ignores `OPENAI_API_KEY`. [Provider key resolution](reference/providers.md#provider-key-resolution) covers precedence and readiness.

## Files and projects

Without a project, app workflows and chats run in isolated app-managed workspaces, which are not exposed as an MCP project root. With a project, file tools and the run use the selected project's execution folder and `.flow` directory.

Attachments are copied into run-owned storage and validated by size, type, and hash before provider invocation. Checkpoints keep attachment metadata, not source paths or inline bytes. Image previews are bounded local derivatives; document previews are not executed as HTML or scripts.

## Tool and MCP trust

Check a node's approval mode before running it: **Read only** omits write and shell tools; **Read auto-approve, write prompt** still asks before writes; **Always ask** prompts for each call; **Auto-approve all** removes those prompts for allowed tools.

MCP servers are external programs or services. Remote connections require HTTPS, except explicit loopback development; unsafe private/link-local targets and redirects are blocked. Review each server's source, command, environment, discovered tools, policy, and project-root exposure. Security-relevant configuration changes invalidate trust, so run **Approve & Test** again. [MCP troubleshooting](troubleshooting/README.md#mcp-servers) covers failed tests and authentication.

### MCP and external services

MCP resources and prompts are selected explicitly and snapshotted at run start; treat them as untrusted input. Sampling and elicitation are off by default, need server capability enablement plus one-time approval, and are bounded by request and token budgets. Once you enable those capabilities, a server can receive tool arguments and selected project-root access.

Provider, web search, and MCP calls can send prompt text, selected files, tool results, and workflow context to that service. Check its retention, billing, and access policy before enabling it.

## Cleanup and deletion

Removing a project from the sidebar leaves its folder and `.flow` files on disk. Deleting a chat removes its OpenFlow history and run-owned copies; filesystem cleanup may be retried later. While OpenFlow is running, do not manually delete active run directories or edit JSON stores.
