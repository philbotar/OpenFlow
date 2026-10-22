# Provider adapters

`crates/providers` implements the outbound LLM adapters behind `engine::AiPort`, with transport through **Rig 0.39** (`rig-core`) in `rig_adapter/`. Orchestration resolves settings into provider configuration and calls `create_provider()`; it never depends on concrete provider clients.

For keys, OAuth, and readiness controls, follow [Provider setup](../guides/provider-setup.md).

## Request routing

```text
Workflow run (engine)
  → AgentRequest (prompt, tools, provider id, model id)
    → AiPort (engine port)
      → orchestration routes to the effective provider
      → providers::create_provider(...) for each referenced provider
        → mapping/ (transcript + tool wire shape)
        → rig_adapter/ (Rig HTTP + streaming to the vendor API)
```

- **Engine** owns run logic and sees only `AiPort`: one interface for model turns, streaming, and tool-call results.
- **Orchestration** loads `settings.json`, merges secrets, resolves every provider the workflow references, validates all their credentials before execution, and builds a run-scoped router over those clients. It imports only factory and config types from `providers`, never Rig or vendor SDKs.
- **`crates/providers`** owns transport. Rig does most outbound HTTP and SSE streaming. OpenFlow code around it handles:
  - `AgentRequest` ↔ payload conversion (`mapping/`, `rig_adapter/convert.rs`).
  - API keys, Anthropic headers, OpenAI-compatible base URLs (`auth.rs`, profile fields).
  - ChatGPT Codex OAuth, refresh, and endpoints (`codex_oauth/`, `codex.rs`).
  - Bedrock credentials and Converse calls when the desktop build enables the `bedrock` feature (`aws_runtime.rs`).

A provider failure stays attached to its node. OpenFlow never retries through a different provider.

### Wire protocols

`mapping/` builds Rig `CompletionRequest` values from `AgentRequest`; workflow code never calls OpenAI or Anthropic SDKs. `create_provider()` picks an adapter family from the active builtin profile ([built-in profiles](../reference/providers.md)):

| Profile family | `ProviderAdapterConfig` | Rig model (simplified) | HTTP shape |
| --- | --- | --- | --- |
| **Anthropic** | `Anthropic` | `rig_core::providers::anthropic` completion model | Anthropic **Messages** API (`/v1/messages`), `x-api-key` + Anthropic version headers |
| **OpenAI** and most hosted vendors | `OpenAiCompatible` | `rig_core::providers::openai` | Default **Responses** API for OpenAI; **Chat Completions** for OpenRouter, Groq, Gemini compat, locals, etc. |
| Per-model override on a compat profile | `OpenAiCompatible` + `model_transports` | Same Rig Anthropic client against the profile **base URL** | **Anthropic Messages** on a non-Anthropic host (gateway or proxy) |
| **ChatGPT (Codex)** | `OpenAiCodex` (separate `CodexClient`) | Rig ChatGPT responses + OpenFlow OAuth | Private Codex backend (not `api.openai.com` API-key traffic) |
| **Amazon Bedrock** | `Bedrock` | `rig-bedrock` completion model | AWS Bedrock **Converse** (not Anthropic Messages on `api.anthropic.com`) |

At invoke time, `rig_adapter` builds one `RigModel` enum arm (Anthropic, OpenAiChat, OpenAiResponses, ChatGPT, or Bedrock), runs the turn, and maps the result to `AgentTurnOutcome`. The engine sees the same `AiPort` whichever arm ran.

Switching **Anthropic** and **OpenAI** in Settings changes wire protocol and auth, not workflow JSON. **OpenAI** and **Groq** share the OpenAI-compatible adapter type; `spec.rs` gives each its base URL, key environment variable, and default transport. Managed OpenAI defaults to Responses; most other compat profiles default to Chat Completions.

## Supported adapter families

