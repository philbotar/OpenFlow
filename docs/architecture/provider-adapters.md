# Provider adapters

`crates/providers` implements outbound LLM adapters for `engine::AiPort`. Orchestration resolves settings into provider configuration and calls `create_provider()`; it does not depend on concrete provider clients.

Transport goes through **Rig 0.39** (`rig_adapter/`). Pre-Rig modules `openai_compat.rs`, `anthropic.rs`, and `sse.rs` were removed.

## Supported adapter families

| Adapter family | Implementation | Role |
| --- | --- | --- |
| OpenAI-compatible (via Rig) | `rig_adapter/model.rs` + `convert.rs` | Chat completions / Responses wire shape, tools, streaming |
| ChatGPT (Codex) OAuth + Rig | `codex_oauth/`, `codex.rs`, and `rig_adapter/model.rs` | Browser/device login, refreshable credentials, ChatGPT Codex Responses streaming |
| Anthropic Messages (via Rig) | `rig_adapter/` (+ `anthropic_http.rs`, `claude_thinking.rs`) | Anthropic-native mapping, thinking blocks, prompt cache |
| Amazon Bedrock Converse (via Rig) | `rig_adapter/model.rs` + `aws_runtime.rs` | AWS Bedrock transport and credential resolution |
| Shared mapping | `crates/providers/src/mapping/` | Transcript conversion, tool argument parsing, `jsonrepair-rs` recovery |
| Factory | `crates/providers/src/lib.rs` | `create_provider()` returns `Box<dyn AiPort>` |
| Client entry | `crates/providers/src/client.rs` | `AiClient: AiPort`, config enums, model cache |

## Deterministic recovery and overseer repair

Providers own wire decoding and local `jsonrepair-rs` recovery. Semantic acceptance of `openflow_submit_node_output` lives in the engine completion protocol (`execution/completion_protocol.rs`).

When deterministic recovery cannot satisfy the node output schema, providers attach a redacted `OutputRepairCandidate` on `AgentError::MalformedSubmitOutput`. Orchestration wraps the run provider once in `RepairingAiPort` (before `AiInvocationAdapter`) so nodes and subagents share one overseer pass on the **same** provider.

| Setting | Meaning |
| --- | --- |
| `WorkflowSettings.output_repair_model` (`outputRepairModel`) | Optional overseer model override |
| Blank / absent | Inherit the failed worker request's model |
| Nonblank | Use that model string; credentials stay on the run provider |

See [`output-repair.md`](output-repair.md) for sequence, guards, and deferred scope.

## ChatGPT (Codex) OAuth

The **ChatGPT (Codex)** provider authenticates with a ChatGPT account instead of an OpenAI API key. OpenFlow owns login, token refresh, persistence, and one-shot unauthorized recovery; Rig's ChatGPT provider supplies the Codex Responses request and SSE mapping.

| Concern | Behavior |
| --- | --- |
| Browser login | PKCE S256 on `http://localhost:1455/auth/callback`, loopback-only listener, exact state validation |
| Port conflict | Falls back to ChatGPT's device-code flow only when port 1455 cannot be bound |
| Refresh | Refreshes within five minutes of expiry, serializes concurrent refreshes, persists rotated credentials, then retries one unauthorized request |
| Inference | SSE `POST /backend-api/codex/responses` through Rig with `originator: openflow` and `OpenAI-Beta: responses=experimental`; no WebSocket |
| Storage | Access, refresh, ID token, expiry, and account metadata are plaintext in `ProviderProfile.codex_oauth` inside `settings.json` |
| UI boundary | Settings IPC returns only tagged login state, safe email, and device instructions—never tokens or account ID |

OpenFlow rewrites the OAuth protocol in Rust. Endpoint, PKCE, token-body, and request-shape details derive from [oh-my-pi's MIT-licensed implementation](https://github.com/badlogic/pi-mono/tree/main/packages/ai/src/auth/oauth), then are pinned by fixture tests. OpenFlow intentionally differs from the Codex CLI by choosing device authorization, rather than a second callback port, when port 1455 is occupied.

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
3. In a terminal, run `aws sso login --profile <name>` before starting a run. SSO tokens expire (typically 8–12h); re-login when credentials fail.
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
cargo test -p orchestration --test live_workflow -- --ignored --nocapture
```

See [testing workflows](../contributing/testing-workflows.md) for the full live-AI rules.
