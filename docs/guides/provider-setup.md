# Provider setup

Open **Settings → Providers** to connect the model backend for runs, agent chat, workflow authoring, and post-run review. **Run** stays disabled until a provider is ready; the header chip reports what is missing.

On first launch, **Set up provider →** at the end of onboarding opens the same screen. Install instructions are in [Install OpenFlow](../getting-started/README.md).

## Without a provider

You can still create workflows, edit nodes, and save graphs when no API key or Codex session is configured. **Run**, **Create with AI**, and other model-backed actions stay disabled until readiness shows **Ready**.

## Choose the active provider

In **Settings → Providers**:

1. Under **Active provider**, pick a row from the **Provider** dropdown (for example **OpenAI**, **Anthropic**, **ChatGPT (Codex)**, **Amazon Bedrock**, or a hosted OpenAI-compatible vendor).
2. Authenticate the profile using the matching instructions below.
3. Adjust models, reasoning, or endpoint fields if needed.
4. Provider profile changes save automatically. For API-key providers, select **Save API key** after entering or replacing the key.

The active profile is the default for agent chat and workflows that do not select a shared provider. A readiness chip on this page mirrors the header: **Ready** when configuration resolves, or a specific missing step otherwise.

## Authenticate by provider type

### API key providers

**OpenAI**, **Anthropic**, **OpenRouter**, and other managed API-key profiles require a key. Local **Ollama** / **LM Studio** and **Custom OpenAI-compatible API** profiles allow an empty key.

1. Open the **API key** panel.
2. Paste a key into the key field when the endpoint requires one. For a custom endpoint reached through localhost, a VPN, or another trusted network, leave the optional field empty.
3. Readiness can turn **Ready** before you save; the typed value is also passed as the transient key for runs until you leave the session.
4. Select **Save API key** to persist the key in local `settings.json` (plaintext on disk). Other provider fields save automatically.

Alternatively, export the provider environment variable (for example `OPENAI_API_KEY` or `ANTHROPIC_API_KEY`) before launching OpenFlow. The [provider profile reference](../reference/providers.md#provider-key-resolution) lists key precedence and environment variables.

### ChatGPT (Codex)

1. Select **ChatGPT (Codex)** as the active provider.
2. In the **ChatGPT account** panel, select **Sign in with ChatGPT** and complete the browser callback on `http://localhost:1455/auth/callback`, or the device-code flow if port 1455 is in use.
3. When connected, readiness reports **Ready**. OAuth tokens stay in `settings.json`; the UI never shows them. Use **Disconnect** to remove the session.

Codex does not use `OPENAI_API_KEY`. Billing and entitlement follow the ChatGPT account, not OpenAI API usage. For failed callbacks or expired sessions, follow [ChatGPT (Codex) sign-in troubleshooting](../troubleshooting/README.md#chatgpt-codex-sign-in).

### Amazon Bedrock

1. Select **Amazon Bedrock** as the active provider.
2. Set **AWS profile** and **AWS region** (or rely on `AWS_PROFILE` / `AWS_REGION` when OpenFlow inherits your shell environment).
3. Wait for the automatic settings save, then select **Test AWS connection** to verify the credential chain.
4. Optionally **Fetch from AWS**, then select the foundation model IDs to expose in OpenFlow.

Readiness may show region configured before credentials are verified; use **Test AWS connection** to confirm. For SSO credentials and differences in the GUI’s `HOME`, read [Bedrock with SSO](../architecture/provider-adapters.md#bedrock-with-sso).

## Local and custom endpoints

For **Ollama local** and **LM Studio local**, start the local server before **Run**; an API key is not required. For **Custom OpenAI-compatible API**, the profile shows **Custom endpoint**; set **API base URL** in the main provider card. Leave the key empty for a trusted no-auth endpoint, or add one when the endpoint requires auth.

## Optional profile tuning

| Control | When to use it |
| --- | --- |
| **Fetch models** / **Fetch from AWS** | Query the active provider, then select multiple model IDs to expose in OpenFlow. |
| **Known models** / add model | Review selected models or add a model ID the API did not return. |
| Per-model transport | Override **Responses API**, **Chat Completions API**, or **Anthropic Messages API** for a specific model on custom OpenAI-compatible profiles. |
| Reasoning effort | Set default extended-thinking effort where the provider supports it. |
| **Custom endpoint** profile | Edit base URL and default wire API for self-hosted or third-party OpenAI-compatible gateways. |

In **Workflow settings**, select the shared provider. In a node's **Inspector**, leave **Provider** on **Use shared provider** or select a node override. Resolution order: node provider → shared workflow provider → active provider. The selected profile supplies that node's credentials, model list, and defaults.

## Verify

1. Confirm **Settings → Providers** shows **Ready**. Bedrock instead shows **Region configured — use Test AWS connection to verify credentials**; select **Test AWS connection** to check those credentials.
2. Open **New chat** and send a short question. Confirm that a response arrives. **Ready** means the configuration resolves; a successful response also checks that the provider accepts a model request.
3. In the workflow editor, the provider should be configured and **Run** should be enabled when no run state blocks it. A missing key shows **API key missing**, or Codex sign-in text for an unconnected ChatGPT account.

If readiness still fails, follow [Provider not ready](../troubleshooting/README.md#provider-not-ready).

With the connection ready, [send a message in a direct chat](direct-chats.md) or [run the two-agent walkthrough](first-workflow.md).

<a id="built-in-providers"></a>

Default endpoints, authentication groups, and wire APIs are listed in [Provider profiles](../reference/providers.md). Web search and MCP have [separate connection settings](tools-and-integrations.md).

<a id="how-providers-connect-to-a-run-and-rig"></a>
<a id="openai-compatible-vs-anthropic-native-wire"></a>

For transport implementation, use [Request routing](../architecture/provider-adapters.md#request-routing).
