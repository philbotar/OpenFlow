use engine::{
    filter_tool_turn_assistant_message, AgentError, AgentNeedUserInput, AgentRequest,
    AgentToolCallBatch, AgentTranscriptItem, AgentTurnOutcome, AgentTurnSuccess, ToolCall,
    ToolDefinition,
};
use serde::Deserialize;
use serde_json::{json, Value};

pub const SUBMIT_OUTPUT_TOOL: &str = "openflow_submit_node_output";
pub const REQUEST_INPUT_TOOL: &str = "openflow_request_user_input";
pub const MALFORMED_SUBMIT_OUTPUT_MARKER: &str = "final output tool arguments were not valid JSON";

#[derive(Debug, Clone)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

pub fn build_node_context(request: &AgentRequest) -> String {
    format!(
        "Node: {}\nTask:\n{}\n\nUpstream input JSON:\n{}",
        request.node_label, request.task_prompt, request.input
    )
}

pub fn should_allow_user_input(request: &AgentRequest) -> bool {
    request.transcript.iter().any(|item| {
        matches!(
            item,
            AgentTranscriptItem::UserMessage { .. } | AgentTranscriptItem::AssistantMessage { .. }
        )
    })
}

pub fn submit_output_tool(request: &AgentRequest) -> ToolSpec {
    ToolSpec {
        name: SUBMIT_OUTPUT_TOOL.to_string(),
        description: "Submit the final structured node output when the task is complete. Required shape: {\"output\": {...schema fields...}, \"assistant_message\": null|string}. Schema fields must be nested under \"output\", not at the top level."
            .to_string(),
        parameters: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "output": request.output_schema,
                "assistant_message": {
                    "type": ["string", "null"],
                    "description": "Optional human-facing note to show alongside the final result."
                }
            },
            "required": ["output", "assistant_message"]
        }),
    }
}

pub fn request_input_tool() -> ToolSpec {
    ToolSpec {
        name: REQUEST_INPUT_TOOL.to_string(),
        description:
            "Pause the node and ask the human one direct clarifying question before continuing."
                .to_string(),
        parameters: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "assistant_message": {
                    "type": "string",
                    "description": "The exact question for the human (typically ending with ?). Must not be preamble, narration, or a plan — ask the question directly."
                }
            },
            "required": ["assistant_message"]
        }),
    }
}

pub fn external_tool_spec(tool: &ToolDefinition) -> ToolSpec {
    ToolSpec {
        name: tool.name.clone(),
        description: tool.description.clone(),
        parameters: tool.input_schema.clone(),
    }
}

pub fn all_tool_specs(request: &AgentRequest) -> Vec<ToolSpec> {
    let mut tools = request
        .available_tools
        .iter()
        .map(external_tool_spec)
        .collect::<Vec<_>>();
    tools.push(submit_output_tool(request));
    if should_allow_user_input(request) {
        tools.push(request_input_tool());
    }
    tools
}

pub fn tool_payload(tool: &ToolSpec) -> Value {
    json!({
        "type": "function",
        "name": tool.name,
        "description": tool.description,
        "parameters": tool.parameters,
        "strict": true
    })
}

pub fn transcript_to_responses_input(request: &AgentRequest) -> Result<Vec<Value>, AgentError> {
    let mut input = vec![
        json!({ "role": "system", "content": request.system_content() }),
        json!({ "role": "user", "content": build_node_context(request) }),
    ];

    for item in &request.transcript {
        match item {
            AgentTranscriptItem::AssistantMessage { content } => {
                input.push(json!({ "role": "assistant", "content": content }));
            }
            AgentTranscriptItem::UserMessage { content } => {
                input.push(json!({ "role": "user", "content": content }));
            }
            AgentTranscriptItem::ToolCall { call } => {
                input.push(json!({
                    "type": "function_call",
                    "call_id": call.id,
                    "name": call.name,
                    "arguments": serde_json::to_string(&call.arguments).map_err(|e| AgentError::Failed(format!("tool arguments serialize: {e}")))?
                }));
            }
            AgentTranscriptItem::ToolResult { result } => {
                input.push(json!({
                    "type": "function_call_output",
                    "call_id": result.tool_call_id,
                    "output": result.content
                }));
            }
        }
    }

    Ok(input)
}

