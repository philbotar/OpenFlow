# Troubleshooting

Find your symptom below and keep the exact error text for retries or reports. Controls are in [Using the app](../guides/using-the-app.md); recovery actions and limits are in [Runs and recovery](../guides/runs.md) and [Configuration](../reference/configuration.md).

## ChatGPT Codex sign-in

| Symptom | Cause | Fix |
| --- | --- | --- |
| Device code instead of a browser callback | Another process owns loopback port `1455` | Complete the device flow, or stop that process and retry. |
| Browser opened, OpenFlow keeps waiting | Callback didn't reach `http://localhost:1455/auth/callback`, or the state/callback expired | Allow loopback traffic in local firewall/proxy tools. Retry if expired. |
| **Sign in with ChatGPT** after a previous connection | Refresh session missing, invalidated, reused, or expired | Sign in again. |
| Workspace/entitlement rejection | Selected account or workspace lacks Codex access | Switch accounts or ask the workspace administrator. |
| HTTP 403 from the Codex backend | The private ChatGPT backend may have changed or rejected third-party `originator: openflow` | An OpenAI API key won't fix this. Verify current official Codex behavior and the documented compatibility boundary. |
| Usage/rate-limit error | ChatGPT plan/workspace limits, separate from OpenAI API billing | Check those limits. |

OAuth credentials are plain text in the local OpenFlow `settings.json`, like stored provider API keys. Normal settings IPC and logs redact them. In **Settings → Providers**, with **ChatGPT (Codex)** selected, **Disconnect** deletes them.

## Provider not ready

