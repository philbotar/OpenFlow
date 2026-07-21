#![allow(
    clippy::panic,
    clippy::unwrap_used,
    reason = "integration tests use unwrap/panic for brevity"
)]

use engine::AgentTurnOutcome;
use providers::{
    create_provider, AiClientConfig, AuthConfig, CodexOAuthCredentials, OpenAiCodexConfig,
    ProviderAdapterConfig, ProviderId,
};
use serde_json::json;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn test_request() -> engine::AgentRequest {
    engine::AgentRequest {
        workflow_id: engine::WorkflowId("wf-1".into()),
        node_id: engine::NodeId("idea".into()),
        node_label: "Idea".into(),
        model: "gpt-5.3-codex".into(),
        system_messages: vec!["You are precise.".into()],
        task_prompt: "Summarize the kickoff.".into(),
        input: json!({"entrypoint": {"text": "ORCHID-91"}, "upstream": []}),
        output_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {"summary": {"type": "string"}},
            "required": ["summary"]
        }),
        tool_config: engine::NodeToolConfig::default(),
        available_tools: Vec::new(),
        transcript: Vec::new(),
        model_attempt: 1,
        reasoning_effort: Some("high".into()),
        reasoning_budget_tokens: None,
        tool_access_policy: engine::ToolAccessPolicy::Execution,
        allow_user_input: false,
    }
}

fn codex_config(base_url: &str) -> AiClientConfig {
    AiClientConfig {
        provider_id: ProviderId::from("openai-codex"),
        provider_label: "OpenAI Codex".into(),
        auth: AuthConfig::NoneAllowed,
        adapter: ProviderAdapterConfig::OpenAiCodex(OpenAiCodexConfig {
            base_url: base_url.into(),
            request_timeout: std::time::Duration::from_mins(5),
            credentials: CodexOAuthCredentials {
                access_token: "access-token".into(),
                refresh_token: "refresh-token".into(),
                id_token: Some("id-token".into()),
                expires_at: 4_000_000_000,
                account_id: "account-123".into(),
                email: Some("person@example.com".into()),
            },
            credential_sink: None,
        }),
    }
}

fn completed_submit_sse() -> String {
    let response = json!({
        "type": "response.completed",
        "response": {
            "id": "resp_1",
            "object": "response",
            "created_at": 1,
            "status": "completed",
            "error": null,
            "incomplete_details": null,
            "instructions": null,
            "max_output_tokens": null,
            "model": "gpt-5.3-codex",
            "usage": {
                "input_tokens": 5,
                "input_tokens_details": {"cached_tokens": 0},
                "output_tokens": 4,
                "output_tokens_details": {"reasoning_tokens": 1},
                "total_tokens": 9
            },
            "output": [{
                "type": "function_call",
                "id": "fc_1",
                "call_id": "call-submit",
                "name": "openflow_submit_node_output",
                "arguments": "{\"output\":{\"summary\":\"done\"},\"assistant_message\":null}",
                "status": "completed"
            }],
            "tools": []
        }
    });
    format!("data: {response}\n\ndata: [DONE]\n\n")
}

#[tokio::test]
async fn codex_uses_subscription_responses_wire_contract() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/responses"))
        .and(header("authorization", "Bearer access-token"))
        .and(header("chatgpt-account-id", "account-123"))
        .and(header("openai-beta", "responses=experimental"))
        .and(header("originator", "openflow"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_string(completed_submit_sse()),
        )
        .mount(&server)
        .await;

    let outcome = create_provider(codex_config(&server.uri()))
        .invoke(test_request())
        .await
        .unwrap();
    let AgentTurnOutcome::Completed(success) = outcome else {
        panic!("expected completed outcome");
    };
    assert_eq!(success.output, json!({"summary": "done"}));
    assert_eq!(success.usage.unwrap().total_tokens, 9);

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    let request = &requests[0];
    let body: serde_json::Value = serde_json::from_slice(&request.body).unwrap();
    assert_eq!(body["model"], "gpt-5.3-codex");
    assert_eq!(body["stream"], true);
    assert_eq!(body["store"], false);
    assert!(body.get("max_output_tokens").is_none());
    assert!(body.get("max_completion_tokens").is_none());
    assert_eq!(
        body["reasoning"],
        json!({"effort": "high", "summary": "auto"})
    );
    assert_eq!(body["include"], json!(["reasoning.encrypted_content"]));
    assert_eq!(
        request
            .headers
            .get("openai-beta")
            .and_then(|value| value.to_str().ok()),
        Some("responses=experimental")
    );
}
