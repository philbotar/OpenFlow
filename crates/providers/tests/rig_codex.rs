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
        provider_id: None,
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
        entrypoint_attachments: Vec::new(),
        resolved_attachments: std::collections::BTreeMap::default(),
        model_attempt: 1,
        reasoning_effort: Some("high".into()),
        reasoning_budget_tokens: None,
        tool_access_policy: engine::ToolAccessPolicy::Execution,
        allow_user_input: false,
        conversation_mode: false,
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
        debug_output: false,
    }
}

fn completed_submit_sse() -> String {
    let tool_call = json!({
        "type": "response.output_item.done",
        "sequence_number": 1,
        "output_index": 0,
        "item": {
            "type": "function_call",
            "id": "fc_1",
            "call_id": "call-submit",
            "name": "openflow_submit_node_output",
            "arguments": "{\"output\":{\"summary\":\"done\"},\"assistant_message\":null}",
            "status": "completed"
        }
    });
    let response = json!({
        "type": "response.completed",
        "sequence_number": 2,
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
    format!("data: {tool_call}\n\ndata: {response}\n\ndata: [DONE]\n\n")
}

fn completed_authoring_tool_sse() -> String {
    let tool_call = json!({
        "type": "response.output_item.done",
        "sequence_number": 1,
        "output_index": 0,
        "item": {
            "type": "function_call",
            "id": "fc_authoring_1",
            "call_id": "call_authoring_1",
            "name": "openflow_add_node",
            "arguments": "{\"id\":\"draft\",\"label\":\"Draft\"}",
            "status": "completed"
        }
    });
    let response = json!({
        "type": "response.completed",
        "sequence_number": 2,
        "response": {
            "id": "resp_tool_1",
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
                "id": "fc_authoring_1",
                "call_id": "call_authoring_1",
                "name": "openflow_add_node",
                "arguments": "{\"id\":\"draft\",\"label\":\"Draft\"}",
                "status": "completed"
            }],
            "tools": []
        }
    });
    format!("data: {tool_call}\n\ndata: {response}\n\ndata: [DONE]\n\n")
}

fn completed_submit_with_stream_only_tool_sse() -> String {
    let tool_call = json!({
        "type": "response.output_item.done",
        "sequence_number": 1,
        "output_index": 1,
        "item": {
            "type": "function_call",
            "id": "fc_stream_only",
            "call_id": "call-stream-only",
            "name": "openflow_submit_node_output",
            "arguments": "{\"output\":{\"summary\":\"recovered\"},\"assistant_message\":null}",
            "status": "completed"
        }
    });
    let response = json!({
        "type": "response.completed",
        "sequence_number": 2,
        "response": {
            "id": "resp_stream_only",
            "object": "response",
            "created_at": 1,
            "status": "completed",
            "error": null,
            "incomplete_details": null,
            "instructions": null,
            "max_output_tokens": null,
            "model": "gpt-5.6-luna",
            "usage": {
                "input_tokens": 5,
                "input_tokens_details": {"cached_tokens": 0},
                "output_tokens": 4,
                "output_tokens_details": {"reasoning_tokens": 4},
                "total_tokens": 9
            },
            "output": [{
                "type": "reasoning",
                "id": "rs_1",
                "summary": [],
                "encrypted_content": "opaque",
                "status": "completed"
            }],
            "tools": []
        }
    });
    format!("data: {tool_call}\n\ndata: {response}\n\ndata: [DONE]\n\n")
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
            ResponseTemplate::new(200).set_body_raw(completed_submit_sse(), "text/event-stream"),
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

#[tokio::test]
async fn codex_invoke_preserves_tool_call_from_sse_events_when_final_output_is_reasoning_only() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/responses"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            completed_submit_with_stream_only_tool_sse(),
            "text/event-stream",
        ))
        .mount(&server)
        .await;

    let outcome = create_provider(codex_config(&server.uri()))
        .invoke(test_request())
        .await
        .unwrap();
    let AgentTurnOutcome::Completed(success) = outcome else {
        panic!("expected completed outcome");
    };
    assert_eq!(success.output, json!({"summary": "recovered"}));
}

#[tokio::test]
async fn codex_replays_tool_call_and_result_with_responses_call_id() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/responses"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_raw(completed_authoring_tool_sse(), "text/event-stream"),
        )
        .expect(1)
        .mount(&server)
        .await;
    let provider = create_provider(codex_config(&server.uri()));
    let mut request = test_request();

    let first = provider.invoke(request.clone()).await.unwrap();
    let AgentTurnOutcome::ToolCalls(batch) = first else {
        panic!("expected authoring tool call");
    };
    let call = batch.tool_calls.into_iter().next().unwrap();
    request.transcript = vec![
        engine::AgentTranscriptItem::ToolCall { call: call.clone() },
        engine::AgentTranscriptItem::ToolResult {
            result: engine::ToolResult {
                tool_call_id: call.id.clone(),
                tool_name: call.name.clone(),
                content: "{\"ok\":true}".into(),
                is_error: false,
                artifact_ids: Vec::new(),
                output_meta: None,
            },
        },
    ];

    server.reset().await;
    Mock::given(method("POST"))
        .and(path("/responses"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(completed_submit_sse(), "text/event-stream"),
        )
        .expect(1)
        .mount(&server)
        .await;

    let second = provider.invoke(request).await.unwrap();
    assert!(matches!(second, AgentTurnOutcome::Completed(_)));

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    let body: serde_json::Value = serde_json::from_slice(&requests[0].body).unwrap();
    let input = body["input"].as_array().unwrap();
    let replayed_call = input
        .iter()
        .find(|item| item["type"] == "function_call")
        .unwrap();
    assert_eq!(replayed_call["id"], "fc_authoring_1");
    assert_eq!(replayed_call["call_id"], "call_authoring_1");
    let replayed_result = input
        .iter()
        .find(|item| item["type"] == "function_call_output")
        .unwrap();
    assert_eq!(replayed_result["call_id"], "call_authoring_1");
}
