use super::*;
use crate::auth::AuthConfig;
use crate::client::AnthropicConfig;
use crate::{AiClient, AiClientConfig, ProviderAdapterConfig, ProviderId};
use engine::{AiPort, ToolDefinition};
use wiremock::matchers::{body_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn request() -> AgentRequest {
    AgentRequest {
        workflow_id: engine::WorkflowId("wf-1".to_string()),
        node_id: engine::NodeId("idea".to_string()),
        node_label: "Idea".to_string(),
        model: "claude-3-5-sonnet-latest".to_string(),
        system_messages: vec!["You are precise.".to_string()],
        task_prompt: "Summarize the kickoff.".to_string(),
        input: json!({"entrypoint": {"text": "ORCHID-91"}, "upstream": []}),
        output_schema: json!({
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

fn client(base_url: String) -> AiClient {
    AiClient::with_config(AiClientConfig {
        provider_id: ProviderId::from("anthropic"),
        provider_label: "Anthropic".to_string(),
        auth: AuthConfig::Header {
            name: "x-api-key".to_string(),
            api_key: Some("test-key".to_string()),
            required: true,
        },
        adapter: ProviderAdapterConfig::Anthropic(AnthropicConfig {
            base_url,
            messages_path: "v1/messages".to_string(),
            anthropic_version: "2023-06-01".to_string(),
        }),
    })
}

fn read_tool_definition() -> ToolDefinition {
    ToolDefinition {
        name: "read".to_string(),
        description: "Read a file or URL.".to_string(),
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "path": { "type": "string" }
            },
            "required": ["path"]
        }),
        tier: engine::ToolTier::Read,
        concurrency: engine::ToolConcurrency::Shared,
    }
}

fn read_tool_loop_transcript() -> Vec<AgentTranscriptItem> {
    vec![
        AgentTranscriptItem::ToolCall {
            call: ToolCall {
                id: "toolu_1".to_string(),
                name: "read".to_string(),
                arguments: json!({"path": "README.md"}),
            },
        },
        AgentTranscriptItem::ToolResult {
            result: engine::ToolResult {
                tool_call_id: "toolu_1".to_string(),
                tool_name: "read".to_string(),
                content: "# Title".to_string(),
                is_error: false,
                artifact_ids: Vec::new(),
                output_meta: None,
            },
        },
    ]
}

fn tool_loop_cache_control_expected_body() -> Value {
    json!({
        "model": "claude-3-5-sonnet-latest",
        "max_tokens": 4096,
        "system": [{
            "type": "text",
            "text": "You are precise.",
            "cache_control": { "type": "ephemeral" }
        }],
        "messages": [
            {
                "role": "user",
                "content": [{
                    "type": "text",
                    "text": "Node: Idea\nTask:\nSummarize the kickoff.\n\nUpstream input JSON:\n{\"entrypoint\":{\"text\":\"ORCHID-91\"},\"upstream\":[]}"
                }]
            },
            {
                "role": "assistant",
                "content": [{
                    "type": "tool_use",
                    "id": "toolu_1",
                    "name": "read",
                    "input": {"path": "README.md"},
                    "cache_control": { "type": "ephemeral" }
                }]
            },
            {
                "role": "user",
                "content": [{
                    "type": "tool_result",
                    "tool_use_id": "toolu_1",
                    "content": "# Title",
                    "is_error": false
                }]
            }
        ],
        "tools": [{
            "name": "read",
            "description": "Read a file or URL.",
            "input_schema": {
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "path": { "type": "string" }
                },
                "required": ["path"]
            }
        }, {
            "name": "openflow_submit_node_output",
            "description": "Submit the final structured node output when the task is complete. Required shape: {\"output\": {...schema fields...}, \"assistant_message\": null|string}. Schema fields must be nested under \"output\", not at the top level.",
            "input_schema": {
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "output": {
                        "type": "object",
                        "additionalProperties": false,
                        "properties": {
                            "summary": { "type": "string" }
                        },
                        "required": ["summary"]
                    },
                    "assistant_message": {
                        "type": ["string", "null"],
                        "description": "Optional human-facing note to show alongside the final result."
                    }
                },
                "required": ["output", "assistant_message"]
            }
        }]
    })
}

