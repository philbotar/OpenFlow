#![allow(
    clippy::panic,
    clippy::unwrap_used,
    reason = "integration tests use unwrap/panic for brevity"
)]

use std::sync::{Arc, Mutex};

use engine::{
    AgentToolCallBatch, AgentTurnOutcome, AiStreamEvent, AiStreamSink, ToolCall, ToolDefinition,
};
use providers::{
    create_provider, AiClientConfig, AuthConfig, OpenAiCompatibleConfig, ProviderAdapterConfig,
    ProviderId, WireApi,
};
use serde_json::json;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn chat_completion_response(message: &serde_json::Value) -> serde_json::Value {
    json!({
        "id": "chatcmpl-1",
        "object": "chat.completion",
        "created": 0,
        "model": "test-model",
        "choices": [{
            "index": 0,
            "message": message,
            "finish_reason": "tool_calls"
        }]
    })
}

fn responses_submit_fixture() -> serde_json::Value {
    json!({
        "id": "resp_1",
        "object": "response",
        "created_at": 0,
        "status": "completed",
        "model": "test-model",
        "output": [{
            "type": "function_call",
            "id": "fc_1",
            "call_id": "call-1",
            "name": "openflow_submit_node_output",
            "arguments": "{\"output\":{\"summary\":\"done\"},\"assistant_message\":null}",
            "status": "completed"
        }]
    })
}

fn test_request() -> engine::AgentRequest {
    engine::AgentRequest {
        workflow_id: engine::WorkflowId("wf-1".into()),
        node_id: engine::NodeId("idea".into()),
        node_label: "Idea".into(),
        model: "test-model".into(),
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
    }
}

fn openai_test_config(base_url: &str, wire_api: WireApi) -> AiClientConfig {
    AiClientConfig {
        provider_id: ProviderId::from("openai"),
        provider_label: "OpenAI".into(),
        auth: AuthConfig::Bearer {
            api_key: Some("key".into()),
            required: true,
        },
        adapter: ProviderAdapterConfig::OpenAiCompatible(OpenAiCompatibleConfig {
            base_url: base_url.into(),
            wire_api,
            responses_path: "v1/responses".into(),
            chat_completions_path: "v1/chat/completions".into(),
        }),
    }
}

#[derive(Clone, Default)]
struct RecordingSink(Arc<Mutex<Vec<AiStreamEvent>>>);

impl RecordingSink {
    fn events(&self) -> Vec<AiStreamEvent> {
        self.0.lock().unwrap().clone()
    }
}

impl AiStreamSink for RecordingSink {
    fn on_stream_event(&self, event: AiStreamEvent) {
        self.0.lock().unwrap().push(event);
    }
}

#[tokio::test]
async fn responses_submit_output_completes_node() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .and(header("authorization", "Bearer key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(responses_submit_fixture()))
        .mount(&server)
        .await;

    let client = create_provider(openai_test_config(&server.uri(), WireApi::Responses));
    let outcome = client.invoke(test_request()).await.unwrap();
    let AgentTurnOutcome::Completed(success) = outcome else {
        panic!("expected completed outcome");
    };
    assert_eq!(success.output, json!({"summary": "done"}));
}

#[tokio::test]
async fn chat_completions_external_tool_call_batch() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(chat_completion_response(&json!({
                "role": "assistant",
                "content": "I need to inspect the README.",
                "tool_calls": [{
                    "id": "call-7",
                    "type": "function",
                    "function": {
                        "name": "read",
                        "arguments": "{\"path\":\"README.md\"}"
                    }
                }]
            }))),
        )
        .mount(&server)
        .await;

    let mut request = test_request();
    request.available_tools = vec![ToolDefinition {
        name: "read".into(),
        description: "Read a file or URL.".into(),
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": { "path": { "type": "string" } },
            "required": ["path"]
        }),
        tier: engine::ToolTier::Read,
        concurrency: engine::ToolConcurrency::Shared,
    }];

    let client = create_provider(openai_test_config(&server.uri(), WireApi::ChatCompletions));
    let outcome = client.invoke(request).await.unwrap();
    assert_eq!(
        outcome,
        AgentTurnOutcome::ToolCalls(AgentToolCallBatch {
            raw_text: "I need to inspect the README.".into(),
            assistant_message: Some("I need to inspect the README.".into()),
            tool_calls: vec![ToolCall {
                id: "call-7".into(),
                name: "read".into(),
                arguments: json!({"path": "README.md"}),
            }],
            usage: None,
        })
    );
}