pub fn transcript_to_chat_messages(request: &AgentRequest) -> Result<Vec<Value>, AgentError> {
    let mut messages = vec![
        json!({ "role": "system", "content": request.system_content() }),
        json!({ "role": "user", "content": build_node_context(request) }),
    ];

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
                    "content": Value::Null,
                    "tool_calls": [{
                        "id": call.id,
                        "type": "function",
                        "function": {
                            "name": call.name,
                            "arguments": serde_json::to_string(&call.arguments).map_err(|e| AgentError::Failed(format!("tool arguments serialize: {e}")))?
                        }
                    }]
                }));
            }
            AgentTranscriptItem::ToolResult { result } => {
                messages.push(json!({
                    "role": "tool",
                    "tool_call_id": result.tool_call_id,
                    "content": result.content
                }));
            }
        }
    }

    Ok(messages)
}

/// When models omit the `output` wrapper, lift top-level schema fields under `output`.
#[must_use]
pub fn normalize_submit_output_arguments(value: Value, output_schema: Option<&Value>) -> Value {
    if value.get("output").is_some() {
        return value;
    }
    let Value::Object(mut map) = value else {
        return value;
    };
    let assistant_message = map.remove("assistant_message");
    if map.is_empty() {
        return json!({ "assistant_message": assistant_message });
    }

    let schema_keys = output_schema
        .and_then(|schema| schema.get("properties"))
        .and_then(Value::as_object)
        .map(|properties| {
            properties
                .keys()
                .cloned()
                .collect::<std::collections::HashSet<_>>()
        });

    let should_wrap = schema_keys
        .as_ref()
        .is_none_or(|keys| !map.is_empty() && map.keys().all(|key| keys.contains(key)));
    if !should_wrap {
        if let Some(assistant_message) = assistant_message {
            map.insert("assistant_message".to_string(), assistant_message);
        }
        return Value::Object(map);
    }

    json!({
        "output": Value::Object(map),
        "assistant_message": assistant_message
    })
}

pub fn parse_internal_tool_outcome(
    tool_name: &str,
    arguments: &str,
    assistant_message: Option<String>,
    label: &str,
    output_schema: Option<&Value>,
) -> Result<AgentTurnOutcome, AgentError> {
    match tool_name {
        SUBMIT_OUTPUT_TOOL => {
            #[derive(Deserialize)]
            struct SubmitOutputArgs {
                output: Value,
                assistant_message: Option<String>,
            }

            let raw = try_parse_or_recover_json(arguments).map_err(|error| {
                AgentError::Failed(format!("{label} {MALFORMED_SUBMIT_OUTPUT_MARKER}: {error}"))
            })?;
            let normalized = normalize_submit_output_arguments(raw, output_schema);
            let args: SubmitOutputArgs = serde_json::from_value(normalized).map_err(|error| {
                AgentError::Failed(format!("{label} {MALFORMED_SUBMIT_OUTPUT_MARKER}: {error}"))
            })?;
            Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
                output: args.output,
                raw_text: arguments.to_string(),
                assistant_message: filter_tool_turn_assistant_message(
                    args.assistant_message.or(assistant_message),
                ),
            }))
        }
        REQUEST_INPUT_TOOL => {
            #[derive(Deserialize)]
            struct RequestInputArgs {
                assistant_message: String,
            }

            let args: RequestInputArgs =
                try_deserialize_or_recover_json(arguments).map_err(|error| {
                    AgentError::Failed(format!(
                        "{label} human-input tool arguments were not valid JSON: {error}"
                    ))
                })?;
            Ok(AgentTurnOutcome::NeedsUserInput(AgentNeedUserInput {
                raw_text: arguments.to_string(),
                assistant_message: args.assistant_message,
            }))
        }
        _ => Err(AgentError::Failed(format!(
            "{label} attempted unknown internal tool {tool_name}"
        ))),
    }
}

