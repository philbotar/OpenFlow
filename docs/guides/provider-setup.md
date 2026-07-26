# Provider setup

Configure the model backend OpenFlow uses for workflow runs, agent chat, workflow authoring, and post-run review. Without a ready provider, **Run** stays disabled and the header readiness chip reports what is missing.

Prerequisites: OpenFlow installed or running from [`../getting-started/README.md`](../getting-started/README.md).

## First launch

On first open, the onboarding carousel ends with **Set up provider →**. That dismisses onboarding, opens **Settings**, and selects **Providers**.

You can open the same screen anytime: sidebar **Settings** → **Providers**.

## Choose the active provider

In **Settings → Providers**:

1. Under **Active provider**, pick a row from the **Provider** dropdown (for example **OpenAI**, **Anthropic**, **ChatGPT (Codex)**, **Amazon Bedrock**, or a hosted OpenAI-compatible vendor).
2. Complete authentication for that profile (next section).
3. Adjust models, reasoning, or endpoint fields if needed.
4. Select **Save settings**.

The summary line states that the active profile is used for workflow runs and agent chat. A readiness chip on this page mirrors the header: **Ready** when configuration resolves, or a specific missing step otherwise.

## Authenticate by provider type

### API key providers

Applies to **OpenAI**, **Anthropic**, **OpenRouter**, local **Ollama** / **LM Studio**, **Custom OpenAI-compatible API**, and similar profiles.

1. Open the **API key** panel.
2. Paste a key into the key field. Readiness can turn **Ready** before you save; the typed value is also passed as the transient key for runs until you leave the session.
3. Select **Save settings** to persist the key in local `settings.json` (plaintext on disk).

