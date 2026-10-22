# Security and data

OpenFlow runs locally. Model providers, web search, MCP servers, and tools can send data outside the app. Before running work with sensitive information, review the workflow and choose which [project files](guides/projects-and-persistence.md), providers, and [external tools](guides/tools-and-integrations.md) it may access.

## Credentials

The [persistence reference](reference/README.md#runtime-and-persistence-paths) lists credential and data locations.

Provider API keys are stored in local `settings.json` when saved. ChatGPT (Codex) OAuth sessions are also stored locally. MCP inputs and OAuth tokens are stored in `mcp-secrets.json`, plaintext with mode `0600` on Unix. Settings and MCP exports use opaque references for MCP secrets; do not commit these files or place credentials in prompts, workflow JSON, diagnostics, or screenshots.

API-key resolution is transient run input → stored provider profile key → provider environment variable. ChatGPT (Codex) requires its stored OAuth session and does not use `OPENAI_API_KEY`. [Provider key resolution](reference/providers.md#provider-key-resolution) describes precedence and readiness.

## Files and projects

Without a project, app workflows and chats run in isolated app-managed workspaces. With a project, file tools and the run use the selected project's execution folder and `.flow` directory. OpenFlow does not expose app-managed workspaces as an MCP project root.

Attachments are copied into run-owned storage and validated by size, type, and hash before provider invocation. Checkpoints retain attachment metadata, not source paths or inline bytes. Image previews are bounded local derivatives; document previews are not executed as HTML or scripts.

## Tool and MCP trust

Read the approval mode before running a node. **Read only** omits write and shell tools; **Read auto-approve, write prompt** still asks before writes; **Always ask** prompts for each call; **Auto-approve all** removes those prompts for allowed tools.

MCP servers are external programs or services. Remote connections require HTTPS except explicit loopback development, and unsafe private/link-local targets and redirects are blocked. Review the server source, command, environment, discovered tools, policy, and project-root exposure. Trust is invalidated after security-relevant configuration changes, so use **Approve & Test** again. [MCP troubleshooting](troubleshooting/README.md#mcp-servers) covers failed tests and authentication.

### MCP and external services

MCP resources and prompts are selected explicitly and snapshotted at run start. Treat them as untrusted input. Sampling and elicitation are off by default, require server capability enablement and one-time approval, and are bounded by request and token budgets. A server can receive tool arguments and selected project-root access when you enable those capabilities.

Provider calls, web search, and MCP calls can send prompt text, selected files, tool results, and workflow context to the relevant external service. Check that service's retention, billing, and access policy before enabling it.

## Cleanup and deletion

Removing a project from the sidebar leaves its folder and `.flow` files on disk. Deleting a chat removes its OpenFlow history and run-owned copies, although filesystem cleanup may be retried later. Do not manually delete active run directories or edit JSON stores while OpenFlow is running.
