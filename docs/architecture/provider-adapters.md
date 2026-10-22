# Provider adapters

`crates/providers` implements outbound LLM adapters for `engine::AiPort`. Orchestration resolves settings into provider configuration and calls `create_provider()`; it does not depend on concrete provider clients.

Transport goes through **Rig 0.39** in `rig_adapter/`. For keys, OAuth, and readiness controls, follow [Provider setup](../guides/provider-setup.md).

## Request routing

A model turn passes through the engine port, orchestration router, and provider adapter:

```text
Workflow run (engine)
  → AgentRequest (prompt, tools, provider id, model id)
    → AiPort (engine port)
      → orchestration routes to the effective provider
      → providers::create_provider(...) for each referenced provider
        → mapping/ (transcript + tool wire shape)
        → rig_adapter/ (Rig HTTP + streaming to the vendor API)
```

During a run, the **engine** only sees `AiPort`: one interface for model turns, streaming, and tool-call results. **Orchestration** loads `settings.json`, merges secrets, resolves every provider referenced by the workflow, and builds a run-scoped router over those clients. It validates all referenced provider credentials before execution. A provider failure stays attached to that node; OpenFlow does not retry the request through a different provider.

Inside **`crates/providers`**, **Rig** (`rig-core` 0.39) owns most outbound HTTP and SSE streaming to vendor APIs. OpenFlow code around Rig handles:

- `AgentRequest` ↔ provider payload conversion (`mapping/`, `rig_adapter/convert.rs`);
- API keys, Anthropic headers, and OpenAI-compatible base URLs (`auth.rs`, profile fields);
- ChatGPT Codex OAuth, refresh, and Codex-specific endpoints (`codex_oauth/`, `codex.rs`);
- Amazon Bedrock credentials and Converse calls when the desktop build enables the `bedrock` feature (`aws_runtime.rs`).

Orchestration does not import Rig or vendor SDKs directly; it only uses the factory and config types from `providers`. Run logic stays in the engine; provider transport stays in `providers`.

### Wire protocols

The `providers` crate uses `rig-core` for HTTP calls. Its `mapping/` code builds Rig `CompletionRequest` values from `AgentRequest`; workflow code does not call the OpenAI or Anthropic client SDKs directly.

`create_provider()` picks an adapter family from the active builtin profile ([built-in profiles](../reference/providers.md)):

| Profile family | `ProviderAdapterConfig` | Rig model (simplified) | HTTP shape |
| --- | --- | --- | --- |
| **Anthropic** | `Anthropic` | `rig_core::providers::anthropic` completion model | Anthropic **Messages** API (`/v1/messages`), `x-api-key` + Anthropic version headers |
| **OpenAI** and most hosted vendors | `OpenAiCompatible` | `rig_core::providers::openai` | Default **Responses** API for OpenAI; **Chat Completions** for OpenRouter, Groq, Gemini compat, locals, etc. |
| Per-model override on a compat profile | `OpenAiCompatible` + `model_transports` | Same Rig Anthropic client against the profile **base URL** | **Anthropic Messages** on a non-Anthropic host (gateway or proxy) |
| **ChatGPT (Codex)** | `OpenAiCodex` (separate `CodexClient`) | Rig ChatGPT responses + OpenFlow OAuth | Private Codex backend (not `api.openai.com` API-key traffic) |
| **Amazon Bedrock** | `Bedrock` | `rig-bedrock` completion model | AWS Bedrock **Converse** (not Anthropic Messages on `api.anthropic.com`) |

At invoke time, `rig_adapter` builds one `RigModel` enum arm (Anthropic, OpenAiChat, OpenAiResponses, ChatGPT, or Bedrock), runs the turn, then maps the result back to `AgentTurnOutcome`. The engine always sees the same `AiPort` regardless of which arm ran.

Selecting **Anthropic** or **OpenAI** in Settings changes the wire protocol and authentication without changing workflow JSON. **OpenAI** and **Groq** share the OpenAI-compatible adapter type, with different base URLs, key environment variables, and default Responses or Chat Completions transport in `spec.rs`.

UI control for transport: **Settings → Providers** → per-model **Responses API**, **Chat Completions API**, or **Anthropic Messages API** on editable or custom profiles. Managed OpenAI defaults to Responses; most other compat profiles default to Chat Completions.

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

Buffered and streaming `AiClient` calls share dispatch in `rig_adapter/mod.rs`; model construction
selects the provider transport. The per-client cache reuses models and rebuilds Bedrock models before credential expiry.
Each invocation selects the model and builds its payload from the current `AgentRequest`, including
its transcript and reasoning/fast-mode settings. Codex retains its separate refreshable client and
shares the underlying Rig model invocation methods.

Inference and HTTP model discovery share key/header validation in `auth.rs`. Discovery retains its
shorter total timeout. Tool schemas use Rig's tool definition directly inside providers; engine
contracts remain independent of Rig.

## Provider prompt caching

Prompt caching reuses exact LLM prompt prefixes. It is independent from OpenFlow's validated
tool-result cache.