#[tokio::test]
async fn messages_request_sends_headers_body_and_parses_internal_submit_output() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .and(header("x-api-key", "test-key"))
        .and(header("anthropic-version", "2023-06-01"))
        .and(body_json(json!({
            "model": "claude-3-5-sonnet-latest",
            "max_tokens": 4096,
            "system": [{
                "type": "text",
                "text": "You are precise.",
                "cache_control": { "type": "ephemeral" }
            }],
            "messages": [{
                "role": "user",
                "content": [{
                    "type": "text",
                    "text": "Node: Idea\nTask:\nSummarize the kickoff.\n\nUpstream input JSON:\n{\"entrypoint\":{\"text\":\"ORCHID-91\"},\"upstream\":[]}"
                }]
            }],
            "tools": [{
                "name": "openflow_submit_node_output",
                "description": "Submit the final structured node output when the task is complete. Required shape: {\"output\": {...schema fields...}, \"assistant_message\": null|string}. Schema fields must be nested under \"output\", not at the top level.",
                "input_schema": {
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                        "output": {
                            "type": "object",
                            "additionalProperties": false,
                            "properties": {
                                "summary": { "type": "string" }
                            },
                            "required": ["summary"]
                        },
                        "assistant_message": {
                            "type": ["string", "null"],
                            "description": "Optional human-facing note to show alongside the final result."
                        }
                    },
                    "required": ["output", "assistant_message"]
                }
            }]
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "usage": {
                "input_tokens": 101,
                "output_tokens": 19
            },
            "content": [{
                "type": "tool_use",
                "id": "toolu_1",
                "name": "openflow_submit_node_output",
                "input": {
                    "output": {"summary": "done"},
                    "assistant_message": null
                }
            }]
        })))
        .mount(&server)
        .await;

    let outcome = client(server.uri()).invoke(request()).await.unwrap();
    let AgentTurnOutcome::Completed(success) = outcome else {
        panic!("expected completed outcome");
    };
    assert_eq!(success.output, json!({"summary": "done"}));
    assert_eq!(
        serde_json::from_str::<Value>(&success.raw_text).unwrap(),
        json!({"output": {"summary": "done"}, "assistant_message": null})
    );
    assert_eq!(
        success.usage,
        Some(engine::UsageReport {
            prompt_tokens: 101,
            completion_tokens: 19,
            total_tokens: 120,
        })
    );
}

#[tokio::test]
async fn messages_response_routes_external_tool_calls() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "usage": {
                "input_tokens": 88,
                "output_tokens": 12,
                "total_tokens": 111
            },
            "content": [
                {"type": "text", "text": "I need to inspect the README."},
                {
                    "type": "tool_use",
                    "id": "toolu_2",
                    "name": "read",
                    "input": {"path": "README.md"}
                }
            ]
        })))
        .mount(&server)
        .await;
    let mut request = request();
    request.available_tools = vec![read_tool_definition()];

    let outcome = client(server.uri()).invoke(request).await.unwrap();

    assert_eq!(
        outcome,
        AgentTurnOutcome::ToolCalls(AgentToolCallBatch {
            raw_text: "I need to inspect the README.".to_string(),
            assistant_message: Some("I need to inspect the README.".to_string()),
            tool_calls: vec![ToolCall {
                id: "toolu_2".to_string(),
                name: "read".to_string(),
                arguments: json!({"path": "README.md"}),
            }],
            usage: Some(engine::UsageReport {
                prompt_tokens: 88,
                completion_tokens: 12,
                total_tokens: 111,
            }),
        })
    );
}

#[tokio::test]
async fn messages_tool_loop_places_cache_control_on_second_to_last_message() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .and(body_json(tool_loop_cache_control_expected_body()))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "content": [{
                "type": "tool_use",
                "id": "toolu_2",
                "name": "openflow_submit_node_output",
                "input": {
                    "output": {"summary": "done"},
                    "assistant_message": null
                }
            }]
        })))
        .mount(&server)
        .await;

    let mut request = request();
    request.available_tools = vec![read_tool_definition()];
    request.transcript = read_tool_loop_transcript();

    let outcome = client(server.uri()).invoke(request).await.unwrap();
    let AgentTurnOutcome::Completed(success) = outcome else {
        panic!("expected completed outcome");
    };
    assert_eq!(success.output, json!({"summary": "done"}));
}

#[tokio::test]
async fn messages_errors_map_auth_and_rate_limit_statuses() {
    for (status, expected, retryable) in [
        (401, "Anthropic authentication failed", false),
        (429, "Anthropic rate limit exceeded", true),
    ] {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(status).set_body_json(json!({
                "error": {"message": "provider error"}
            })))
            .mount(&server)
            .await;

        let error = client(server.uri()).invoke(request()).await.unwrap_err();
        assert!(error.to_string().contains(expected));
        assert_eq!(error.is_retryable(), retryable);
        assert!(!error.to_string().contains("test-key"));
    }
}
