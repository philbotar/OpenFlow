use crate::auth::{apply_auth, AuthConfig};
use crate::client::AnthropicConfig;
use crate::mapping::{
    all_tool_specs, build_node_context, parse_internal_tool_outcome, parse_plain_json_completion,
    should_allow_user_input, ToolSpec, REQUEST_INPUT_TOOL, SUBMIT_OUTPUT_TOOL,
};
use engine::{
    emit_assistant_deltas_from_outcome, AgentError, AgentNeedUserInput, AgentRequest,
    AgentToolCallBatch, AgentTranscriptItem, AgentTurnOutcome, AiStreamSink, ToolCall,
};
use reqwest::Client;
use serde_json::{json, Value};

const DEFAULT_MAX_TOKENS: u16 = 4096;

pub async fn invoke_stream(
    http: &Client,
    config: &AnthropicConfig,
    auth: &AuthConfig,
    request: AgentRequest,
    sink: &dyn AiStreamSink,
) -> Result<AgentTurnOutcome, AgentError> {
    let outcome = invoke(http, config, auth, request).await?;
    emit_assistant_deltas_from_outcome(sink, &outcome);
    Ok(outcome)
}

pub async fn invoke(
    http: &Client,
    config: &AnthropicConfig,
    auth: &AuthConfig,
    request: AgentRequest,
) -> Result<AgentTurnOutcome, AgentError> {
    let body = json!({
        "model": request.model,
        "max_tokens": DEFAULT_MAX_TOKENS,
        "system": request.system_content(),
        "messages": transcript_to_anthropic_messages(&request),
        "tools": all_tool_specs(&request)
            .into_iter()
            .map(|tool| anthropic_tool_payload(&tool))
            .collect::<Vec<_>>()
    });

    let payload = post_json(http, config, auth, &config.messages_path, body).await?;
    parse_anthropic_output(
        &payload,
        should_allow_user_input(&request),
        Some(&request.output_schema),
    )
}

fn transcript_to_anthropic_messages(request: &AgentRequest) -> Vec<Value> {
    let mut messages = vec![json!({
        "role": "user",
        "content": build_node_context(request)
    })];

    for item in &request.transcript {
        match item {
            AgentTranscriptItem::AssistantMessage { content } => {
                messages.push(json!({ "role": "assistant", "content": content }));
            }
            AgentTranscriptItem::UserMessage { content } => {
                messages.push(json!({ "role": "user", "content": content }));
            }
            AgentTranscriptItem::ToolCall { call } => {
                messages.push(json!({
                    "role": "assistant",
                    "content": [{
                        "type": "tool_use",
                        "id": call.id,
                        "name": call.name,
                        "input": call.arguments
                    }]
                }));
            }
            AgentTranscriptItem::ToolResult { result } => {
                messages.push(json!({
                    "role": "user",
                    "content": [{
                        "type": "tool_result",
                        "tool_use_id": result.tool_call_id,
                        "content": result.content,
                        "is_error": result.is_error
                    }]
                }));
            }
        }
    }

    messages
}

fn anthropic_tool_payload(tool: &ToolSpec) -> Value {
    json!({
        "name": tool.name,
        "description": tool.description,
        "input_schema": tool.parameters
    })
}

async fn post_json(
    http: &Client,
    config: &AnthropicConfig,
    auth: &AuthConfig,
    path: &str,
    body: Value,
) -> Result<Value, AgentError> {
    let request = http
        .post(endpoint(&config.base_url, path))
        .header("anthropic-version", config.anthropic_version.as_str());
    let request = apply_auth(request, auth, "Anthropic")?.json(&body);
    let response = request
        .send()
        .await
        .map_err(|error| AgentError::Transient(format!("Anthropic request failed: {error}")))?;

    let status = response.status();
    let payload: Value = response
        .json()
        .await
        .map_err(|error| AgentError::Failed(format!("Anthropic response JSON failed: {error}")))?;

    if !status.is_success() {
        let prefix = match status.as_u16() {
            401 | 403 => "Anthropic authentication failed",
            429 => "Anthropic rate limit exceeded",
            _ => "Anthropic returned",
        };
        let message = format!("{prefix} HTTP {status}: {payload}");
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

fn parse_anthropic_output(
    payload: &Value,
    allow_plain_text_follow_up: bool,
    output_schema: Option<&Value>,
) -> Result<AgentTurnOutcome, AgentError> {
    let content = payload
        .get("content")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            AgentError::Failed("Anthropic response missing content array".to_string())
        })?;

    let mut assistant_text_parts = Vec::new();
    let mut tool_calls = Vec::new();

    for block in content {
        match block.get("type").and_then(Value::as_str) {
            Some("text") => {
                if let Some(text) = block.get("text").and_then(Value::as_str) {
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        assistant_text_parts.push(trimmed.to_string());
                    }
                }
            }
            Some("tool_use") => tool_calls.push(parse_anthropic_tool_call(block)?),
            _ => {}
        }
    }

    let assistant_message =
        (!assistant_text_parts.is_empty()).then(|| assistant_text_parts.join("\n"));

    if tool_calls.is_empty() {
        if let Some(outcome) = parse_plain_json_completion(assistant_message.as_deref()) {
            return Ok(outcome);
        }
        if allow_plain_text_follow_up {
            if let Some(assistant_message) = assistant_message {
                return Ok(AgentTurnOutcome::NeedsUserInput(AgentNeedUserInput {
                    raw_text: assistant_message.clone(),
                    assistant_message,
                }));
            }
        }
        return Err(AgentError::Failed(
            "Anthropic response did not contain a tool call, plain JSON completion, or follow-up prompt"
                .to_string(),
        ));
    }

    if let Some(index) = tool_calls
        .iter()
        .position(|call| call.name == SUBMIT_OUTPUT_TOOL || call.name == REQUEST_INPUT_TOOL)
    {
        if tool_calls.len() != 1 {
            return Err(AgentError::Failed(
                "Anthropic response mixed internal and external tool calls".to_string(),
            ));
        }
        let call = &tool_calls[index];
        return parse_internal_tool_outcome(
            &call.name,
            &call.arguments.to_string(),
            assistant_message,
            "Anthropic",
            output_schema,
        );
    }

    Ok(AgentTurnOutcome::ToolCalls(AgentToolCallBatch {
        raw_text: assistant_message.clone().unwrap_or_default(),
        assistant_message,
        tool_calls,
    }))
}

fn parse_anthropic_tool_call(block: &Value) -> Result<ToolCall, AgentError> {
    let id = block
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| AgentError::Failed("Anthropic tool_use missing id".to_string()))?;
    let name = block
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| AgentError::Failed("Anthropic tool_use missing name".to_string()))?;
    let input = block
        .get("input")
        .cloned()
        .ok_or_else(|| AgentError::Failed("Anthropic tool_use missing input".to_string()))?;

    Ok(ToolCall {
        id: id.to_string(),
        name: name.to_string(),
        arguments: input,
        intent: None,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
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
                "system": "You are precise.",
                "messages": [{
                    "role": "user",
                    "content": "Node: Idea\nTask:\nSummarize the kickoff.\n\nUpstream input JSON:\n{\"entrypoint\":{\"text\":\"ORCHID-91\"},\"upstream\":[]}"
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
    }

    #[tokio::test]
    async fn messages_response_routes_external_tool_calls() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
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
                    intent: None,
                }],
            })
        );
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
}
