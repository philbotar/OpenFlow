use crate::auth::{apply_auth, AuthConfig};
use crate::client::AnthropicConfig;
use crate::mapping::{
    all_tool_specs, build_node_context, extract_usage_from_anthropic, resolve_tool_turn_outcome,
    should_allow_user_input, NoToolCallsPolicy, ResolveToolTurnParams, ToolSpec,
};
use crate::prompt_cache::{
    apply_cache_control_to_message, ephemeral_cache_control, second_to_last_index,
};
use engine::{
    emit_assistant_deltas_from_outcome, AgentError, AgentRequest, AgentTranscriptItem,
    AgentTurnOutcome, AiStreamSink, ToolCall,
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
    let body = build_anthropic_request_body(&request);

    let payload = post_json(http, config, auth, &config.messages_path, body).await?;
    parse_anthropic_output(
        &payload,
        should_allow_user_input(&request),
        Some(&request.output_schema),
    )
}

fn anthropic_system_blocks(request: &AgentRequest) -> Vec<Value> {
    vec![json!({
        "type": "text",
        "text": request.system_content(),
        "cache_control": ephemeral_cache_control(),
    })]
}

fn build_anthropic_request_body(request: &AgentRequest) -> Value {
    json!({
        "model": request.model,
        "max_tokens": DEFAULT_MAX_TOKENS,
        "system": anthropic_system_blocks(request),
        "messages": transcript_to_anthropic_messages(request),
        "tools": all_tool_specs(request)
            .into_iter()
            .map(|tool| anthropic_tool_payload(&tool))
            .collect::<Vec<_>>()
    })
}

fn transcript_to_anthropic_messages(request: &AgentRequest) -> Vec<Value> {
    let mut messages = vec![json!({
        "role": "user",
        "content": [{
            "type": "text",
            "text": build_node_context(request),
        }]
    })];

    for item in &request.transcript {
        match item {
            AgentTranscriptItem::AssistantMessage { content } => {
                messages.push(json!({
                    "role": "assistant",
                    "content": [{ "type": "text", "text": content }]
                }));
            }
            AgentTranscriptItem::UserMessage { content } => {
                messages.push(json!({
                    "role": "user",
                    "content": [{ "type": "text", "text": content }]
                }));
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

    if let Some(index) = second_to_last_index(messages.len()) {
        apply_cache_control_to_message(&mut messages[index]);
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
    let usage = extract_usage_from_anthropic(payload);
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

    resolve_tool_turn_outcome(ResolveToolTurnParams {
        tool_calls,
        assistant_message,
        no_tool_calls: NoToolCallsPolicy::Recover {
            allow_plain_text_follow_up,
            error: "Anthropic response did not contain a tool call, plain JSON completion, or follow-up prompt",
        },
        output_schema,
        provider_label: "Anthropic",
        usage,
        filter_assistant_on_external_batch: false,
    })
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
    })
}

#[cfg(test)]
#[path = "anthropic_tests.rs"]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "provider wire tests use unwrap/expect and panic for concise failures"
)]
mod anthropic_tests;
