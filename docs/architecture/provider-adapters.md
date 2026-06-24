# Provider adapters

`crates/providers` implements outbound LLM adapters for `engine::AiPort`. Orchestration resolves settings into provider configuration and calls `create_provider()`; it does not depend on concrete provider clients.

## Supported adapter families

| Adapter family | Implementation | Role |
| --- | --- | --- |
| OpenAI-compatible HTTP | `crates/providers/src/openai_compat.rs` | Chat completions-compatible transport, tool mapping, and streaming. |
| Anthropic Messages | `crates/providers/src/anthropic.rs` | Anthropic-native request and response mapping. |
| Amazon Bedrock Converse | `crates/providers/src/bedrock.rs` | AWS Bedrock `Converse` and `ConverseStream` transport. |
| Shared mapping | `crates/providers/src/mapping.rs` | Transcript conversion, tool argument parsing, and `jsonrepair-rs` recovery. |
| Factory | `crates/providers/src/lib.rs` | `create_provider()` returns `Box<dyn AiPort>`. |

## Amazon Bedrock

Bedrock uses AWS credentials and region settings rather than a normal API-key header.

| Concern | Source |
| --- | --- |
| Auth | AWS credential chain (`AWS_ACCESS_KEY_ID` / `AWS_SECRET_ACCESS_KEY`, SSO, shared config, instance role). Optional profile name in Settings is stored in the provider `api_key` field, or use `AWS_PROFILE`. |
| Region | Settings **AWS region** field (`ProviderProfile.base_url`, default `us-east-1`) or `AWS_REGION`. |
| Inference | AWS Bedrock Converse API through `aws-sdk-bedrockruntime`, not native Anthropic Messages on `bedrock-mantle`. |
| Model refresh | Settings **Refresh from AWS** calls `ListFoundationModels` and filters active text-capable on-demand models. |
| Minimum IAM | `bedrock:InvokeModel`, `bedrock:InvokeModelWithResponseStream`, and `bedrock:ListFoundationModels`. |

## Manual smoke

Run the live workflow smoke only when intentionally checking a real provider:

```bash
export AWS_REGION=us-east-1
export STEP_WORKFLOW_LIVE_AI=1
export STEP_WORKFLOW_LIVE_MODEL=anthropic.claude-sonnet-4-20250514-v1:0
cargo test -p orchestration --test live_workflow -- --ignored --nocapture
```

See [testing workflows](../contributing/testing-workflows.md) for the full live-AI rules.