| Provider path | OpenFlow behavior |
| --- | --- |
| OpenAI API, Responses | Sends a stable `prompt_cache_key` per workflow node. GPT-5.6 models also use `prompt_cache_options.mode: "explicit"` and a breakpoint on the final stable system block, so changing user/tool history does not trigger a billable cache write. |
| OpenAI API, Chat Completions | Sends the same stable per-node `prompt_cache_key`; the provider selects the automatic breakpoint. |
| Anthropic Messages | Combines automatic moving-history caching with explicit stable system and tool-definition markers. Uses Anthropic's default five-minute TTL. |
| Anthropic-compatible model override | Uses the same Messages caching policy. Backend support remains part of that endpoint's compatibility contract. |
| Amazon Bedrock Converse | Enables Rig prompt caching only for the Claude and Nova model-ID allowlist in `rig_adapter/model.rs`; unsupported models receive no cache points. |
| Other OpenAI-compatible endpoints | Sends `prompt_cache_key` except for Ollama and LM Studio. Cache acceptance, discounts, and lifetime remain provider-defined. |
| ChatGPT (Codex) OAuth | Sends no OpenFlow-managed cache controls because the private backend contract does not document them. |

The provider response maps cache reads to `UsageReport.cached_input_tokens` and cache writes to
`UsageReport.cache_creation_input_tokens`. Orchestration projects the latest values into each
node's `ContextWindowSnapshot` as `cachedInputTokens` and `cacheCreationInputTokens`. OpenAI
Responses requires a final-wire compatibility shim because Rig 0.39 does not yet model
`prompt_cache_key`, `prompt_cache_options`, or `cache_write_tokens`.

Caching starts only after the provider's minimum prompt size. Cache keys improve routing; they do
not memoize responses or change model output semantics. Keep stable system context and tool
definitions first. Keep run-specific user input and tool results later.

## Per-model transport routing

Custom OpenAI-compatible profiles keep a default Responses or Chat Completions transport. A model
can override that default through `ProviderProfile.model_transports` with `responses`,
`chat_completions`, or `anthropic_messages`. Settings stores model IDs as data; provider code does
not infer transport from vendor names or model allowlists.

An `anthropic_messages` override uses the Rig Anthropic client against the profile base URL. It
sends the standard `/v1/messages` request, Anthropic auth/version headers, tool schema, and
Anthropic response mapping. Removing the override restores the profile default.

OpenAI Responses and ChatGPT (Codex) preserve MCP schemas with map-like object parameters. Rig
0.39 marks every Responses function tool as strict, but OpenAI strict mode rejects objects that
allow additional properties. The final-wire adapters set `strict: false` only on affected tools;
they do not close or otherwise rewrite the MCP server's schema. Closed schemas remain strict.

When an adapter must invoke without streaming, it keeps typed reasoning on the turn outcome.
The fallback stream emits displayable text and summary blocks before assistant text. It never
emits encrypted or redacted reasoning payloads.

## Fast mode

Fast mode is an independent request setting. It does not select or rewrite reasoning effort.
OpenAI API and ChatGPT (Codex) requests map the saved `fastMode` boolean to
`service_tier: "priority"`. The ChatGPT UI name **Fast** and the Codex config value `fast` are not
wire-level service tier values. Other providers do not expose this control.

Rig's ChatGPT adapter clears `service_tier` during request construction. `CodexHttpClient` restores
the user-selected priority tier at the final HTTP boundary while preserving Rig's request and
stream mapping.

## Stream cutoff recovery

Rig streams tool-call names and argument deltas before the final response. OpenFlow accumulates
those deltas by Rig's internal call ID. If a Responses stream ends with
`response.incomplete: max_output_tokens`, the provider returns
`AgentError::OutputTruncated` with redacted partial-call metadata instead of dropping the in-flight
call or reporting a generic provider failure. Raw partial arguments stay in memory and are omitted
from `Debug`, `Display`, telemetry, and chat projection.

The engine executes no call from the incomplete response. It retries the node up to three times
with instructions to create a small valid skeleton, then expand it through focused edits. Claude 4
models recognized by Rig keep Rig's larger model-specific output limit; other Anthropic-compatible
models receive a 16K minimum fallback. Successful edit-tool writes use a sibling temporary file plus
atomic rename, so cancellation cannot expose a partially written destination.

## Deterministic recovery and overseer repair

Providers own wire decoding and local `jsonrepair-rs` recovery. Semantic acceptance of `openflow_submit_node_output` lives in the engine completion protocol (`execution/completion_protocol.rs`).

When deterministic recovery cannot satisfy the node output schema, providers attach a redacted `OutputRepairCandidate` on `AgentError::MalformedSubmitOutput`. Orchestration wraps the run provider once in `RepairingAiPort` (before `AiInvocationAdapter`) so nodes and subagents share one overseer pass on the **same** provider.

Malformed wire-argument markers retain at most 64 KiB of raw UTF-8 text, ending on a character
boundary. The engine independently caps the repair candidate.

| Setting | Meaning |
| --- | --- |
| `WorkflowSettings.output_repair_model` (`outputRepairModel`) | Optional overseer model override |
| Blank / absent | Inherit the failed worker request's model |
| Nonblank | Use that model string; credentials stay on the run provider |

