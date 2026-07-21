#![allow(
    clippy::panic,
    clippy::unwrap_used,
    reason = "integration tests use unwrap/panic for brevity"
)]

use providers::{
    create_provider, AiClientConfig, AnthropicConfig, AuthConfig, ProviderAdapterConfig, ProviderId,
};
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn test_request() -> engine::AgentRequest {
    engine::AgentRequest {
        workflow_id: engine::WorkflowId("wf-1".into()),
        node_id: engine::NodeId("idea".into()),
        node_label: "Idea".into(),
        model: "claude-3-5-sonnet-latest".into(),
        system_messages: vec!["You are precise.".into()],
        task_prompt: "Summarize the kickoff.".into(),
        input: serde_json::json!({"entrypoint": {"text": "ORCHID-91"}, "upstream": []}),
        output_schema: serde_json::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "summary": { "type": "string" }
            },
            "required": ["summary"]
        }),
        tool_config: engine::NodeToolConfig::default(),
        available_tools: Vec::new(),
        transcript: Vec::new(),
        model_attempt: 1,
        reasoning_effort: None,
        reasoning_budget_tokens: None,
        tool_access_policy: engine::ToolAccessPolicy::Execution,
        allow_user_input: true,
    }
}

fn anthropic_test_config(base_url: &str) -> AiClientConfig {
    AiClientConfig {
        provider_id: ProviderId::from("anthropic"),
        provider_label: "Anthropic".into(),
        auth: AuthConfig::Header {
            name: "x-api-key".into(),
            api_key: Some("test-key".into()),
            required: true,
        },
        adapter: ProviderAdapterConfig::Anthropic(AnthropicConfig {
            base_url: base_url.into(),
            messages_path: "v1/messages".into(),
            anthropic_version: "2023-06-01".into(),
            request_timeout: std::time::Duration::from_mins(5),
        }),
    }
}

#[tokio::test]
async fn anthropic_submit_output_completes_node() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .and(header("x-api-key", "test-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "msg_1",
            "type": "message",
            "role": "assistant",
            "model": "claude-3-5-sonnet-latest",
            "content": [{
                "type": "tool_use",
                "id": "toolu_1",
                "name": "openflow_submit_node_output",
                "input": {
                    "output": {"summary": "done"},
                    "assistant_message": null
                }
            }],
            "stop_reason": "tool_use",
            "usage": {
                "input_tokens": 101,
                "output_tokens": 19
            }
        })))
        .mount(&server)
        .await;

    let client = create_provider(anthropic_test_config(&server.uri()));
    let outcome = client.invoke(test_request()).await.unwrap();
    let engine::AgentTurnOutcome::Completed(success) = outcome else {
        panic!("expected completed outcome");
    };
    assert_eq!(success.output, serde_json::json!({"summary": "done"}));
}

#[tokio::test]
async fn anthropic_nullable_citations_do_not_break_tool_call() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .and(header("x-api-key", "test-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "msg_nullable_citations",
            "type": "message",
            "role": "assistant",
            "model": "minimax-m3",
            "content": [
                {
                    "type": "text",
                    "text": "Done.",
                    "citations": null
                },
                {
                    "type": "tool_use",
                    "id": "toolu_1",
                    "name": "openflow_submit_node_output",
                    "input": {
                        "output": {"summary": "done"},
                        "assistant_message": null
                    }
                }
            ],
            "stop_reason": "tool_use",
            "usage": {
                "input_tokens": 101,
                "output_tokens": 19
            }
        })))
        .mount(&server)
        .await;

    let client = create_provider(anthropic_test_config(&server.uri()));
    let outcome = client.invoke(test_request()).await.unwrap();
    let engine::AgentTurnOutcome::Completed(success) = outcome else {
        panic!("expected completed outcome");
    };
    assert_eq!(success.output, serde_json::json!({"summary": "done"}));
}

#[tokio::test]
async fn anthropic_null_content_uses_empty_turn_recovery() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "msg_null_content",
            "type": "message",
            "role": "assistant",
            "model": "minimax-m3",
            "content": null,
            "stop_reason": "end_turn",
            "usage": {
                "input_tokens": 101,
                "output_tokens": 0
            }
        })))
        .mount(&server)
        .await;

    let client = create_provider(anthropic_test_config(&server.uri()));
    let error = client.invoke(test_request()).await.unwrap_err();

    assert!(error.is_empty_provider_turn(), "unexpected error: {error}");
    assert!(!error.to_string().contains("response JSON error"));
}