pub fn parse_plain_json_completion(content: Option<&str>) -> Option<AgentTurnOutcome> {
    let content = content
        .map(str::trim)
        .filter(|content| !content.is_empty())?;
    let candidate = content
        .strip_prefix("```json")
        .and_then(|value| value.strip_suffix("```"))
        .map(str::trim)
        .or_else(|| {
            content
                .strip_prefix("```")
                .and_then(|value| value.strip_suffix("```"))
                .map(str::trim)
        })
        .unwrap_or(content);
    let Ok(output) = try_parse_or_recover_json(candidate) else {
        return None;
    };
    Some(AgentTurnOutcome::Completed(AgentTurnSuccess {
        output,
        raw_text: content.to_string(),
        assistant_message: None,
    }))
}

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

/// Attempt to parse a JSON string, repairing common LLM output issues when needed.
/// Uses `jsonrepair-rs` for truncation, trailing commas, single quotes, and similar
/// malformed JSON before falling back to the original serde error.
fn try_parse_or_recover_json(input: &str) -> Result<Value, serde_json::Error> {
    match serde_json::from_str(input) {
        Ok(value) => Ok(value),
        Err(original_err) => {
            let trimmed = input.trim_start();
            if !trimmed.starts_with('{') && !trimmed.starts_with('[') {
                return Err(original_err);
            }

            let repaired = jsonrepair_rs::jsonrepair(input).map_err(|_| original_err)?;
            serde_json::from_str(&repaired)
        }
    }
}

fn try_deserialize_or_recover_json<T: for<'de> Deserialize<'de>>(
    input: &str,
) -> Result<T, serde_json::Error> {
    let value = try_parse_or_recover_json(input)?;
    serde_json::from_value(value)
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
        arguments: try_parse_or_recover_json(arguments).map_err(|error| {
            AgentError::Failed(format!(
                "OpenAI-compatible tool call arguments were not valid JSON: {error}"
            ))
        })?,
        intent: None,
    })
}

pub fn parse_responses_output(
    payload: &Value,
    output_schema: Option<&Value>,
) -> Result<AgentTurnOutcome, AgentError> {
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
                    arguments: try_parse_or_recover_json(arguments).map_err(|error| {
                        AgentError::Failed(format!(
                            "OpenAI function_call arguments were not valid JSON: {error}"
                        ))
                    })?,
                    intent: None,
                });
            }
            _ => {}
        }
    }

    if tool_calls.is_empty() {
        return Err(AgentError::Failed(
            "OpenAI response did not contain a function call".to_string(),
        ));
    }

    if let Some(index) = tool_calls
        .iter()
        .position(|call| call.name == SUBMIT_OUTPUT_TOOL || call.name == REQUEST_INPUT_TOOL)
    {
        if tool_calls.len() != 1 {
            return Err(AgentError::Failed(
                "OpenAI response mixed internal and external tool calls".to_string(),
            ));
        }
        let call = &tool_calls[index];
        return parse_internal_tool_outcome(
            &call.name,
            &call.arguments.to_string(),
            assistant_message,
            "OpenAI",
            output_schema,
        );
    }

    let assistant_message = filter_tool_turn_assistant_message(assistant_message);
    Ok(AgentTurnOutcome::ToolCalls(AgentToolCallBatch {
        raw_text: assistant_message.clone().unwrap_or_default(),
        assistant_message,
        tool_calls,
    }))
}

