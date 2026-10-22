# Data locations

<a id="reference"></a>

App-managed data lives under `{data_local}/openflow/`; project data lives in the selected project's `.flow/` directory.

## Runtime and persistence paths

| Data | Path |
| --- | --- |
| App workflows | `{data_local}/openflow/workflows.json` |
| Chats | `{data_local}/openflow/chats.json` |
| Settings | `{data_local}/openflow/settings.json` |
| MCP inputs and OAuth credentials | `{data_local}/openflow/mcp-secrets.json` (plaintext, mode `0600` on Unix) |
| Projects | `{data_local}/openflow/projects.json` |
| Saved agents | `{data_local}/openflow/agents.json` |
| Project workflows | `{project}/.flow/workflows/{workflowId}.workflow.json` |
| App-managed execution folders | `{data_local}/openflow/workspaces/{workflows\|chats}/{id}` |
| App-run attachments | `{data_local}/openflow/runs/{runId}/attachments/{attachmentId}.{ext}` |
| Project-run attachments | `{project}/.flow/runs/{runId}/attachments/{attachmentId}.{ext}` |
| App-run node handoffs | `{data_local}/openflow/runs/{runId}/handoffs/{nodeId}/HANDOFF.md` or `HANDOFF.json` |
| Project-run node handoffs | `{project}/.flow/runs/{runId}/handoffs/{nodeId}/HANDOFF.md` or `HANDOFF.json` |
| Provider API keys | Plaintext in `settings.json` as `ProviderProfile.api_key` |
| Web search API keys | Plaintext in `settings.json` under `search.keys` (per search-cli provider id) |
| ChatGPT Codex OAuth | Plaintext in `settings.json` as `ProviderProfile.codex_oauth`; redacted from normal settings IPC |
| MCP inputs and OAuth | Plaintext in `mcp-secrets.json`; `settings.json` and exports contain opaque refs only |

`AppBackend::load_all_workflows` merges app-store and project-discovered workflows; project files win on ID collision.

Run start takes a project ID, and the backend resolves the cwd and run root. Project runs use the exact selected project; app workflow and chat runs use the isolated managed folders above. There is no global `~/.flow`; `.flow` stays project-local.

Browser paste/drop staging uses opaque tokens under app data. A successful send or explicit removal cleans up staged copies; stale copies are removed after 24 hours as a fallback.

<a id="provider-key-resolution"></a>

[Provider profiles](providers.md#provider-key-resolution) defines credential precedence. [Security and data](../security-and-data.md) covers which values are sensitive and which services receive them.

<a id="dev-commands"></a>
<a id="architecture-check-config"></a>

For source checkouts, see [Development commands](../contributing/commands.md) and the [architecture check configuration](../contributing/commands.md#architecture-check-config).

## Chat records

OpenFlow stores chat metadata and a durable run ID, and runs each conversation on the workflow runtime. Direct chats have no visible nodes, do not appear on the workflow canvas, and are not in `workflows.json`.

Chat checkpoints store attachment refs only: generated ID, sanitized filename, media type, byte size, SHA-256, and kind. They never store source paths or inline bytes. Orchestration verifies and hydrates the managed bytes immediately before provider invocation.