| Adapter family | Implementation | Role |
| --- | --- | --- |
| OpenAI-compatible (via Rig) | `rig_adapter/model.rs` + `convert.rs` | Chat completions / Responses wire shape, tools, streaming |
| ChatGPT (Codex) OAuth + Rig | `codex_oauth/`, `codex.rs`, `rig_adapter/model.rs`, and `rig_adapter/codex_http.rs` | Browser/device login, refreshable credentials, ChatGPT Codex Responses streaming |
| Anthropic Messages (via Rig) | `rig_adapter/` (+ `anthropic_http.rs`, `claude_thinking.rs`) | Anthropic-native mapping, thinking blocks, prompt cache |
| Amazon Bedrock Converse (via Rig) | `rig_adapter/model.rs` + `aws_runtime.rs` | AWS Bedrock transport and credential resolution (`bedrock` Cargo feature; off by default on `providers`/`orchestration`, on via `desktop`) |
| Shared mapping | `crates/providers/src/mapping/` | Transcript conversion, tool argument parsing, `jsonrepair-rs` recovery |
| Factory | `crates/providers/src/lib.rs` | `create_provider()` returns `Box<dyn AiPort>` |
| Client entry | `crates/providers/src/client.rs` | `AiClient: AiPort`, config enums, model cache |

- Buffered and streaming `AiClient` calls share dispatch in `rig_adapter/mod.rs`; model construction selects the transport.
- The per-client cache reuses models and rebuilds Bedrock models before credential expiry.
- Each invocation picks the model and builds its payload from the current `AgentRequest`, including transcript and reasoning/fast-mode settings.
- Codex keeps its own refreshable client but shares the underlying Rig model invocation methods.
- Inference and HTTP model discovery share key/header validation in `auth.rs`. Discovery keeps its shorter total timeout.
- Tool schemas use Rig's tool definition directly inside providers. Engine contracts stay independent of Rig.

## Provider prompt caching

Prompt caching reuses exact prompt prefixes. It is separate from OpenFlow's validated tool-result cache.

| Provider path | OpenFlow behavior |
| --- | --- |
| OpenAI API, Responses | Stable `prompt_cache_key` per workflow node. GPT-5.6 models add `prompt_cache_options.mode: "explicit"` and a breakpoint on the final stable system block, so changing user/tool history triggers no billable cache write. |
| OpenAI API, Chat Completions | Same per-node `prompt_cache_key`; the provider picks the automatic breakpoint. |
| Anthropic Messages | Automatic moving-history caching plus explicit stable system and tool-definition markers. Anthropic's default five-minute TTL. |
| Anthropic-compatible model override | Same Messages policy. Backend support is part of that endpoint's compatibility contract. |
| Amazon Bedrock Converse | Rig prompt caching only for the Claude and Nova model-ID allowlist in `rig_adapter/model.rs`; other models get no cache points. |
| Other OpenAI-compatible endpoints | `prompt_cache_key`, except Ollama and LM Studio. Acceptance, discounts, and lifetime are provider-defined. |
| ChatGPT (Codex) OAuth | No OpenFlow-managed cache controls; the private backend contract does not document them. |

- Cache reads map to `UsageReport.cached_input_tokens`, writes to `UsageReport.cache_creation_input_tokens`. Orchestration projects the latest values into each node's `ContextWindowSnapshot` as `cachedInputTokens` and `cacheCreationInputTokens`.
- OpenAI Responses needs a final-wire shim: Rig 0.39 does not yet model `prompt_cache_key`, `prompt_cache_options`, or `cache_write_tokens`.
- Caching starts only above the provider's minimum prompt size. Cache keys improve routing; they do not memoize responses or change output semantics.
- Put stable system context and tool definitions first, run-specific user input and tool results later.

## Per-model transport routing

Custom OpenAI-compatible profiles have a default Responses or Chat Completions transport. A model overrides it through `ProviderProfile.model_transports` (`responses`, `chat_completions`, or `anthropic_messages`), set per model in **Settings → Providers** as **Responses API**, **Chat Completions API**, or **Anthropic Messages API** on editable or custom profiles. Settings stores model IDs as data; provider code never infers transport from vendor names or model allowlists.

