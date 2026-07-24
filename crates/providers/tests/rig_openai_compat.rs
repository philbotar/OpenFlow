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
        tool_access_policy: engine::ToolAccessPolicy::Execution,
        allow_user_input: true,
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
            model_transports: std::collections::BTreeMap::default(),
            request_timeout: std::time::Duration::from_mins(5),
        }),
        debug_output: false,
    }
}

fn custom_openai_test_config(base_url: &str) -> AiClientConfig {
    let mut config = openai_test_config(base_url, WireApi::ChatCompletions);
    config.provider_id = ProviderId::from("custom_openai_compatible");
    config.provider_label = "Custom OpenAI-compatible API".into();
    config
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
async fn custom_chat_submits_large_file_backed_output_without_document_duplication() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(chat_completion_response(&json!({
                "role": "assistant",
                "tool_calls": [{
                    "id": "call-submit",
                    "type": "function",
                    "function": {
                        "name": "openflow_submit_node_output",
                        "arguments": "{\"output\":{\"implementation_spec_markdown\":\"artifacts/0005-implementation-spec.md\",\"status\":\"complete\"},\"assistant_message\":null}"
                    }
                }]
            }))),
        )
        .mount(&server)
        .await;

    let mut request = test_request();
    request.output_schema = json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "implementation_spec_markdown": { "type": "string" },
            "status": { "type": "string" }
        },
        "required": ["implementation_spec_markdown", "status"]
    });
    request.available_tools = vec![ToolDefinition {
        name: "write".into(),
        description: "Write a file".into(),
        input_schema: json!({ "type": "object" }),
        tier: engine::ToolTier::Write,
        concurrency: engine::ToolConcurrency::Exclusive,
    }];
    request.transcript = vec![
        engine::AgentTranscriptItem::ToolCall {
            call: ToolCall {
                id: "write-large-spec".into(),
                name: "write".into(),
                arguments: json!({
                    "path": "artifacts/0005-implementation-spec.md",
                    "content": "<large document omitted from this fixture>"
                }),
            },
        },
        engine::AgentTranscriptItem::ToolResult {
            result: engine::ToolResult {
                tool_call_id: "write-large-spec".into(),
                tool_name: "write".into(),
                content: "wrote artifacts/0005-implementation-spec.md".into(),
                is_error: false,
                artifact_ids: Vec::new(),
                output_meta: None,
            },
        },
    ];

    let client = create_provider(custom_openai_test_config(&server.uri()));
    let outcome = client
        .invoke_stream(request, &RecordingSink::default())
        .await
        .unwrap();
    let AgentTurnOutcome::Completed(success) = outcome else {
        panic!("expected completed outcome");
    };
    assert_eq!(
        success.output["implementation_spec_markdown"],
        "artifacts/0005-implementation-spec.md"
    );

    let requests = server.received_requests().await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&requests[0].body).unwrap();
    assert_eq!(body["tool_choice"], "required");
    let control_tool_names = body["tools"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|tool| tool["function"]["name"].as_str())
        .collect::<Vec<_>>();
    assert!(control_tool_names.contains(&"openflow_submit_node_output"));
    assert!(
        control_tool_names.contains(&"write"),
        "unified catalog advertises executable tools alongside harness tools"
    );
    let submit_tool = body["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|tool| tool["function"]["name"] == "openflow_submit_node_output")
        .unwrap();
    assert!(
        submit_tool["function"]["parameters"]["properties"]["output"]["properties"]
            ["implementation_spec_markdown"]["description"]
            .as_str()
            .is_some_and(|description| description.contains("repository-relative file path"))
    );
}

#[tokio::test]
async fn custom_chat_honors_configured_request_timeout() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_delay(std::time::Duration::from_millis(100))
                .set_body_json(chat_completion_response(&json!({
                    "role": "assistant",
                    "content": "late"
                }))),
        )
        .mount(&server)
        .await;
    let mut config = custom_openai_test_config(&server.uri());
    let ProviderAdapterConfig::OpenAiCompatible(openai) = &mut config.adapter else {
        panic!("expected OpenAI-compatible config");
    };
    openai.request_timeout = std::time::Duration::from_millis(10);

    let error = create_provider(config)
        .invoke(test_request())
        .await
        .unwrap_err();

    assert!(error.is_retryable());
    assert!(error.to_string().contains("HTTP transport timed out"));
}