#[tokio::test]
async fn anthropic_429_maps_to_transient() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(429).set_body_string("rate limited"))
        .mount(&server)
        .await;

    let client = create_provider(anthropic_test_config(&server.uri()));
    let err = client.invoke(test_request()).await.unwrap_err();
    assert!(err.is_retryable());
}

const ANTHROPIC_SSE_FIXTURE: &str = concat!(
    "data: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_stream_1\",\"type\":\"message\",\"role\":\"assistant\",\"model\":\"claude-3-5-sonnet-latest\",\"content\":[],\"stop_reason\":null,\"stop_sequence\":null,\"usage\":{\"input_tokens\":10,\"output_tokens\":0}}}\n\n",
    "data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n",
    "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"He\"}}\n\n",
    "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"llo\"}}\n\n",
    "data: {\"type\":\"content_block_stop\",\"index\":0}\n\n",
    "data: {\"type\":\"content_block_start\",\"index\":1,\"content_block\":{\"type\":\"tool_use\",\"id\":\"toolu_1\",\"name\":\"openflow_submit_node_output\",\"input\":{}}}\n\n",
    "data: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"output\\\": {\\\"summary\\\": \\\"done\\\"}, \\\"assistant_message\\\": null}\"}}\n\n",
    "data: {\"type\":\"content_block_stop\",\"index\":1}\n\n",
    "data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"tool_use\",\"stop_sequence\":null},\"usage\":{\"output_tokens\":19}}\n\n",
    "data: {\"type\":\"message_stop\"}\n\n",
);

#[derive(Clone, Default)]
struct RecordingSink(std::sync::Arc<std::sync::Mutex<Vec<engine::AiStreamEvent>>>);

impl RecordingSink {
    fn events(&self) -> Vec<engine::AiStreamEvent> {
        self.0.lock().unwrap().clone()
    }
}

impl engine::AiStreamSink for RecordingSink {
    fn on_stream_event(&self, event: engine::AiStreamEvent) {
        self.0.lock().unwrap().push(event);
    }
}

#[tokio::test]
async fn anthropic_stream_emits_deltas_and_completes() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .and(header("x-api-key", "test-key"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(ANTHROPIC_SSE_FIXTURE, "text/event-stream"),
        )
        .mount(&server)
        .await;

    let client = create_provider(anthropic_test_config(&server.uri()));
    let sink = RecordingSink::default();
    let outcome = client.invoke_stream(test_request(), &sink).await.unwrap();

    let events = sink.events();
    assert!(events.iter().any(|event| matches!(
        event,
        engine::AiStreamEvent::AssistantDelta { content } if content == "He"
    )));
    let engine::AgentTurnOutcome::Completed(success) = outcome else {
        panic!("expected completed outcome");
    };
    assert_eq!(success.output, serde_json::json!({"summary": "done"}));
}

#[tokio::test]
async fn anthropic_structurally_invalid_submit_input_is_typed_repair_candidate() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .and(header("x-api-key", "test-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "msg_bad_envelope",
            "type": "message",
            "role": "assistant",
            "model": "claude-3-5-sonnet-latest",
            "content": [{
                "type": "tool_use",
                "id": "toolu_bad",
                "name": "openflow_submit_node_output",
                "input": {
                    "path": ".flow/README.md",
                    "assistant_message": null
                }
            }],
            "stop_reason": "tool_use",
            "usage": {
                "input_tokens": 10,
                "output_tokens": 5
            }
        })))
        .mount(&server)
        .await;

    let client = create_provider(anthropic_test_config(&server.uri()));
    let err = client.invoke(test_request()).await.unwrap_err();
    assert!(err.is_malformed_submit_output());
    assert!(err.is_repairable_submit_output());
    let candidate = err.output_repair_candidate().unwrap();
    assert!(candidate.raw_arguments().contains(".flow/README.md"));
    assert!(!format!("{candidate:?}").contains(".flow/README.md"));
    assert!(!err.to_string().contains(".flow/README.md"));
}