An `anthropic_messages` override uses the Rig Anthropic client against the profile base URL: standard `/v1/messages` request, Anthropic auth/version headers, tool schema, and response mapping. Removing the override restores the profile default.

OpenAI Responses and ChatGPT (Codex) preserve MCP schemas with map-like object parameters. Rig 0.39 marks every Responses function tool strict, but OpenAI strict mode rejects objects that allow additional properties. So the final-wire adapters set `strict: false` on affected tools only, without closing or rewriting the MCP server's schema. Closed schemas stay strict.

An adapter that must invoke without streaming keeps typed reasoning on the turn outcome. Its fallback stream emits displayable text and summary blocks before assistant text, never encrypted or redacted reasoning payloads.

## Fast mode

Fast mode is an independent request setting; it does not select or rewrite reasoning effort.

- OpenAI API and ChatGPT (Codex) map the saved `fastMode` boolean to `service_tier: "priority"`. Other providers lack this control.
- The ChatGPT UI name **Fast** and Codex config value `fast` are not wire-level service tiers.
- Rig's ChatGPT adapter clears `service_tier` while building the request. `CodexHttpClient` restores the selected priority tier at the final HTTP boundary, keeping Rig's request and stream mapping.

## Stream cutoff recovery

Rig streams tool-call names and argument deltas before the final response; OpenFlow accumulates them by Rig's internal call ID. If a Responses stream ends with `response.incomplete: max_output_tokens`, the provider returns `AgentError::OutputTruncated` with redacted partial-call metadata, rather than dropping the in-flight call or reporting a generic provider failure. Raw partial arguments stay in memory, omitted from `Debug`, `Display`, telemetry, and chat projection.

- The engine executes no call from the incomplete response. It retries the node up to three times, instructing the model to write a small valid skeleton and expand it through focused edits.
- Claude 4 models recognized by Rig keep Rig's larger model-specific output limit. Other Anthropic-compatible models get a 16K minimum fallback.
- Successful edit-tool writes use a sibling temporary file plus atomic rename, so cancellation never exposes a partially written destination.

## Deterministic recovery and overseer repair

Providers own wire decoding and local `jsonrepair-rs` recovery. Semantic acceptance of `openflow_submit_node_output` lives in the engine completion protocol (`execution/completion_protocol.rs`).

When deterministic recovery cannot satisfy the node output schema, providers attach a redacted `OutputRepairCandidate` to `AgentError::MalformedSubmitOutput`. Orchestration wraps the run provider once in `RepairingAiPort` (before `AiInvocationAdapter`), so nodes and subagents share one overseer pass on the **same** provider.

Malformed wire-argument markers keep at most 64 KiB of raw UTF-8 text, ending on a character boundary. The engine caps the repair candidate independently.

`WorkflowSettings.output_repair_model` (`outputRepairModel`) optionally overrides the overseer model. Blank or absent inherits the failed worker request's model; nonblank uses that model string. Credentials stay on the run provider. [Overseer output repair](output-repair.md) covers the sequence, guards, and deferred scope.

## ChatGPT (Codex) OAuth

**ChatGPT (Codex)** authenticates with a ChatGPT account, not an OpenAI API key. OpenFlow owns login, token refresh, persistence, and one-shot unauthorized recovery. Rig's ChatGPT provider supplies the Codex Responses request and SSE mapping.

| Concern | Behavior |
| --- | --- |
| Browser login | PKCE S256 on `http://localhost:1455/auth/callback`, loopback-only listener, exact state validation |
| Port conflict | Device-code flow, only when port 1455 cannot be bound |
| Refresh | Within five minutes of expiry; concurrent refreshes serialized; rotated credentials persisted; then one unauthorized request retried |
| Cancellation | Drops the whole pending login attempt (stalled callback reads, device requests, token exchange) and releases the callback listener |
| Inference | SSE `POST /backend-api/codex/responses` through Rig with `originator: openflow` and `OpenAI-Beta: responses=experimental`; no WebSocket |
| Storage | Access, refresh, ID token, expiry, and account metadata in plaintext in `ProviderProfile.codex_oauth` inside `settings.json` |
| UI boundary | Settings IPC returns only tagged login state, safe email, and device instructions; never tokens or account ID |