| Symptom | Cause | Fix |
| --- | --- | --- |
| Header shows API key missing, or **Run** stays disabled | No resolvable key for the active API-key provider | In **Settings → Providers**, select the active profile, add a key or set the provider env var (such as `OPENAI_API_KEY`), and save. |
| Readiness message on the authoring or run screen | Same, or a shared/node provider is incomplete | Give every provider the workflow references valid auth and a model, or switch profile or model. |
| ChatGPT (Codex) not ready | No stored OAuth session, or expired login | With **ChatGPT (Codex)** active in **Settings → Providers**, select **Sign in with ChatGPT**, or **Disconnect** and sign in again. See [ChatGPT Codex sign-in](#chatgpt-codex-sign-in). |
| Bedrock not ready | AWS credentials or region misconfigured | Set profile/region in the Bedrock provider panel and verify the credential chain. |

How credentials resolve ([full rules](../reference/providers.md#provider-key-resolution)):

- **API-key providers**: transient run input, then the stored `settings.json` key, then the environment variable.
- **ChatGPT (Codex)**: needs a stored refreshable login; ignores `OPENAI_API_KEY`.
- **Bedrock**: the AWS credential chain and configured region/profile.

## Chat attachments

| Symptom | Cause | Fix |
| --- | --- | --- |
| File stays pending with a size/type error | More than 4 files, one over 10 MiB, total over 25 MiB, unsupported extension, empty file, or content/type mismatch | Remove it, or export it as JPEG, PNG, GIF, WebP, PDF, TXT, Markdown, CSV, JSON, HTML, CSS, JavaScript, or Python, and retry. |
| Provider rejects an accepted image or document | The transport can serialize it, but the selected model/account doesn't accept that media | Switch to a media-capable model for that provider. Custom models remain provider-dependent. |
| “missing or corrupt” attachment | Managed run copy is absent, changed, or fails its saved size/hash/type checks | Reattach the original. OpenFlow rejects the request before provider HTTP. |
| Image card has no preview | Bounded local preview failed, or its run data is unavailable | The message still shows as a generic attachment card. Reattach only if the provider also reports an attachment error. |
| Chat deletion reports cleanup pending | Metadata was deleted, but run-directory cleanup didn't finish | Restart or open run history to retry quarantined cleanup. Don't hand-edit `chats.json`. |

## MCP servers

| State / symptom | Cause | Fix |
| --- | --- | --- |
| **Not installed** | Registry package metadata exists without a completed exact-version install | Review the package, version, and command, install it, then run **Approve & Test**. |
| **Needs config** | Command or required environment/header input is empty | Enter it on the server row in **Settings → MCP Servers**, save, and test again. Credential values go in `mcp-secrets.json`, out of settings payloads and exports. |
| **Auth required** | OAuth token absent, expired without refresh, revoked, or rejected | Use **Connect OAuth** or **Re-authenticate**, verify the displayed scopes and issuer, and test again. |
| **MCP command `npx` was not found** | OpenFlow launched outside your interactive shell, so the GUI inherited a minimal `PATH` | Confirm `command -v npx` in the embedded **Terminal**, then retry the test. The Terminal, bash tools, and MCP probes, installs, and runs resolve the same login-shell `PATH`; explicit command or MCP `PATH` config still wins. |
| **OAuth browser does not open** | Protected-resource discovery, authorization-server discovery, PKCE validation, or client registration failed before browser launch | Read the row's OAuth setup error and retry **Authenticate OAuth** (any remote server). Fix the server's OAuth metadata, or enter its pre-registered client ID if dynamic registration is unavailable. A changed authorization-server issuer triggers fresh registration automatically. |
| **Atlassian OAuth does not open** | Older Rovo URLs (`/v1/sse` or `/v1/mcp`) use retired or incomplete OAuth discovery metadata | Re-import or retry the server. OpenFlow migrates Atlassian Rovo to `https://mcp.atlassian.com/v1/mcp/authv2`, refreshes stale DCR metadata, and starts OAuth from the current authorization server. |
| **Atlassian consent rejects the callback** | Atlassian organization policy blocks the local callback origin | Ask an Atlassian admin to allow `http://localhost:*/**` and `http://127.0.0.1:*/**` in Rovo MCP domain settings, then retry in a fresh browser session. |
| **Failed · Preflight** | Missing executable, rejected remote URL, unavailable secret ref, or stale trust | Read the exact row error, fix config or runtime, then **Retry**. Security-relevant changes need fresh approval. |
| **Failed · Connect/List tools/Close** | Process/HTTP initialization, capability negotiation, pagination, or shutdown failed | Use **Copy diagnostics**. Retry only as a user action; OpenFlow doesn't replay ambiguous tool calls automatically. |
| Enabled server skipped during a run | That server failed setup; others continue | Read the system chat message, disable or fix the named server, and restart the run if you need its tools. |
| Sampling/elicitation card appears | An allowed server made a server-to-client request from an active MCP tool | Review the server and originating tool, then approve once or deny. **Stop** cancels pending callbacks. |

Local children run only during probes and runs; OpenFlow keeps no background MCP process. Remote endpoints require HTTPS unless explicit loopback development is enabled. Private/link-local targets and unsafe redirects stay blocked.

## Workflow validation and authoring

| Symptom | Cause | Fix |
| --- | --- | --- |
| **Run** disabled, or canvas shows validation errors | Empty graph, cycle, missing endpoint, or invalid schema or handoff | Fix the highlighted node or edge, keeping every Markdown handoff heading. Confirm validation clears before selecting **Run**. See [Workflow authoring](../guides/workflow-authoring.md). |
| **Create with AI** made a proposal but no workflow | Proposals aren't saved until **Create Workflow** or **Apply Changes** succeeds | Review the graph and use that action. For a draft left open, choose **Resume AI workflow draft**. |
| Project workflow can't be saved or run | `.flow` missing or not writable, or the project is being removed while active | Confirm the folder is writable, stop the active run, and retry. Project files stay under `.flow/`; see [Projects and persistence](../guides/projects-and-persistence.md). |

## Runs, recovery, and schedules

| Symptom | Cause | Fix |
| --- | --- | --- |
| Node failed or was interrupted, but the run is still active | The node is waiting for manual retry | Read the error, change the model if needed, then select **Retry** or send a correction in that node's composer. See [Retry a node](../guides/runs.md#retry-a-failed-or-interrupted-node). |
| **Continue** isn't available | Run still active, no recoverable checkpoint in this app session, a different run selected, or run completed | Answer active questions and approvals in the conversation. For a durable paused, stopped, or failed run, use **Resume run** in **History** when offered. |
| Scheduled workflow didn't start | App closed, schedule Off, workflow/provider not ready, or project `.flow` not writable | Start OpenFlow, turn the row On, and check **Next**. Confirm readiness and project write access, then inspect **Last** and History. See [Scheduling](../guides/scheduling.md). |
| Run pauses on a tool or question | Human input or tool approval is pending | Answer, or approve/deny the tool, in the node conversation. Submitting resumes the active run without **Continue**. Stopping the run cancels pending callbacks. |

<a id="focused-verification"></a>
<a id="rust-build-disk-usage"></a>

For source-checkout failures, see [Build and test troubleshooting](../contributing/build-troubleshooting.md).
