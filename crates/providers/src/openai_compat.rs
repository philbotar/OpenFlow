use crate::auth::{apply_auth, AuthConfig};
use crate::mapping::{
    all_tool_specs, parse_chat_completion_output, parse_responses_output, should_allow_user_input,
    tool_payload, transcript_to_chat_messages, transcript_to_responses_input,
};
use crate::spec::WireApi;
use engine::{AgentError, AgentRequest, AgentTurnOutcome};
use reqwest::Client;
use serde_json::{json, Value};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenAiCompatibleConfig {
    pub base_url: String,
    pub wire_api: WireApi,
    pub responses_path: String,
    pub chat_completions_path: String,
}

impl OpenAiCompatibleConfig {
    #[must_use]
    pub fn openai_default() -> Self {
        Self {
            base_url: "https://api.openai.com".to_string(),
            wire_api: WireApi::Responses,
            responses_path: "v1/responses".to_string(),
            chat_completions_path: "v1/chat/completions".to_string(),
        }
    }
}

pub async fn invoke(
    http: &Client,
    config: &OpenAiCompatibleConfig,
    auth: &AuthConfig,
    request: AgentRequest,
) -> Result<AgentTurnOutcome, AgentError> {
    match config.wire_api {
        WireApi::Responses => invoke_responses(http, config, auth, request).await,
        WireApi::ChatCompletions => invoke_chat_completions(http, config, auth, request).await,
    }
}

async fn invoke_responses(
    http: &Client,
    config: &OpenAiCompatibleConfig,
    auth: &AuthConfig,
    request: AgentRequest,
) -> Result<AgentTurnOutcome, AgentError> {
    let body = json!({
        "model": request.model,
        "input": transcript_to_responses_input(&request)?,
        "tools": all_tool_specs(&request)
            .into_iter()
            .map(|tool| tool_payload(&tool))
            .collect::<Vec<_>>()
    });

    let payload = post_json(http, config, auth, &config.responses_path, body, "OpenAI").await?;
    parse_responses_output(&payload, Some(&request.output_schema))
}

async fn invoke_chat_completions(
    http: &Client,
    config: &OpenAiCompatibleConfig,
    auth: &AuthConfig,
    request: AgentRequest,
) -> Result<AgentTurnOutcome, AgentError> {
    let body = json!({
        "model": request.model,
        "messages": transcript_to_chat_messages(&request)?,
        "tools": all_tool_specs(&request)
            .into_iter()
            .map(|tool| json!({
                "type": "function",
                "function": {
                    "name": tool.name,
                    "description": tool.description,
                    "parameters": tool.parameters,
                    "strict": true
                }
            }))
            .collect::<Vec<_>>()
    });

    let payload = post_json(
        http,
        config,
        auth,
        &config.chat_completions_path,
        body,
        "OpenAI-compatible",
    )
    .await?;
    parse_chat_completion_output(
        &payload,
        should_allow_user_input(&request),
        Some(&request.output_schema),
    )
}

async fn post_json(
    http: &Client,
    config: &OpenAiCompatibleConfig,
    auth: &AuthConfig,
    path: &str,
    body: Value,
    label: &str,
) -> Result<Value, AgentError> {
    let request = http.post(endpoint(&config.base_url, path));
    let request = apply_auth(request, auth, label)?.json(&body);
    let response = request
        .send()
        .await
        .map_err(|error| AgentError::Transient(format!("{label} request failed: {error}")))?;

    let status = response.status();
    let payload: Value = response
        .json()
        .await
        .map_err(|error| AgentError::Failed(format!("{label} response JSON failed: {error}")))?;

    if !status.is_success() {
        let message = format!("{label} returned HTTP {status}: {payload}");
        return if status.as_u16() == 429 || status.is_server_error() {
            Err(AgentError::Transient(message))
        } else {
            Err(AgentError::Permanent(message))
        };
    }

    Ok(payload)
}