Alternatively, export the provider environment variable (for example `OPENAI_API_KEY` or `ANTHROPIC_API_KEY`) before launching OpenFlow. Resolution order: transient key from the Settings field during a session, stored profile key, then environment variable. See [`../reference/README.md#provider-key-resolution`](../reference/README.md#provider-key-resolution).

### ChatGPT (Codex)

1. Select **ChatGPT (Codex)** as the active provider.
2. In the **ChatGPT account** panel, select **Sign in with ChatGPT** and complete the browser callback on `http://localhost:1455/auth/callback`, or the device-code flow if port 1455 is in use.
3. When connected, readiness reports **Ready**. OAuth tokens stay in `settings.json`; the UI never shows them. Use **Disconnect** to remove the session.

Codex does not use `OPENAI_API_KEY`. Billing and entitlement follow the ChatGPT account, not OpenAI API usage. Failures: [`../troubleshooting/README.md#chatgpt-codex-sign-in`](../troubleshooting/README.md#chatgpt-codex-sign-in).

### Amazon Bedrock

1. Select **Amazon Bedrock** as the active provider.
2. Set **AWS profile** and **AWS region** (or rely on `AWS_PROFILE` / `AWS_REGION` when OpenFlow inherits your shell environment).
3. Select **Save settings**, then **Test AWS connection** to verify the credential chain.
4. Optionally **Refresh from AWS** to load foundation model IDs into the profile.

Readiness may show region configured before credentials are verified; use **Test AWS connection** to confirm. SSO and GUI `HOME` behavior: [`../architecture/provider-adapters.md#bedrock-with-sso`](../architecture/provider-adapters.md#bedrock-with-sso).

## Built-in providers

Each row matches the **Provider** dropdown in **Settings → Providers**. Setup steps are the same within an auth group above; this table lists credentials, default endpoints, and default wire API.

| Settings label | Auth group | Env fallback | Default base URL | Default wire API |
| --- | --- | --- | --- | --- |
| OpenAI | API key | `OPENAI_API_KEY` | `https://api.openai.com` | Responses |
| Anthropic | API key | `ANTHROPIC_API_KEY` | `https://api.anthropic.com` | Anthropic Messages |
| ChatGPT (Codex) | ChatGPT account | _(none — OAuth only)_ | ChatGPT Codex backend | Codex (OpenFlow OAuth + Rig) |
| OpenRouter | API key | `OPENROUTER_API_KEY` | `https://openrouter.ai/api/v1` | Chat Completions |
| Groq | API key | `GROQ_API_KEY` | `https://api.groq.com/openai/v1` | Chat Completions |
| Together AI | API key | `TOGETHER_API_KEY` | `https://api.together.xyz/v1` | Chat Completions |
| Fireworks AI | API key | `FIREWORKS_API_KEY` | `https://api.fireworks.ai/inference/v1` | Chat Completions |
| DeepSeek | API key | `DEEPSEEK_API_KEY` | `https://api.deepseek.com/v1` | Chat Completions |
| xAI / Grok | API key | `XAI_API_KEY` | `https://api.x.ai/v1` | Chat Completions |
| Mistral AI | API key | `MISTRAL_API_KEY` | `https://api.mistral.ai/v1` | Chat Completions |
| Perplexity | API key | `PERPLEXITY_API_KEY` | `https://api.perplexity.ai` | Chat Completions |
| Gemini OpenAI compatibility | API key | `GEMINI_API_KEY` | `https://generativelanguage.googleapis.com/v1beta/openai` | Chat Completions |
| Ollama local | API key optional | _(none)_ | `http://localhost:11434/v1` | Chat Completions |
| LM Studio local | API key optional | _(none)_ | `http://localhost:1234/v1` | Chat Completions |
| Custom OpenAI-compatible API | API key | `OPENAI_COMPATIBLE_API_KEY` | Editable (default `http://localhost:11434/v1`) | Chat Completions (editable) |
| Amazon Bedrock | AWS credentials | `AWS_PROFILE`, `AWS_REGION` | _(region-driven)_ | Bedrock Converse (Rig) |

For **Ollama local** and **LM Studio local**, start the local server before **Run**; an API key is not required unless you configure one. For **Custom OpenAI-compatible API**, the profile shows **Custom endpoint**; edit base URL and default wire API there, then **Save settings**.

Source of truth for IDs, models, and auth rules: `crates/providers/src/spec.rs` (`builtin_provider_specs`).

## Optional profile tuning

| Control | When to use it |
| --- | --- |
| **Known models** / add model | Pick models that appear on agent nodes for this provider. |
| Per-model transport | Override **Responses API**, **Chat Completions API**, or **Anthropic Messages API** for a specific model on custom OpenAI-compatible profiles. |
| Reasoning effort | Set default extended-thinking effort where the provider supports it. |
| **Custom endpoint** profile | Edit base URL and default wire API for self-hosted or third-party OpenAI-compatible gateways. |

Node-level model fields still select which model each agent calls; the provider profile supplies credentials and defaults.

## Verify

| Check | Expected |
| --- | --- |
| **Settings → Providers** readiness chip | **Ready** (Bedrock may prompt you to test AWS separately). |
| Editor header readiness chip | **Ready**; **Run** enabled when no blocking run state. |
| Test run | Open a workflow and select **Run**; a missing key shows **API key missing** or Codex sign-in text in the chip. |

If readiness fails after setup, see [`../troubleshooting/README.md#provider-not-ready`](../troubleshooting/README.md#provider-not-ready).

## How providers connect to a run (and Rig)

OpenFlow splits responsibilities so workflows stay provider-agnostic:

```text
Workflow run (engine)
  → AgentRequest (prompt, tools, model id)
    → AiPort (engine port)
      → orchestration resolves settings + keys
      → providers::create_provider(...)
        → mapping/ (transcript + tool wire shape)
        → rig_adapter/ (Rig HTTP + streaming to the vendor API)
```

During a run, the **engine** only sees `AiPort`: one interface for model turns, streaming, and tool-call results. **Orchestration** loads `settings.json`, merges secrets, resolves the active `ProviderProfile`, and builds an `AiClientConfig`. It then calls **`create_provider()`** in `crates/providers`, which returns a boxed `AiPort` implementation.

Inside **`crates/providers`**, **Rig** (`rig-core` 0.39) owns most outbound HTTP and SSE streaming to vendor APIs. OpenFlow code around Rig handles:

- `AgentRequest` ↔ provider payload conversion (`mapping/`, `rig_adapter/convert.rs`);
- API keys, Anthropic headers, and OpenAI-compatible base URLs (`auth.rs`, profile fields);
- ChatGPT Codex OAuth, refresh, and Codex-specific endpoints (`codex_oauth/`, `codex.rs`);
- Amazon Bedrock credentials and Converse calls when the desktop build enables the `bedrock` feature (`aws_runtime.rs`).

Orchestration does not import Rig or vendor SDKs directly; it only uses the factory and config types from `providers`. That keeps run logic in the engine and transport quirks in one crate.

### OpenAI-compatible vs Anthropic native wire

OpenFlow does **not** call the official OpenAI or Anthropic client SDKs from workflow code. The `providers` crate turns each run into HTTP through **`rig-core`**, with OpenFlow `mapping/` building Rig `CompletionRequest` values from `AgentRequest`.

`create_provider()` picks an adapter family from the active builtin profile (see table above):

| Profile family | `ProviderAdapterConfig` | Rig model (simplified) | HTTP shape |
| --- | --- | --- | --- |
| **Anthropic** | `Anthropic` | `rig_core::providers::anthropic` completion model | Anthropic **Messages** API (`/v1/messages`), `x-api-key` + Anthropic version headers |
| **OpenAI** and most hosted vendors | `OpenAiCompatible` | `rig_core::providers::openai` | Default **Responses** API for OpenAI; **Chat Completions** for OpenRouter, Groq, Gemini compat, locals, etc. |
| Per-model override on a compat profile | `OpenAiCompatible` + `model_transports` | Same Rig Anthropic client against the profile **base URL** | **Anthropic Messages** on a non-Anthropic host (gateway or proxy) |
| **ChatGPT (Codex)** | `OpenAiCodex` (separate `CodexClient`) | Rig ChatGPT responses + OpenFlow OAuth | Private Codex backend (not `api.openai.com` API-key traffic) |
| **Amazon Bedrock** | `Bedrock` | `rig-bedrock` completion model | AWS Bedrock **Converse** (not Anthropic Messages on `api.anthropic.com`) |

At invoke time, `rig_adapter` builds one `RigModel` enum arm (Anthropic, OpenAiChat, OpenAiResponses, ChatGPT, or Bedrock), runs the turn, then maps the result back to `AgentTurnOutcome`. The engine always sees the same `AiPort` regardless of which arm ran.

Choosing **Anthropic** vs **OpenAI** in Settings therefore changes **wire protocol and auth**, not workflow JSON. Choosing **OpenAI** vs **Groq** on OpenAI-compatible paths shares the same adapter type; only base URL, key env var, and default Chat Completions vs Responses differ per `spec.rs` profile.

UI control for transport: **Settings → Providers** → per-model **Responses API**, **Chat Completions API**, or **Anthropic Messages API** on editable or custom profiles. Managed OpenAI defaults to Responses; most other compat profiles default to Chat Completions.

Adapter matrix, Codex compatibility boundary, and output repair: [`../architecture/provider-adapters.md`](../architecture/provider-adapters.md). Runtime placement in a full run: [`../architecture/end-to-end-runtime.md`](../architecture/end-to-end-runtime.md).

## Related

- [`first-workflow.md`](first-workflow.md) — run after the provider is ready.
- [`using-the-app.md`](using-the-app.md) — **Search** keys (separate from LLM providers) and MCP.
- [`../concepts/how-openflow-works.md`](../concepts/how-openflow-works.md) — UI → orchestration → engine → providers path.