[Overseer output repair](output-repair.md) describes the sequence, guards, and deferred scope.

## ChatGPT (Codex) OAuth

The **ChatGPT (Codex)** provider authenticates with a ChatGPT account instead of an OpenAI API key. OpenFlow owns login, token refresh, persistence, and one-shot unauthorized recovery; Rig's ChatGPT provider supplies the Codex Responses request and SSE mapping.

| Concern | Behavior |
| --- | --- |
| Browser login | PKCE S256 on `http://localhost:1455/auth/callback`, loopback-only listener, exact state validation |
| Port conflict | Falls back to ChatGPT's device-code flow only when port 1455 cannot be bound |
| Refresh | Refreshes within five minutes of expiry, serializes concurrent refreshes, persists rotated credentials, then retries one unauthorized request |
| Cancellation | Drops the entire pending login attempt, including stalled callback reads, device requests, and token exchange; releases the callback listener |
| Inference | SSE `POST /backend-api/codex/responses` through Rig with `originator: openflow` and `OpenAI-Beta: responses=experimental`; no WebSocket |
| Storage | Access, refresh, ID token, expiry, and account metadata are plaintext in `ProviderProfile.codex_oauth` inside `settings.json` |
| UI boundary | Settings IPC returns only tagged login state, safe email, and device instructions; it never returns tokens or account ID |

OpenFlow implements the OAuth protocol in Rust. Endpoint, PKCE, token-body, and request-shape details derive from [oh-my-pi's MIT-licensed implementation](https://github.com/badlogic/pi-mono/tree/main/packages/ai/src/auth/oauth), then are pinned by fixture tests. OpenFlow intentionally differs from the Codex CLI by choosing device authorization, rather than a second callback port, when port 1455 is occupied.

### Compatibility boundary

`chatgpt.com/backend-api/codex` and the reused Codex OAuth client ID are private, unsupported integration contracts. They can change independently of the public OpenAI API, and ChatGPT workspace policy may reject third-party clients or `originator: openflow`. Fixture tests prove OpenFlow's current request contract; only an interactive account smoke can prove entitlement and live backend acceptance.

Do not impersonate a first-party originator. If the backend rejects OpenFlow, surface the error and update the documented adapter contract after verifying current official Codex behavior.

## Amazon Bedrock

Bedrock uses AWS credentials and region settings rather than a normal API-key header.

| Concern | Source |
| --- | --- |
| Auth | AWS credential chain (`AWS_ACCESS_KEY_ID` / `AWS_SECRET_ACCESS_KEY`, SSO, shared config, instance role). Optional profile name in Settings (`ProviderProfile.aws_profile`), or `AWS_PROFILE` env var. |
| Region | Settings **AWS region** field (`ProviderProfile.aws_region`, default `us-east-1`) or `AWS_REGION`. Older settings with a Bedrock region in `ProviderProfile.base_url` are migrated on load. |
| Inference | AWS Bedrock Converse API through Rig / `aws-sdk-bedrockruntime`, not native Anthropic Messages on `bedrock-mantle`. |
| Model refresh | Settings **Refresh from AWS** calls `ListFoundationModels` and filters active text-capable on-demand models. |
| Minimum IAM | `bedrock:InvokeModel`, `bedrock:InvokeModelWithResponseStream`, and `bedrock:ListFoundationModels`. |

### Bedrock with SSO

1. In Settings → Bedrock, set **AWS profile** to the profile name from `~/.aws/config` (e.g. `bedrock`).
2. Set **AWS region** to the region where your models are enabled (not necessarily `us-east-1`).
3. In a terminal, run `aws sso login --profile <name>` before starting a run. SSO tokens expire (typically 8 to 12 hours); re-login when credentials fail.
4. On macOS, apps launched from the Dock do not inherit `AWS_PROFILE` from `~/.zshrc`. Either enter the profile in Settings or launch OpenFlow from a terminal (`./scripts/start.sh`) where the env var is set.
5. Use **Test AWS connection** in Settings to confirm the app can load credentials (Settings → Bedrock). This checks the same credential chain used at run time.
6. Verify credentials: `aws sts get-caller-identity --profile <name>` and `aws bedrock list-foundation-models --profile <name> --region <region>`.

### GUI apps and `HOME`

The AWS Rust SDK resolves `~/.aws/config`, `~/.aws/credentials`, and SSO token cache via the `HOME` environment variable. GUI launches often omit `HOME` even when a terminal session works. OpenFlow sets `HOME` from the OS user directory when it is missing so shared AWS config files are discoverable in-process.

## Manual smoke

Run the live workflow smoke only when intentionally checking a real provider:

```bash
export AWS_REGION=us-east-1
export STEP_WORKFLOW_LIVE_AI=1
export STEP_WORKFLOW_LIVE_MODEL=anthropic.claude-sonnet-4-20250514-v1:0
cargo nextest run -p orchestration --test live_workflow --run-ignored ignored-only --no-capture
```

[Testing workflows](../contributing/testing-workflows.md) defines the live-AI checks and their limits.
