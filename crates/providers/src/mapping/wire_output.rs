use engine::{AgentError, AgentTurnOutcome, ToolCall};
use serde_json::Value;

use super::{
    extract_usage_from_openai, resolve_tool_turn_outcome, NoToolCallsPolicy, ResolveToolTurnParams,
};

pub fn extract_chat_message_text(content: Option<&Value>) -> Option<String> {
    match content {
        Some(Value::String(text)) => Some(text.clone()),
        Some(Value::Array(parts)) => {
            let mut text = String::new();
            for part in parts {
                match part {
                    Value::String(value) => text.push_str(value),
                    Value::Object(map) => {
                        if let Some(value) = map.get("text").and_then(Value::as_str) {
                            text.push_str(value);
                        } else if let Some(value) = map.get("refusal").and_then(Value::as_str) {
                            text.push_str(value);
                        }
                    }
                    _ => {}
                }
            }
            (!text.trim().is_empty()).then_some(text)
        }
        _ => None,
    }
}

pub fn parse_compatible_tool_call(call: &Value) -> Result<ToolCall, AgentError> {
    let call_id = call
        .get("id")
        .and_then(Value::as_str)
        .or_else(|| call.get("call_id").and_then(Value::as_str))
        .unwrap_or("call-legacy");

    let function = call.get("function").unwrap_or(call);

    let name = function
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            AgentError::Failed("OpenAI-compatible tool call missing function.name".to_string())
        })?;

    let arguments = function
        .get("arguments")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            AgentError::Failed("OpenAI-compatible tool call missing function.arguments".to_string())
        })?;

    Ok(ToolCall {
        id: call_id.to_string(),
        name: name.to_string(),
        arguments: super::try_parse_or_recover_json(arguments).map_err(|error| {
            AgentError::Failed(format!(
                "OpenAI-compatible tool call arguments were not valid JSON: {error}"
            ))
        })?,
    })
}

pub fn parse_responses_output(
    payload: &Value,
    output_schema: Option<&Value>,
) -> Result<AgentTurnOutcome, AgentError> {
    let usage = extract_usage_from_openai(payload);
    let output = payload
        .get("output")
        .and_then(Value::as_array)
        .ok_or_else(|| AgentError::Failed("OpenAI response missing output array".to_string()))?;

    let mut assistant_message = None;
    let mut tool_calls = Vec::new();

    for item in output {
        match item.get("type").and_then(Value::as_str) {
            Some("message") => {
                let content = item
                    .get("content")
                    .and_then(Value::as_array)
                    .ok_or_else(|| {
                        AgentError::Failed("OpenAI message missing content array".to_string())
                    })?;
                for content_item in content {
                    match content_item.get("type").and_then(Value::as_str) {
                        Some("output_text") => {
                            if let Some(text) = content_item.get("text").and_then(Value::as_str) {
                                assistant_message = Some(text.to_string());
                            }
                        }
                        Some("refusal") => {
                            let refusal = content_item
                                .get("refusal")
                                .and_then(Value::as_str)
                                .unwrap_or("model refused the request");
                            return Err(AgentError::Failed(format!("OpenAI refusal: {refusal}")));
                        }
                        _ => {}
                    }
                }
            }
            Some("function_call") => {
                let call_id = item
                    .get("call_id")
                    .or_else(|| item.get("id"))
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        AgentError::Failed("OpenAI function_call missing call id".to_string())
                    })?;

                let name = item.get("name").and_then(Value::as_str).ok_or_else(|| {
                    AgentError::Failed("OpenAI function_call missing name".to_string())
                })?;

                let arguments = item
                    .get("arguments")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        AgentError::Failed("OpenAI function_call missing arguments".to_string())
                    })?;

                tool_calls.push(ToolCall {
                    id: call_id.to_string(),
                    name: name.to_string(),
                    arguments: super::try_parse_or_recover_json(arguments).map_err(|error| {
                        AgentError::Failed(format!(
                            "OpenAI function_call arguments were not valid JSON: {error}"
                        ))
                    })?,
                });
            }
            _ => {}
        }
    }

    resolve_tool_turn_outcome(ResolveToolTurnParams {
        tool_calls,
        assistant_message,
        no_tool_calls: NoToolCallsPolicy::Error("OpenAI response did not contain a function call"),
        output_schema,
        provider_label: "OpenAI",
        usage,
        filter_assistant_on_external_batch: true,
    })
}

pub fn parse_chat_completion_output(
    payload: &Value,
    allow_plain_text_follow_up: bool,
    output_schema: Option<&Value>,
) -> Result<AgentTurnOutcome, AgentError> {
    let usage = extract_usage_from_openai(payload);
    let message = payload
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("message"))
        .ok_or_else(|| {
            AgentError::Failed("OpenAI-compatible response missing choices[0].message".to_string())
        })?;

    if let Some(refusal) = message.get("refusal").and_then(Value::as_str) {
        return Err(AgentError::Failed(format!(
            "OpenAI-compatible refusal: {refusal}"
        )));
    }

    let assistant_message = extract_chat_message_text(message.get("content"))
        .map(|content| content.trim().to_string())
        .filter(|content| !content.is_empty());

    let tool_calls = message
        .get("tool_calls")
        .and_then(Value::as_array)
        .cloned()
        .or_else(|| message.get("function_call").cloned().map(|call| vec![call]))
        .unwrap_or_default();

    let parsed = tool_calls
        .iter()
        .map(parse_compatible_tool_call)
        .collect::<Result<Vec<_>, AgentError>>()?;

    resolve_tool_turn_outcome(ResolveToolTurnParams {
        tool_calls: parsed,
        assistant_message,
        no_tool_calls: NoToolCallsPolicy::Recover {
            allow_plain_text_follow_up,
            error: "OpenAI-compatible response did not contain a tool call, plain JSON completion, or follow-up prompt",
        },
        output_schema,
        provider_label: "OpenAI-compatible",
        usage,
        filter_assistant_on_external_batch: true,
    })
}