#[tokio::test]
async fn chat_completions_429_maps_to_transient() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(429).set_body_string("rate limited"))
        .mount(&server)
        .await;

    let client = create_provider(openai_test_config(&server.uri(), WireApi::ChatCompletions));
    let err = client.invoke(test_request()).await.unwrap_err();
    assert!(err.is_retryable());
}

#[tokio::test]
async fn chat_completions_stream_emits_deltas() {
    let server = MockServer::start().await;
    let sse = concat!(
        "data: {\"choices\":[{\"delta\":{\"content\":\"Hi\",\"tool_calls\":[]}}]}\n\n",
        "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_1\",\"type\":\"function\",\"function\":{\"name\":\"openflow_submit_node_output\",\"arguments\":\"\"}}]}}]}\n\n",
        "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"{\\\"output\\\":{\\\"summary\\\":\\\"done\\\"},\\\"assistant_message\\\":null}\"}}]}}]}\n\n",
        "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"tool_calls\"}]}\n\n",
        "data: {\"choices\":[],\"usage\":{\"prompt_tokens\":4,\"completion_tokens\":2,\"total_tokens\":6}}\n\n",
        "data: [DONE]\n\n",
    );
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(sse, "text/event-stream"))
        .mount(&server)
        .await;

    let client = create_provider(openai_test_config(&server.uri(), WireApi::ChatCompletions));
    let sink = RecordingSink::default();
    let outcome = client.invoke_stream(test_request(), &sink).await.unwrap();
    let events = sink.events();
    assert!(events.iter().any(|event| matches!(
        event,
        AiStreamEvent::AssistantDelta { content } if content == "Hi"
    )));
    assert!(matches!(outcome, AgentTurnOutcome::Completed(_)));
}

#[tokio::test]
async fn custom_header_auth_reaches_wire() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .and(header("x-custom-key", "secret"))
        .respond_with(ResponseTemplate::new(200).set_body_json(chat_completion_response(&json!({
            "role": "assistant",
            "tool_calls": [{
                "id": "call-1",
                "type": "function",
                "function": {
                    "name": "openflow_submit_node_output",
                    "arguments": "{\"output\":{\"summary\":\"done\"},\"assistant_message\":null}"
                }
            }]
        }))))
        .mount(&server)
        .await;

    let config = AiClientConfig {
        provider_id: ProviderId::from("custom"),
        provider_label: "Custom".into(),
        auth: AuthConfig::Header {
            name: "x-custom-key".into(),
            api_key: Some("secret".into()),
            required: true,
        },
        adapter: ProviderAdapterConfig::OpenAiCompatible(OpenAiCompatibleConfig {
            base_url: server.uri(),
            wire_api: WireApi::ChatCompletions,
            responses_path: "v1/responses".into(),
            chat_completions_path: "v1/chat/completions".into(),
        }),
    };
    let client = create_provider(config);
    let outcome = client.invoke(test_request()).await.unwrap();
    assert!(matches!(outcome, AgentTurnOutcome::Completed(_)));
}

#[tokio::test]
async fn ollama_skips_prompt_cache_key() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(chat_completion_response(&json!({
            "role": "assistant",
            "tool_calls": [{
                "id": "call-1",
                "type": "function",
                "function": {
                    "name": "openflow_submit_node_output",
                    "arguments": "{\"output\":{\"summary\":\"done\"},\"assistant_message\":null}"
                }
            }]
        }))))
        .expect(1)
        .mount(&server)
        .await;

    let config = AiClientConfig {
        provider_id: ProviderId::from("ollama"),
        provider_label: "Ollama".into(),
        auth: AuthConfig::NoneAllowed,
        adapter: ProviderAdapterConfig::OpenAiCompatible(OpenAiCompatibleConfig {
            base_url: server.uri(),
            wire_api: WireApi::ChatCompletions,
            responses_path: "v1/responses".into(),
            chat_completions_path: "v1/chat/completions".into(),
        }),
    };
    let client = create_provider(config);
    client.invoke(test_request()).await.unwrap();
}