fn endpoint(base_url: &str, path: &str) -> String {
    if path.starts_with("http://") || path.starts_with("https://") {
        path.to_string()
    } else {
        format!(
            "{}{}",
            base_url.trim_end_matches('/'),
            if path.starts_with('/') {
                path.to_string()
            } else {
                format!("/{path}")
            }
        )
    }
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    clippy::panic,
    clippy::too_many_lines,
    clippy::unwrap_used
)]
mod tests {
    use super::*;
    use crate::{AiClient, AiClientConfig, ProviderAdapterConfig, ProviderId};
    use engine::{
        AgentToolCallBatch, AgentTranscriptItem, AgentTurnSuccess, AiPort, ToolCall, ToolDefinition,
    };
    use wiremock::matchers::{body_json, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn request() -> AgentRequest {
        AgentRequest {
            workflow_id: engine::WorkflowId("wf-1".to_string()),
            node_id: engine::NodeId("idea".to_string()),
            node_label: "Idea".to_string(),
            model: "test-model".to_string(),
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
        }
    }

    fn client(base_url: String, wire_api: WireApi) -> AiClient {
        AiClient::with_config(AiClientConfig {
            provider_id: ProviderId::from("openai"),
            provider_label: "OpenAI".to_string(),
            auth: AuthConfig::Bearer {
                api_key: Some("key".to_string()),
                required: true,
            },
            adapter: ProviderAdapterConfig::OpenAiCompatible(OpenAiCompatibleConfig {
                base_url,
                wire_api,
                responses_path: "v1/responses".to_string(),
                chat_completions_path: "v1/chat/completions".to_string(),
            }),
        })
    }

    #[test]
    fn endpoint_uses_absolute_paths_without_joining_base_url() {
        assert_eq!(
            endpoint(
                "https://api.example.test",
                "https://other.example.test/v1/chat/completions"
            ),
            "https://other.example.test/v1/chat/completions"
        );
    }

    #[test]
    fn endpoint_joins_base_url_and_relative_path() {
        assert_eq!(
            endpoint("https://api.example.test/v1/", "chat/completions"),
            "https://api.example.test/v1/chat/completions"
        );
    }

    #[tokio::test]
    async fn responses_request_includes_bearer_auth_internal_tool_and_parses_completion() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/responses"))
            .and(header("authorization", "Bearer key"))
            .and(body_json(json!({
                "model": "test-model",
                "input": [
                    { "role": "system", "content": "You are precise." },
                    {
                        "role": "user",
                        "content": "Node: Idea\nTask:\nSummarize the kickoff.\n\nUpstream input JSON:\n{\"entrypoint\":{\"text\":\"ORCHID-91\"},\"upstream\":[]}"
                    }
                ],
                "tools": [{
                    "type": "function",
                    "name": "openflow_submit_node_output",
                    "description": "Submit the final structured node output when the task is complete. Required shape: {\"output\": {...schema fields...}, \"assistant_message\": null|string}. Schema fields must be nested under \"output\", not at the top level.",
                    "parameters": {
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
                    },
                    "strict": true
                }]
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "output": [{
                    "type": "function_call",
                    "call_id": "call-1",
                    "name": "openflow_submit_node_output",
                    "arguments": "{\"output\":{\"summary\":\"done\"},\"assistant_message\":null}"
                }]
            })))
            .mount(&server)
            .await;

        let outcome = client(server.uri(), WireApi::Responses)
            .invoke(request())
            .await
            .unwrap();
        let AgentTurnOutcome::Completed(success) = outcome else {
            panic!("expected completed outcome");
        };
        assert_eq!(success.output, json!({"summary": "done"}));
        assert_eq!(
            serde_json::from_str::<Value>(&success.raw_text).unwrap(),
            json!({"output": {"summary": "done"}, "assistant_message": null})
        );
        assert_eq!(success.assistant_message, None);
    }

    #[tokio::test]
    async fn chat_completions_request_sends_external_tools_and_parses_tool_calls() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .and(body_json(json!({
                "model": "test-model",
                "messages": [
                    { "role": "system", "content": "You are precise." },
                    {
                        "role": "user",
                        "content": "Node: Idea\nTask:\nSummarize the kickoff.\n\nUpstream input JSON:\n{\"entrypoint\":{\"text\":\"ORCHID-91\"},\"upstream\":[]}"
                    }
                ],
                "tools": [
                    {
                        "type": "function",
                        "function": {
                            "name": "read",
                            "description": "Read a file or URL.",
                            "parameters": {
                                "type": "object",
                                "additionalProperties": false,
                                "properties": {
                                    "path": { "type": "string" }
                                },
                                "required": ["path"]
                            },
                            "strict": true
                        }
                    },
                    {
                        "type": "function",
                        "function": {
                            "name": "openflow_submit_node_output",
                            "description": "Submit the final structured node output when the task is complete. Required shape: {\"output\": {...schema fields...}, \"assistant_message\": null|string}. Schema fields must be nested under \"output\", not at the top level.",
                            "parameters": {
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
                            },
                            "strict": true
                        }
                    }
                ]
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "choices": [{
                    "message": {
                        "content": "I need to inspect the README.",
                        "tool_calls": [{
                            "id": "call-7",
                            "type": "function",
                            "function": {
                                "name": "read",
                                "arguments": "{\"path\":\"README.md\"}"
                            }
                        }]
                    }
                }]
            })))
            .mount(&server)
            .await;

        let mut request = request();
        request.available_tools = vec![ToolDefinition {
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
        }];

