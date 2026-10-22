# Troubleshooting

Find the matching symptom below and keep the exact error when retrying or reporting it. For control locations, use [Using the app](../guides/using-the-app.md); for recovery actions and limits, use [Runs and recovery](../guides/runs.md) and [Configuration](../reference/configuration.md).

## ChatGPT Codex sign-in

| Symptom | Check |
| --- | --- |
| Device code appears instead of a browser callback | Another process owns loopback port `1455`. Complete the device flow or stop that process before retrying. |
| Browser opened but OpenFlow keeps waiting | Confirm the callback reached `http://localhost:1455/auth/callback`; local firewall/proxy tools must allow loopback traffic. Retry if the state/callback expired. |
| “Sign in with ChatGPT” after a previous connection | The refresh session is missing, invalidated, reused, or expired. Sign in again. |
| Workspace/entitlement rejection | The selected ChatGPT account or workspace does not have Codex access. Switch accounts or ask the workspace administrator. |
| HTTP 403 from the Codex backend | The private ChatGPT backend may have changed or rejected third-party `originator: openflow`. This is not fixed by an OpenAI API key; verify current official Codex behavior and the documented compatibility boundary. |
| Usage/rate-limit error | Check ChatGPT plan/workspace limits. ChatGPT subscription limits are separate from OpenAI API billing. |

OAuth credentials are plaintext in the local OpenFlow `settings.json`, matching stored provider API keys. Normal settings IPC and logs redact them. Use Settings → ChatGPT (Codex) → **Disconnect** to delete them.

## Provider not ready