#[tokio::test]
async fn custom_chat_empty_turn_preserves_safe_response_diagnostics() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "chatcmpl-empty",
            "object": "chat.completion",
            "created": 0,
            "model": "test-model",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": null,
                    "reasoning_content": "sensitive private reasoning"
                },
                "finish_reason": "length"
            }],
            "usage": {
                "prompt_tokens": 7000,
                "completion_tokens": 4096,
                "total_tokens": 11096
            }
        })))
        .mount(&server)
        .await;

    let client = create_provider(custom_openai_test_config(&server.uri()));
    let error = client
        .invoke_stream(test_request(), &RecordingSink::default())
        .await
        .unwrap_err()
        .to_string();

    assert!(error.contains("finish_reason=length"), "{error}");
    assert!(error.contains("content=reasoning"), "{error}");
    assert!(
        error.contains("prompt=7000, completion=4096, total=11096"),
        "{error}"
    );
    assert!(!error.contains("sensitive private reasoning"), "{error}");
}

#[tokio::test]
async fn custom_chat_non_streaming_fallback_emits_reasoning() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(chat_completion_response(&json!({
            "role": "assistant",
            "content": null,
            "reasoning_content": "Inspecting the requested output.",
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

    let sink = RecordingSink::default();
    let outcome = create_provider(custom_openai_test_config(&server.uri()))
        .invoke_stream(test_request(), &sink)
        .await
        .unwrap();

    assert!(matches!(outcome, AgentTurnOutcome::Completed(_)));
    assert!(sink.events().iter().any(|event| matches!(
        event,
        AiStreamEvent::ThinkingDelta { content }
            if content == "Inspecting the requested output."
    )));
}

#[tokio::test]
async fn custom_chat_fully_empty_choice_is_empty_provider_turn() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "chatcmpl-blank",
            "object": "chat.completion",
            "created": 0,
            "model": "test-model",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": null
                },
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 0,
                "total_tokens": 10
            }
        })))
        .mount(&server)
        .await;

    let client = create_provider(custom_openai_test_config(&server.uri()));
    let error = client
        .invoke_stream(test_request(), &RecordingSink::default())
        .await
        .unwrap_err();

    assert!(
        error.is_empty_provider_turn(),
        "Rig empty choice must classify as empty-turn for engine retries: {error}"
    );
    let message = error.to_string();
    assert!(
        message.contains("no tool calls and no usable text"),
        "expected enriched empty-turn message, got {message}"
    );
    assert!(
        !message.contains("no message or tool call"),
        "raw Rig phrase must not leak: {message}"
    );
}

#[tokio::test]
async fn debug_output_logs_raw_chat_completion_body() {
    let marker = format!("openflow-debug-marker-{}", uuid_ish());
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "chatcmpl-debug",
            "object": "chat.completion",
            "created": 0,
            "model": "test-model",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": null,
                    "debug_marker": marker
                },
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 1,
                "completion_tokens": 0,
                "total_tokens": 1
            }
        })))
        .mount(&server)
        .await;

    let mut config = custom_openai_test_config(&server.uri());
    config.debug_output = true;
    let path = std::env::temp_dir().join(format!("openflow-debug-{}.jsonl", std::process::id()));
    let before = std::fs::read_to_string(&path).unwrap_or_default();

    let _ = create_provider(config)
        .invoke_stream(test_request(), &RecordingSink::default())
        .await;

    let after = std::fs::read_to_string(&path).unwrap_or_default();
    let new_text = after.strip_prefix(&before).unwrap_or(after.as_str());
    assert!(
        new_text.contains("model-response") && new_text.contains(&marker),
        "expected model-response line with marker; got:\n{new_text}"
    );
}

fn uuid_ish() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or_else(|_| "0".into(), |duration| duration.as_nanos().to_string())
}

#[tokio::test]
async fn debug_output_disabled_skips_model_response_log() {
    let marker = format!("openflow-debug-absent-{}", uuid_ish());
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "chatcmpl-debug-off",
            "object": "chat.completion",
            "created": 0,
            "model": "test-model",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": null,
                    "debug_marker": marker
                },
                "finish_reason": "stop"
            }]
        })))
        .mount(&server)
        .await;

    let config = custom_openai_test_config(&server.uri());
    assert!(!config.debug_output);
    let path = std::env::temp_dir().join(format!("openflow-debug-{}.jsonl", std::process::id()));
    let before = std::fs::read_to_string(&path).unwrap_or_default();

    let _ = create_provider(config)
        .invoke_stream(test_request(), &RecordingSink::default())
        .await;

    let after = std::fs::read_to_string(&path).unwrap_or_default();
    let new_text = after.strip_prefix(&before).unwrap_or(after.as_str());
    assert!(
        !new_text.contains(&marker),
        "disabled debug_output must not log body; got:\n{new_text}"
    );
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
            reasoning: vec![],
            usage: None,
        })
    );

    let requests = server.received_requests().await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&requests[0].body).unwrap();
    let tool_names = body["tools"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|tool| tool["function"]["name"].as_str())
        .collect::<Vec<_>>();
    assert!(tool_names.contains(&"openflow_submit_node_output"));
    assert!(tool_names.contains(&"read"));
}