OpenFlow implements the OAuth protocol in Rust. Endpoint, PKCE, token-body, and request-shape details derive from [oh-my-pi's MIT-licensed implementation](https://github.com/badlogic/pi-mono/tree/main/packages/ai/src/auth/oauth) and are pinned by fixture tests. Unlike the Codex CLI, OpenFlow intentionally falls back to device authorization, not a second callback port, when port 1455 is occupied.

### Compatibility boundary

`chatgpt.com/backend-api/codex` and the reused Codex OAuth client ID are private, unsupported contracts. They can change independently of the public OpenAI API, and ChatGPT workspace policy may reject third-party clients or `originator: openflow`. Fixture tests prove the current request contract; only an interactive account smoke proves entitlement and live backend acceptance.

Never impersonate a first-party originator. If the backend rejects OpenFlow, surface the error, verify current official Codex behavior, then update the documented adapter contract.

## Amazon Bedrock

Bedrock uses AWS credentials and region settings, not an API-key header.

| Concern | Source |
| --- | --- |
| Auth | AWS credential chain (`AWS_ACCESS_KEY_ID` / `AWS_SECRET_ACCESS_KEY`, SSO, shared config, instance role). Optional profile name in Settings (`ProviderProfile.aws_profile`) or `AWS_PROFILE`. |
| Region | Settings **AWS region** (`ProviderProfile.aws_region`, default `us-east-1`) or `AWS_REGION`. A Bedrock region in older settings' `ProviderProfile.base_url` is migrated on load. |
| Inference | Bedrock Converse API through Rig / `aws-sdk-bedrockruntime`, not native Anthropic Messages on `bedrock-mantle`. |
| Model refresh | Settings **Refresh from AWS** calls `ListFoundationModels` and keeps active text-capable on-demand models. |
| Minimum IAM | `bedrock:InvokeModel`, `bedrock:InvokeModelWithResponseStream`, `bedrock:ListFoundationModels`. |

### Bedrock with SSO

1. In Settings → Bedrock, set **AWS profile** to the profile name from `~/.aws/config` (e.g. `bedrock`).
2. Set **AWS region** to the region where your models are enabled (not necessarily `us-east-1`).
3. Run `aws sso login --profile <name>` before starting a run. SSO tokens expire (typically 8 to 12 hours); log in again when credentials fail.
4. On macOS, Dock-launched apps do not inherit `AWS_PROFILE` from `~/.zshrc`. Enter the profile in Settings, or launch OpenFlow from a terminal where it is set (`./scripts/start.sh`).
5. Select **Test AWS connection** (Settings → Bedrock) to confirm the app loads credentials through the same chain runs use.
6. Verify: `aws sts get-caller-identity --profile <name>` and `aws bedrock list-foundation-models --profile <name> --region <region>`.

### GUI apps and `HOME`

The AWS Rust SDK finds `~/.aws/config`, `~/.aws/credentials`, and the SSO token cache through `HOME`, which GUI launches often omit even when a terminal works. If `HOME` is missing, OpenFlow sets it from the OS user directory so shared AWS config files are discoverable in-process.

## Manual smoke

Run the live workflow smoke only when intentionally checking a real provider:

```bash
export AWS_REGION=us-east-1
export STEP_WORKFLOW_LIVE_AI=1
export STEP_WORKFLOW_LIVE_MODEL=anthropic.claude-sonnet-4-20250514-v1:0
cargo nextest run -p orchestration --test live_workflow --run-ignored ignored-only --no-capture
```

[Testing workflows](../contributing/testing-workflows.md) defines the live-AI checks and their limits.