pub fn parse_chat_completion_output(
    payload: &Value,
    allow_plain_text_follow_up: bool,
    output_schema: Option<&Value>,
) -> Result<AgentTurnOutcome, AgentError> {
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
            "OpenAI-compatible response did not contain a tool call, plain JSON completion, or follow-up prompt"
                .to_string(),
        ));
    }

    let parsed = tool_calls
        .iter()
        .map(parse_compatible_tool_call)
        .collect::<Result<Vec<_>, AgentError>>()?;

    if let Some(index) = parsed
        .iter()
        .position(|call| call.name == SUBMIT_OUTPUT_TOOL || call.name == REQUEST_INPUT_TOOL)
    {
        if parsed.len() != 1 {
            return Err(AgentError::Failed(
                "OpenAI-compatible response mixed internal and external tool calls".to_string(),
            ));
        }
        let call = &parsed[index];
        return parse_internal_tool_outcome(
            &call.name,
            &call.arguments.to_string(),
            assistant_message,
            "OpenAI-compatible",
            output_schema,
        );
    }

    let assistant_message = filter_tool_turn_assistant_message(assistant_message);
    Ok(AgentTurnOutcome::ToolCalls(AgentToolCallBatch {
        raw_text: assistant_message.clone().unwrap_or_default(),
        assistant_message,
        tool_calls: parsed,
    }))
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    clippy::panic,
    clippy::too_many_lines,
    clippy::unwrap_used,
    reason = "mapping tests are long and use unwrap/expect for brevity"
)]
mod tests {
    use super::*;
    fn make_tool_call_value(name: &str, arguments: &str) -> Value {
        json!({
            "id": "call-7",
            "type": "function",
            "function": {
                "name": name,
                "arguments": arguments
            }
        })
    }
    #[test]
    fn truncated_json_recovered_missing_close_brace() {
        // Missing trailing `}` - most common truncation pattern
        let args = r#"{"path": "/Users/username/projects/some/really/long/path/file.txt"#;
        let call = make_tool_call_value("read", args);
        let result = parse_compatible_tool_call(&call).unwrap();
        assert_eq!(result.name, "read");
        assert_eq!(
            result.arguments,
            json!({"path": "/Users/username/projects/some/really/long/path/file.txt"})
        );
    }

    #[test]
    fn truncated_json_recovered_mid_string() {
        // Value cut mid-string: `fn ` without closing quote
        let args = r#"{"path": "src/main.rs", "pattern": "fn "#;
        let call = make_tool_call_value("search", args);
        let result = parse_compatible_tool_call(&call).unwrap();
        assert_eq!(result.name, "search");
        assert_eq!(
            result.arguments,
            json!({"path": "src/main.rs", "pattern": "fn"})
        );
    }

    #[test]
    fn truncated_json_recovered_no_closing_string_quote() {
        // String value without closing quote + missing brace
        let args = r#"{"path": "/Users/name/project/very/long/file"#;
        let call = make_tool_call_value("read", args);
        let result = parse_compatible_tool_call(&call).unwrap();
        assert_eq!(result.name, "read");
        assert_eq!(
            result.arguments,
            json!({"path": "/Users/name/project/very/long/file"})
        );
    }

    #[test]
    fn truncated_json_recovered_missing_array_close() {
        // Array value without closing bracket
        let args = r#"{"files": ["src/main.rs", "src/lib.rs"#;
        let call = make_tool_call_value("read", args);
        let result = parse_compatible_tool_call(&call).unwrap();
        assert_eq!(result.name, "read");
        assert_eq!(
            result.arguments["files"],
            json!(["src/main.rs", "src/lib.rs"])
        );
    }

    #[test]
    fn invalid_non_truncated_json_still_returns_error() {
        // Non-EOF invalid JSON should still be an error
        let args = "not-json-at-all";
        let call = make_tool_call_value("read", args);
        let result = parse_compatible_tool_call(&call);
        let err = result.unwrap_err().to_string();
        assert!(err.contains("were not valid JSON"), "expected parse error");
    }

    #[test]
    fn empty_args_still_returns_error() {
        // Empty string should still be an error
        let args = "";
        let call = make_tool_call_value("read", args);
        let result = parse_compatible_tool_call(&call);
        let err = result.unwrap_err().to_string();
        assert!(err.contains("were not valid JSON"), "expected parse error");
    }

    #[test]
    fn plain_json_completion_recovers_truncated_object() {
        let content = r#"{"summary": "done without closing brace"#;
        let outcome = parse_plain_json_completion(Some(content)).expect("expected outcome");
        let AgentTurnOutcome::Completed(success) = outcome else {
            panic!("expected completed outcome");
        };
        assert_eq!(
            success.output,
            json!({"summary": "done without closing brace"})
        );
    }