#[tokio::test]
async fn multi_tool_call_history_reaches_wire_as_single_assistant_message() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(chat_completion_response(&json!({
                "role": "assistant",
                "content": "ok"
            }))),
        )
        .mount(&server)
        .await;

    let mut request = test_request();
    request.transcript = vec![
        engine::AgentTranscriptItem::ToolCall {
            call: ToolCall {
                id: "c1".into(),
                name: "read".into(),
                arguments: json!({}),
            },
        },
        engine::AgentTranscriptItem::ToolCall {
            call: ToolCall {
                id: "c2".into(),
                name: "read".into(),
                arguments: json!({}),
            },
        },
        engine::AgentTranscriptItem::ToolResult {
            result: engine::ToolResult {
                tool_call_id: "c2".into(),
                tool_name: "read".into(),
                content: "two".into(),
                is_error: false,
                artifact_ids: Vec::new(),
                output_meta: None,
            },
        },
        engine::AgentTranscriptItem::ToolResult {
            result: engine::ToolResult {
                tool_call_id: "c1".into(),
                tool_name: "read".into(),
                content: "one".into(),
                is_error: false,
                artifact_ids: Vec::new(),
                output_meta: None,
            },
        },
    ];

    let client = create_provider(openai_test_config(&server.uri(), WireApi::ChatCompletions));
    let _ = client.invoke(request).await;

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    let body: serde_json::Value = serde_json::from_slice(&requests[0].body).unwrap();
    let messages = body["messages"].as_array().unwrap();
    let assistant_idx = messages
        .iter()
        .position(|m| m["role"] == "assistant" && m["tool_calls"].is_array())
        .unwrap();
    let tool_calls = messages[assistant_idx]["tool_calls"].as_array().unwrap();
    assert_eq!(
        tool_calls.len(),
        2,
        "both calls must be in ONE assistant message"
    );
    assert_eq!(tool_calls[0]["id"], "c1");
    assert_eq!(tool_calls[1]["id"], "c2");
    // Results must immediately follow the assistant message, in call order.
    assert_eq!(messages[assistant_idx + 1]["role"], "tool");
    assert_eq!(messages[assistant_idx + 1]["tool_call_id"], "c1");
    assert_eq!(messages[assistant_idx + 2]["role"], "tool");
    assert_eq!(messages[assistant_idx + 2]["tool_call_id"], "c2");
    // No stray assistant tool-call messages besides the coalesced one.
    let assistant_tool_msgs = messages
        .iter()
        .filter(|m| m["role"] == "assistant" && m["tool_calls"].is_array())
        .count();
    assert_eq!(assistant_tool_msgs, 1);
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
            model_transports: std::collections::BTreeMap::default(),
            request_timeout: std::time::Duration::from_mins(5),
        }),
        debug_output: false,
    };
    let client = create_provider(config);
    let outcome = client.invoke(test_request()).await.unwrap();
    assert!(matches!(outcome, AgentTurnOutcome::Completed(_)));
}

#[tokio::test]
async fn chat_completions_upstream_403_maps_to_transient() {
    let body = r#"{"error":{"message":"Error from provider (Console Go): Upstream request failed","type":"invalid_request_error","param":null,"code":"invalid_request_error"}}"#;
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(403).set_body_string(body))
        .mount(&server)
        .await;

    let config = AiClientConfig {
        provider_id: ProviderId::from("custom_openai_compatible"),
        provider_label: "OpenAI-compatible".into(),
        auth: AuthConfig::Bearer {
            api_key: Some("key".into()),
            required: true,
        },
        adapter: ProviderAdapterConfig::OpenAiCompatible(OpenAiCompatibleConfig {
            base_url: server.uri(),
            wire_api: WireApi::ChatCompletions,
            responses_path: "v1/responses".into(),
            chat_completions_path: "v1/chat/completions".into(),
            model_transports: std::collections::BTreeMap::default(),
            request_timeout: std::time::Duration::from_mins(5),
        }),
        debug_output: false,
    };
    let client = create_provider(config);
    let err = client.invoke(test_request()).await.unwrap_err();
    assert!(err.is_retryable(), "expected transient, got {err}");
    assert!(!matches!(err, engine::AgentError::Permanent(_)));
}