| Symptom | Likely cause | Fix |
| --- | --- | --- |
| Header shows API key missing or **Run** stays disabled | No resolvable key for the active API-key provider | Open **Settings → Providers**, select the active profile, add a key or set the provider env var (for example `OPENAI_API_KEY`), then save. |
| Readiness message on the authoring or run screen | Same as above, or a shared/node provider is incomplete | Confirm every provider referenced by the workflow has valid auth and a model; switch profile or model if needed. |
| ChatGPT (Codex) not ready | No stored OAuth session or expired login | **Settings → Providers** with **ChatGPT (Codex)** active → **Sign in with ChatGPT**, or **Disconnect** and sign in again. See [ChatGPT Codex sign-in](#chatgpt-codex-sign-in). |
| Bedrock not ready | AWS credentials or region misconfigured | Set profile/region in the Bedrock provider panel and verify the credential chain. |

API-key providers resolve transient run input, then the stored `settings.json` key, then the environment variable. ChatGPT (Codex) requires a stored refreshable login and ignores `OPENAI_API_KEY`; Bedrock uses the AWS credential chain and configured region/profile. [Provider key resolution](../reference/providers.md#provider-key-resolution) defines the full rules.

## Chat attachments

| Symptom | Likely cause | Fix |
| --- | --- | --- |
| File stays pending with a size/type error | More than 4 files, one file over 10 MiB, total over 25 MiB, unsupported extension, empty file, or content/type mismatch | Remove the rejected file or export it as JPEG, PNG, GIF, WebP, PDF, TXT, Markdown, CSV, JSON, HTML, CSS, JavaScript, or Python, then retry. |
| Provider rejects an accepted image or document | The configured transport can serialize it, but the selected model/account does not accept that media | Switch to a media-capable model for that provider and retry. Custom models remain provider-dependent. |
| “missing or corrupt” attachment | The managed run copy is absent, changed, or fails its saved size/hash/type checks | Reattach the original file. OpenFlow rejects the request before provider HTTP. |
| Image card has no preview | The bounded local preview failed or its run data is unavailable | The message stays visible as a generic attachment card. Reattach only if the provider also reports an attachment error. |
| Chat deletion reports cleanup pending | Metadata deletion succeeded but run-directory cleanup could not finish | Restart or open run history to retry quarantined cleanup. Do not manually edit `chats.json`. |

## MCP servers

| State / symptom | Likely cause | Fix |
| --- | --- | --- |
| **Not installed** | Registry package metadata exists without a completed exact-version install | Review the package/version/command, install it, then run **Approve & Test**. |
| **Needs config** | Command or required environment/header input is empty | Enter the required value on the server row in **Settings → MCP Servers**, then save and test again. OpenFlow stores credential values in `mcp-secrets.json` and keeps them out of settings payloads and exports. |
| **Auth required** | OAuth token is absent, expired without refresh, revoked, or rejected | Use **Connect OAuth** or **Re-authenticate**. Verify displayed scopes and issuer, then test again. |
| **MCP command `npx` was not found** | OpenFlow was launched outside the user's interactive shell, so the GUI inherited a minimal `PATH` | Confirm `command -v npx` in the embedded **Terminal**, then retry the MCP test. The Terminal, bash tools, and MCP probes, installs, and runs resolve the same login-shell `PATH`; explicit command or MCP `PATH` config still wins. |
| **OAuth browser does not open** | Protected-resource discovery, authorization-server discovery, PKCE validation, or client registration failed before browser launch | Read the OAuth setup error on the server row. Retry **Authenticate OAuth** for any remote server. Fix the server's OAuth metadata; if dynamic registration is unavailable, enter its pre-registered client ID. A changed authorization-server issuer automatically triggers fresh registration. |
| **Atlassian OAuth does not open** | Older Rovo URLs (`/v1/sse` or `/v1/mcp`) use retired or incomplete OAuth discovery metadata | Re-import or retry the server. OpenFlow migrates Atlassian Rovo to `https://mcp.atlassian.com/v1/mcp/authv2`, refreshes stale DCR metadata, and starts OAuth from the current authorization server. |
| **Atlassian consent rejects the callback** | Atlassian organization policy blocks the local callback origin | Ask an Atlassian admin to allow `http://localhost:*/**` and `http://127.0.0.1:*/**` in Rovo MCP domain settings, then retry in a fresh browser session. |
| **Failed · Preflight** | Executable missing, remote URL rejected, secret ref unavailable, or trust stale | Read the exact row error. Fix config/runtime, then **Retry**. Security-relevant changes require fresh approval. |
| **Failed · Connect/List tools/Close** | Process/HTTP initialization, capability negotiation, pagination, or shutdown failed | Use **Copy diagnostics**. Retry only as a user action; OpenFlow does not replay ambiguous tool calls automatically. |
| Enabled server skipped during a run | That server failed setup; other servers continue | Read the system chat message, disable/fix the named server, then restart the run if its tools are required. |
| Sampling/elicitation card appears | An allowed server made a server-to-client request from an active MCP tool | Review the server and originating tool, then approve once or deny. **Stop** cancels pending callbacks. |

Local children run only during probes/runs. OpenFlow keeps no background MCP process. Remote endpoints require HTTPS unless explicit loopback development is enabled; private/link-local targets and unsafe redirects remain blocked.

## Workflow validation and authoring

| Symptom | Likely cause | Fix and verification |
| --- | --- | --- |
| **Run** is disabled or the canvas shows validation errors | The graph is empty, has a cycle, has a missing endpoint, or contains an invalid schema or handoff | Fix the highlighted node or edge, preserve every Markdown handoff heading, and confirm the validation state clears before selecting **Run**. See [Workflow authoring](../guides/workflow-authoring.md). |
| **Create with AI** produced a proposal but no workflow | A proposal is not persisted until **Create Workflow** or **Apply Changes** succeeds | Review the graph and use the explicit action. If the draft was left open, choose **Resume AI workflow draft**. |
| A project workflow cannot be saved or run | `.flow` is missing or not writable, or the project is being removed while active | Confirm the selected folder is writable, stop the active run, and retry. Project files remain under `.flow/`; see [Projects and persistence](../guides/projects-and-persistence.md). |

## Runs, recovery, and schedules

| Symptom | Likely cause | Fix and verification |
| --- | --- | --- |
| A node failed or was interrupted, but the run is still active | The node is waiting for manual retry | Read the error, adjust the model if needed, then select **Retry** or send a correction in that node's composer. See [Retry a node](../guides/runs.md#retry-a-failed-or-interrupted-node). |
| **Continue** is not available | The run is still active, the current app session has no recoverable checkpoint, a different run is selected, or the run completed | Answer active questions and approvals in the conversation. For a durable paused, stopped, or failed run, open **History** and use **Resume run** when offered. |
| A scheduled workflow did not start | The app was closed, the schedule is Off, the workflow/provider is not ready, or the project `.flow` directory is not writable | Start OpenFlow, turn the row On, check **Next**, confirm readiness and project write access, then inspect **Last** and History. See [Scheduling](../guides/scheduling.md). |
| A run pauses on a tool or question | Human input or tool approval is pending | Answer the question or approve/deny the tool in the node conversation. Submission resumes the active run without **Continue**. Stopping the run cancels pending callbacks. |

<a id="focused-verification"></a>
<a id="rust-build-disk-usage"></a>

For source-checkout failures, use [Build and test troubleshooting](../contributing/build-troubleshooting.md).
