# Providers

`crates/providers`

## What it does

Outbound LLM adapters implementing `engine::AiPort`: OpenAI-compatible HTTP, Anthropic Messages, and Amazon Bedrock Converse/ConverseStream.

## Amazon Bedrock

- **Auth:** AWS credential chain (`AWS_ACCESS_KEY_ID` / `AWS_SECRET_ACCESS_KEY`, SSO, shared config, instance role). Optional profile name in Settings (stored in the provider `api_key` field) or `AWS_PROFILE`.
- **Region:** Settings **AWS region** field (`ProviderProfile.base_url`, default `us-east-1`) or `AWS_REGION`.
- **Inference:** [Converse API](https://docs.aws.amazon.com/bedrock/latest/userguide/conversation-inference.html) via `aws-sdk-bedrockruntime` — not native Anthropic Messages on `bedrock-mantle`.
- **Models:** Settings **Refresh from AWS** calls [ListFoundationModels](https://docs.aws.amazon.com/bedrock/latest/APIReference/API_ListFoundationModels.html) (`ON_DEMAND`, TEXT output, ACTIVE lifecycle; embedding IDs filtered client-side).
- **IAM (minimum):** `bedrock:InvokeModel`, `bedrock:InvokeModelWithResponseStream`, `bedrock:ListFoundationModels`.

### Manual smoke

```bash
export AWS_REGION=us-east-1
export STEP_WORKFLOW_LIVE_AI=1
export STEP_WORKFLOW_LIVE_MODEL=anthropic.claude-sonnet-4-20250514-v1:0
cargo test -p orchestration --test live_workflow -- --ignored --nocapture
```

## Why it is structured this way

One `AiClient` dispatches on `ProviderAdapterConfig`; orchestration only calls `create_provider()` with resolved `AiClientConfig`.
