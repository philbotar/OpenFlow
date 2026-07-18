# Provider adapters

`crates/providers` implements outbound LLM adapters for `engine::AiPort`. Orchestration resolves settings into provider configuration and calls `create_provider()`; it does not depend on concrete provider clients.

Transport goes through **Rig 0.39** (`rig_adapter/`). Pre-Rig modules `openai_compat.rs`, `anthropic.rs`, and `sse.rs` were removed.

## Supported adapter families

| Adapter family | Implementation | Role |
| --- | --- | --- |
| OpenAI-compatible (via Rig) | `rig_adapter/model.rs` + `convert.rs` | Chat completions / Responses wire shape, tools, streaming |
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