        let outcome = client(server.uri(), WireApi::ChatCompletions)
            .invoke(request)
            .await
            .unwrap();
        assert_eq!(
            outcome,
            AgentTurnOutcome::ToolCalls(AgentToolCallBatch {
                raw_text: "I need to inspect the README.".to_string(),
                assistant_message: Some("I need to inspect the README.".to_string()),
                tool_calls: vec![ToolCall {
                    id: "call-7".to_string(),
                    name: "read".to_string(),
                    arguments: json!({"path": "README.md"}),
                    intent: None,
                }],
            })
        );
    }

    #[tokio::test]
    async fn chat_completions_plain_json_content_falls_back_to_completion() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "choices": [{
                    "message": {
                        "content": "{\"summary\":\"done without tool call\"}"
                    }
                }]
            })))
            .mount(&server)
            .await;

        let outcome = client(server.uri(), WireApi::ChatCompletions)
            .invoke(request())
            .await
            .unwrap();
        assert_eq!(
            outcome,
            AgentTurnOutcome::Completed(AgentTurnSuccess {
                output: json!({"summary": "done without tool call"}),
                raw_text: "{\"summary\":\"done without tool call\"}".to_string(),
                assistant_message: None,
            })
        );
    }

    #[tokio::test]
    async fn chat_completions_transcript_includes_tool_results_and_allows_human_input() {
        let server = MockServer::start().await;
        let mut request = request();
        request.transcript = vec![
            AgentTranscriptItem::UserMessage {
                content: "Need a safer rollout.".to_string(),
            },
            AgentTranscriptItem::ToolCall {
                call: ToolCall {
                    id: "call-1".to_string(),
                    name: "read".to_string(),
                    arguments: json!({"path": "README.md"}),
                    intent: Some("Reading repo overview".to_string()),
                },
            },
            AgentTranscriptItem::ToolResult {
                result: engine::ToolResult {
                    tool_call_id: "call-1".to_string(),
                    tool_name: "read".to_string(),
                    content: "# Overview".to_string(),
                    is_error: false,
                    artifact_ids: Vec::new(),
                    output_meta: None,
                },
            },
        ];
        request.available_tools = vec![ToolDefinition {
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
        }];

        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "choices": [{
                    "message": {
                        "tool_calls": [{
                            "id": "call-9",
                            "type": "function",
                            "function": {
                                "name": "openflow_request_user_input",
                                "arguments": "{\"assistant_message\":\"Which approver is mandatory?\"}"
                            }
                        }]
                    }
                }]
            })))
            .mount(&server)
            .await;

        let outcome = client(server.uri(), WireApi::ChatCompletions)
            .invoke(request)
            .await
            .unwrap();
        assert_eq!(
            outcome,
            AgentTurnOutcome::NeedsUserInput(engine::AgentNeedUserInput {
                raw_text: "{\"assistant_message\":\"Which approver is mandatory?\"}".to_string(),
                assistant_message: "Which approver is mandatory?".to_string(),
            })
        );
    }

    #[tokio::test]
    async fn malformed_tool_arguments_return_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/responses"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "output": [{
                    "type": "function_call",
                    "call_id": "call-1",
                    "name": "openflow_submit_node_output",
                    "arguments": "not-json"
                }]
            })))
            .mount(&server)
            .await;

        let error = client(server.uri(), WireApi::Responses)
            .invoke(request())
            .await
            .unwrap_err();
        assert!(error
            .to_string()
            .contains("OpenAI function_call arguments were not valid JSON"));
    }

    #[tokio::test]
    async fn chat_completions_truncated_arguments_recovers() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "choices": [{
                    "message": {
                        "content": null,
                        "tool_calls": [{
                            "id": "call-7",
                            "type": "function",
                            "function": {
                                "name": "read",
                                "arguments": "{\"path\": \"/Users/name/project/very/long/file.txt"
                            }
                        }]
                    }
                }]
            })))
            .mount(&server)
            .await;

        let mut request = request();
        request.available_tools = vec![ToolDefinition {
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
        }];

        let outcome = client(server.uri(), WireApi::ChatCompletions)
            .invoke(request)
            .await
            .unwrap();
        let AgentTurnOutcome::ToolCalls(batch) = outcome else {
            panic!("expected ToolCalls outcome");
        };
        assert_eq!(batch.tool_calls.len(), 1);
        assert_eq!(batch.tool_calls[0].name, "read");
        assert_eq!(
            batch.tool_calls[0].arguments,
            json!({"path": "/Users/name/project/very/long/file.txt"})
        );
    }

    #[tokio::test]
    async fn responses_truncated_arguments_recovers() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/responses"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "output": [{
                    "type": "function_call",
                    "call_id": "call-1",
                    "name": "read",
                    "arguments": "{\"path\": \"/Users/name/project/very/long/file.txt"
                }]
            })))
            .mount(&server)
            .await;

        let outcome = client(server.uri(), WireApi::Responses)
            .invoke(request())
            .await
            .unwrap();
        let AgentTurnOutcome::ToolCalls(batch) = outcome else {
            panic!("expected ToolCalls outcome");
        };
        assert_eq!(batch.tool_calls.len(), 1);
        assert_eq!(batch.tool_calls[0].name, "read");
        assert_eq!(
            batch.tool_calls[0].arguments,
            json!({"path": "/Users/name/project/very/long/file.txt"})
        );
    }
}