#[tokio::test]
async fn chat_completions_upstream_400_maps_to_transient_invoke_and_stream() {
    let body = r#"{"error":{"message":"Error from provider (Console Go): Upstream request failed","type":"invalid_request_error","param":null,"code":"invalid_request_error"}}"#;
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(400).set_body_string(body))
        .mount(&server)
        .await;

    let config = AiClientConfig {
        provider_id: ProviderId::from("custom_openai_compatible"),
        provider_label: "OpenAI-compatible".into(),
        auth: AuthConfig::Bearer {
            api_key: Some("key".into()),
            required: true,
        },
        adapter: ProviderAdapterConfig::OpenAiCompatible(OpenAiCompatibleConfig {
            base_url: server.uri(),
            wire_api: WireApi::ChatCompletions,
            responses_path: "v1/responses".into(),
            chat_completions_path: "v1/chat/completions".into(),
            model_transports: std::collections::BTreeMap::default(),
            request_timeout: std::time::Duration::from_mins(5),
        }),
        debug_output: false,
    };
    let client = create_provider(config);

    let err = client.invoke(test_request()).await.unwrap_err();
    assert!(
        err.is_retryable(),
        "invoke expected transient, got {err} (is_retryable={})",
        err.is_retryable()
    );
    assert!(
        !matches!(err, engine::AgentError::Permanent(_)),
        "invoke must not be permanent: {err}"
    );

    let err = client
        .invoke_stream(test_request(), &RecordingSink::default())
        .await
        .unwrap_err();
    assert!(
        err.is_retryable(),
        "stream expected transient, got {err} (is_retryable={})",
        err.is_retryable()
    );
    assert!(
        !matches!(err, engine::AgentError::Permanent(_)),
        "stream must not be permanent: {err}"
    );
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
            model_transports: std::collections::BTreeMap::default(),
            request_timeout: std::time::Duration::from_mins(5),
        }),
        debug_output: false,
    };
    let client = create_provider(config);
    client.invoke(test_request()).await.unwrap();
}

#[tokio::test]
async fn chat_trailing_comma_arguments_repair_without_overseer_candidate() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(chat_completion_response(
            &json!({
                "role": "assistant",
                "tool_calls": [{
                    "id": "call-1",
                    "type": "function",
                    "function": {
                        "name": "openflow_submit_node_output",
                        "arguments": "{\"output\":{\"summary\":\"done\"},\"assistant_message\":null,}"
                    }
                }]
            }),
        )))
        .mount(&server)
        .await;

    let client = create_provider(openai_test_config(&server.uri(), WireApi::ChatCompletions));
    let outcome = client.invoke(test_request()).await.unwrap();
    let AgentTurnOutcome::Completed(success) = outcome else {
        panic!("expected completed outcome, got {outcome:?}");
    };
    assert_eq!(success.output, json!({"summary": "done"}));
}

#[tokio::test]
async fn chat_unrecoverable_submit_arguments_become_typed_repair_candidate() {
    let server = MockServer::start().await;
    let secret = "SECRET_CHAT_HTTP_RAW";
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(chat_completion_response(&json!({
                "role": "assistant",
                "tool_calls": [{
                    "id": "call-bad",
                    "type": "function",
                    "function": {
                        "name": "openflow_submit_node_output",
                        "arguments": format!("not-json-{secret}")
                    }
                }]
            }))),
        )
        .mount(&server)
        .await;

    let client = create_provider(openai_test_config(&server.uri(), WireApi::ChatCompletions));
    let err = client.invoke(test_request()).await.unwrap_err();
    assert!(err.is_malformed_submit_output());
    let candidate = err.output_repair_candidate().unwrap();
    assert!(candidate.raw_arguments().contains(secret));
    assert!(!err.to_string().contains(secret));
    assert!(!format!("{candidate:?}").contains(secret));
}

#[tokio::test]
async fn responses_unrecoverable_submit_arguments_become_typed_repair_candidate() {
    let server = MockServer::start().await;
    let secret = "SECRET_RESPONSES_HTTP_RAW";
    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "resp_bad",
            "object": "response",
            "created_at": 0,
            "status": "completed",
            "model": "test-model",
            "output": [{
                "type": "function_call",
                "id": "fc_bad",
                "call_id": "call-bad",
                "name": "openflow_submit_node_output",
                "arguments": format!("not-json-{secret}"),
                "status": "completed"
            }]
        })))
        .mount(&server)
        .await;

    let client = create_provider(openai_test_config(&server.uri(), WireApi::Responses));
    let err = client.invoke(test_request()).await.unwrap_err();
    assert!(err.is_malformed_submit_output());
    let candidate = err.output_repair_candidate().unwrap();
    assert!(candidate.raw_arguments().contains(secret));
    assert!(!err.to_string().contains(secret));
}

#[tokio::test]
async fn chat_malformed_outer_response_stays_generic_provider_failure() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_string("{not-a-completion"))
        .mount(&server)
        .await;

    let client = create_provider(openai_test_config(&server.uri(), WireApi::ChatCompletions));
    let err = client.invoke(test_request()).await.unwrap_err();
    assert!(!err.is_malformed_submit_output());
    assert!(err.output_repair_candidate().is_none());
}
