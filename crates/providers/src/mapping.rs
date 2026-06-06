use domain::{
    AgentError, AgentNeedUserInput, AgentRequest, AgentToolCallBatch, AgentTranscriptItem,
    AgentTurnOutcome, AgentTurnSuccess, ToolCall, ToolDefinition,
};
use serde::Deserialize;
use serde_json::{json, Value};

pub const SUBMIT_OUTPUT_TOOL: &str = "openflow_submit_node_output";
pub const REQUEST_INPUT_TOOL: &str = "openflow_request_user_input";

#[derive(Debug, Clone)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

pub fn build_node_context(request: &AgentRequest) -> String {
    let tool_instruction = if request.available_tools.is_empty() {
        "When you are ready to finish, call openflow_submit_node_output exactly once.".to_string()
    } else {
        "Use tools when they materially improve correctness. When you are ready to finish, call openflow_submit_node_output exactly once."
            .to_string()
    };
    format!(
        "Node: {}\nTask:\n{}\n\nUpstream input JSON:\n{}\n\n{}",
        request.node_label, request.task_prompt, request.input, tool_instruction
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
        description: "Submit the final structured node output when the task is complete."
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
        description: "Ask the human for one specific missing input before the node can continue."
            .to_string(),
        parameters: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "assistant_message": {
                    "type": "string",
                    "description": "The exact follow-up question or clarification needed from the human."
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
        json!({ "role": "system", "content": request.system_prompt }),
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
        json!({ "role": "system", "content": request.system_prompt }),
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

pub fn parse_internal_tool_outcome(
    tool_name: &str,
    arguments: &str,
    assistant_message: Option<String>,
    label: &str,
) -> Result<AgentTurnOutcome, AgentError> {
    match tool_name {
        SUBMIT_OUTPUT_TOOL => {
            #[derive(Deserialize)]
            struct SubmitOutputArgs {
                output: Value,
                assistant_message: Option<String>,
            }

            let args: SubmitOutputArgs = serde_json::from_str(arguments).map_err(|error| {
                AgentError::Failed(format!(
                    "{label} final output tool arguments were not valid JSON: {error}"
                ))
            })?;
            Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
                output: args.output,
                raw_text: arguments.to_string(),
                assistant_message: args.assistant_message.or(assistant_message),
            }))
        }
        REQUEST_INPUT_TOOL => {
            #[derive(Deserialize)]
            struct RequestInputArgs {
                assistant_message: String,
            }

            let args: RequestInputArgs = serde_json::from_str(arguments).map_err(|error| {
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
    let Ok(output) = serde_json::from_str::<Value>(candidate) else {
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
        arguments: serde_json::from_str(arguments).map_err(|error| {
            AgentError::Failed(format!(
                "OpenAI-compatible tool call arguments were not valid JSON: {error}"
            ))
        })?,
        intent: None,
    })
}

pub fn parse_responses_output(payload: &Value) -> Result<AgentTurnOutcome, AgentError> {
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
                    arguments: serde_json::from_str(arguments).map_err(|error| {
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
        );
    }

    Ok(AgentTurnOutcome::ToolCalls(AgentToolCallBatch {
        raw_text: assistant_message.clone().unwrap_or_default(),
        assistant_message,
        tool_calls,
    }))
}

pub fn parse_chat_completion_output(
    payload: &Value,
    allow_plain_text_follow_up: bool,
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
        );
    }

    Ok(AgentTurnOutcome::ToolCalls(AgentToolCallBatch {
        raw_text: assistant_message.clone().unwrap_or_default(),
        assistant_message,
        tool_calls: parsed,
    }))
}