    #[test]
    fn plain_json_completion_recovers_fenced_truncated_object() {
        let content = "```json\n{\"summary\": \"done\"\n```";
        let outcome = parse_plain_json_completion(Some(content)).expect("expected outcome");
        let AgentTurnOutcome::Completed(success) = outcome else {
            panic!("expected completed outcome");
        };
        assert_eq!(success.output, json!({"summary": "done"}));
    }

    #[test]
    fn internal_submit_output_recovers_truncated_arguments() {
        let arguments = r#"{"output": {"summary": "done"}, "assistant_message": null"#;
        let schema = json!({
            "type": "object",
            "properties": { "summary": { "type": "string" } },
            "required": ["summary"]
        });
        let outcome =
            parse_internal_tool_outcome(SUBMIT_OUTPUT_TOOL, arguments, None, "test", Some(&schema))
                .expect("expected outcome");
        let AgentTurnOutcome::Completed(success) = outcome else {
            panic!("expected completed outcome");
        };
        assert_eq!(success.output, json!({"summary": "done"}));
        assert_eq!(success.assistant_message, None);
    }

    #[test]
    fn internal_submit_output_wraps_flat_schema_fields() {
        let schema = json!({
            "type": "object",
            "properties": { "summary": { "type": "string" } },
            "required": ["summary"]
        });
        let outcome = parse_internal_tool_outcome(
            SUBMIT_OUTPUT_TOOL,
            r#"{"summary": "done", "assistant_message": null}"#,
            None,
            "test",
            Some(&schema),
        )
        .expect("expected outcome");
        let AgentTurnOutcome::Completed(success) = outcome else {
            panic!("expected completed outcome");
        };
        assert_eq!(success.output, json!({"summary": "done"}));
    }

    #[test]
    fn internal_submit_output_does_not_wrap_unrelated_top_level_fields() {
        let schema = json!({
            "type": "object",
            "properties": { "summary": { "type": "string" } },
            "required": ["summary"]
        });
        let err = parse_internal_tool_outcome(
            SUBMIT_OUTPUT_TOOL,
            r#"{"path": ".flow/README.md", "assistant_message": null}"#,
            None,
            "test",
            Some(&schema),
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains(MALFORMED_SUBMIT_OUTPUT_MARKER));
    }

    #[test]
    fn submit_output_strips_echoed_tool_call_markup_from_content_fallback() {
        let preamble = "I'll submit the prepared markdown summary.";
        let echoed = format!(
            "{preamble}<tool_call>\n<function=openflow_submit_node_output>\n<parameter=output>{{\"summary\":\"done\"}}</parameter>\n</function>\n</tool_call>"
        );
        let payload = json!({
            "choices": [{
                "message": {
                    "content": echoed,
                    "tool_calls": [make_tool_call_value(
                        SUBMIT_OUTPUT_TOOL,
                        r#"{"output":{"summary":"done"},"assistant_message":null}"#
                    )]
                }
            }]
        });

        let schema = json!({
            "type": "object",
            "properties": { "summary": { "type": "string" } },
            "required": ["summary"]
        });
        let outcome = parse_chat_completion_output(&payload, false, Some(&schema)).unwrap();
        let AgentTurnOutcome::Completed(success) = outcome else {
            panic!("expected completed outcome");
        };
        assert_eq!(success.assistant_message.as_deref(), Some(preamble));
    }

    #[test]
    fn tool_call_batch_strips_redundant_xml_assistant_message() {
        let payload = json!({
            "choices": [{
                "message": {
                    "content": "<tool_call>\n<function=search>\n<parameter=pattern>TODO</parameter>\n</function>\n</tool_call>",
                    "tool_calls": [make_tool_call_value("search", r#"{"pattern":"TODO","paths":"rpo"}"#)]
                }
            }]
        });

        let outcome = parse_chat_completion_output(&payload, false, None).unwrap();
        let AgentTurnOutcome::ToolCalls(batch) = outcome else {
            panic!("expected tool call batch");
        };
        assert_eq!(batch.tool_calls.len(), 1);
        assert_eq!(batch.tool_calls[0].name, "search");
        assert_eq!(batch.assistant_message, None);
    }
}
