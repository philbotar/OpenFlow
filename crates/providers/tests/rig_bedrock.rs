#![cfg(feature = "bedrock")]
#![allow(
    clippy::multiple_crate_versions,
    clippy::panic,
    clippy::unwrap_used,
    reason = "integration tests use unwrap/panic; bedrock pulls duplicate transitive deps"
)]

use providers::{
    create_provider, AiClientConfig, AuthConfig, BedrockConfig, ProviderAdapterConfig, ProviderId,
};

fn bedrock_test_config() -> AiClientConfig {
    AiClientConfig {
        provider_id: ProviderId::from("bedrock"),
        provider_label: "Bedrock".into(),
        auth: AuthConfig::AwsCredentials {
            profile: Some("dev".into()),
            region: "eu-west-1".into(),
        },
        adapter: ProviderAdapterConfig::Bedrock(BedrockConfig {
            region: "eu-west-1".into(),
            aws_profile: Some("dev".into()),
        }),
    }
}

fn bedrock_request() -> engine::AgentRequest {
    engine::AgentRequest {
        workflow_id: engine::WorkflowId("wf-1".into()),
        node_id: engine::NodeId("idea".into()),
        node_label: "Idea".into(),
        model: "anthropic.claude-sonnet-4".into(),
        system_messages: vec!["sys".into()],
        task_prompt: "task".into(),
        input: serde_json::Value::Null,
        output_schema: serde_json::Value::Null,
        tool_config: engine::NodeToolConfig::default(),
        available_tools: Vec::new(),
        transcript: Vec::new(),
        model_attempt: 1,
        reasoning_effort: None,
        reasoning_budget_tokens: None,
    }
}

#[tokio::test]
async fn bedrock_provider_builds_and_invoke_fails_without_live_aws() {
    let client = create_provider(bedrock_test_config());
    let err = client.invoke(bedrock_request()).await.unwrap_err();
    assert!(!err.to_string().is_empty());
}
