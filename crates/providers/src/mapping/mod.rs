use engine::{
    effective_output_schema, filter_tool_turn_assistant_message, AgentError, AgentNeedUserInput,
    AgentRequest, AgentToolCallBatch, AgentTranscriptItem, AgentTurnOutcome, AgentTurnSuccess,
    ToolCall, ToolDefinition,
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
    format!(
        "Node: {}\nTask:\n{}\n\nUpstream input JSON:\n{}",
        request.node_label, request.task_prompt, request.input
    )
}

pub fn should_allow_user_input(request: &AgentRequest) -> bool {
    if !request.allow_user_input {
        return false;
    }
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
                "output": effective_output_schema(&request.output_schema),
                "assistant_message": {
                    "type": "string",
                    "description": "Optional human-facing note to show alongside the final result. Use an empty string when none."
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

/// When models flatten nested object schema fields, group them under the parent property.
fn nest_flat_fields_into_object_properties(
    map: &mut serde_json::Map<String, Value>,
    output_schema: Option<&Value>,
) {
    let Some(properties) = output_schema
        .and_then(|schema| schema.get("properties"))
        .and_then(Value::as_object)
    else {
        return;
    };

    for (prop_name, prop_schema) in properties {
        if map.contains_key(prop_name) {
            continue;
        }
        let Some(nested_props) = prop_schema.get("properties").and_then(Value::as_object) else {
            continue;
        };
        let nested_keys: std::collections::HashSet<_> = nested_props.keys().cloned().collect();
        if nested_keys.is_empty() {
            continue;
        }
        let present: Vec<String> = map
            .keys()
            .filter(|key| nested_keys.contains(*key))
            .cloned()
            .collect();
        if present.is_empty() {
            continue;
        }
        let required_ok = prop_schema
            .get("required")
            .and_then(Value::as_array)
            .is_none_or(|required| {
                required
                    .iter()
                    .filter_map(Value::as_str)
                    .all(|field| map.contains_key(field))
            });
        if !required_ok {
            continue;
        }
        let mut nested = serde_json::Map::new();
        for key in present {
            if let Some(value) = map.remove(&key) {
                nested.insert(key, value);
            }
        }
        map.insert(prop_name.clone(), Value::Object(nested));
    }
}

/// When the model puts prose in `assistant_message` instead of under `output`, map it to a schema field.
fn salvage_assistant_message_into_output(
    assistant_message: &str,
    output_schema: Option<&Value>,
) -> Value {
    let trimmed = assistant_message.trim();
    if let Some(required) = output_schema
        .and_then(|schema| schema.get("required"))
        .and_then(Value::as_array)
        .and_then(|fields| fields.first())
        .and_then(Value::as_str)
    {
        return json!({ required: trimmed });
    }
    if let Some(properties) = output_schema
        .and_then(|schema| schema.get("properties"))
        .and_then(Value::as_object)
    {
        if properties.contains_key("summary") {
            return json!({ "summary": trimmed });
        }
        if let Some(first_key) = properties.keys().next() {
            return json!({ first_key.clone(): trimmed });
        }
    }
    json!({ "content": trimmed })
}

/// When models omit the `output` wrapper, lift top-level schema fields under `output`.
#[must_use]
pub fn normalize_submit_output_arguments(value: Value, output_schema: Option<&Value>) -> Value {
    if let Value::Object(mut outer) = value {
        if let Some(Value::Object(inner)) = outer.get("output").cloned() {
            let mut inner = inner;
            nest_flat_fields_into_object_properties(&mut inner, output_schema);
            outer.insert("output".to_string(), Value::Object(inner));
            return Value::Object(outer);
        }

        let assistant_message = outer.remove("assistant_message");
        if outer.is_empty() {
            if let Some(text) = assistant_message
                .as_ref()
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|text| !text.is_empty())
            {
                return json!({
                    "output": salvage_assistant_message_into_output(text, output_schema),
                    "assistant_message": Value::Null,
                });
            }
            return json!({ "assistant_message": assistant_message });
        }

        nest_flat_fields_into_object_properties(&mut outer, output_schema);

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
            .is_none_or(|keys| !outer.is_empty() && outer.keys().all(|key| keys.contains(key)));
        if !should_wrap {
            if let Some(assistant_message) = assistant_message {
                outer.insert("assistant_message".to_string(), assistant_message);
            }
            return Value::Object(outer);
        }

        return json!({
            "output": Value::Object(outer),
            "assistant_message": assistant_message
        });
    }

    value
}

/// Attach token usage to an outcome, if usage data is available.
pub fn attach_usage(
    outcome: AgentTurnOutcome,
    usage: Option<engine::UsageReport>,
) -> AgentTurnOutcome {
    match usage {
        None => outcome,
        Some(u) => match outcome {
            AgentTurnOutcome::Completed(mut s) => {
                s.usage = Some(u);
                AgentTurnOutcome::Completed(s)
            }
            AgentTurnOutcome::ToolCalls(mut b) => {
                b.usage = Some(u);
                AgentTurnOutcome::ToolCalls(b)
            }
            AgentTurnOutcome::NeedsUserInput(input) => AgentTurnOutcome::NeedsUserInput(input),
        },
    }
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

            let raw = try_parse_or_recover_json(arguments)
                .map_err(|error| AgentError::malformed_submit_output(label, error.to_string()))?;
            let normalized = normalize_submit_output_arguments(raw, output_schema);
            let args: SubmitOutputArgs = serde_json::from_value(normalized)
                .map_err(|error| AgentError::malformed_submit_output(label, error.to_string()))?;
            Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
                output: args.output,
                raw_text: arguments.to_string(),
                assistant_message: filter_tool_turn_assistant_message(
                    args.assistant_message.or(assistant_message),
                ),
                usage: None,
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

/// How to handle a provider turn that collected no tool calls.
pub enum NoToolCallsPolicy {
    /// Fail immediately (e.g. `OpenAI Responses` API).
    Error(&'static str),
    /// Try plain JSON completion, optional follow-up prompt, then fail.
    Recover {
        allow_plain_text_follow_up: bool,
        error: &'static str,
    },
}

pub struct ResolveToolTurnParams<'a> {
    pub tool_calls: Vec<ToolCall>,
    pub assistant_message: Option<String>,
    pub no_tool_calls: NoToolCallsPolicy,
    pub output_schema: Option<&'a Value>,
    pub provider_label: &'a str,
    pub usage: Option<engine::UsageReport>,
    /// When true, `OpenAI` wire formats strip boilerplate from `assistant_message` on external tool batches.
    pub filter_assistant_on_external_batch: bool,
}

/// Shared tail for provider parsers: empty-turn recovery, internal-tool routing, external batch.
pub fn resolve_tool_turn_outcome(
    params: ResolveToolTurnParams<'_>,
) -> Result<AgentTurnOutcome, AgentError> {
    let ResolveToolTurnParams {
        tool_calls,
        assistant_message,
        no_tool_calls,
        output_schema,
        provider_label,
        usage,
        filter_assistant_on_external_batch,
    } = params;

    if tool_calls.is_empty() {
        return match no_tool_calls {
            NoToolCallsPolicy::Error(message) => Err(AgentError::Failed(message.to_string())),
            NoToolCallsPolicy::Recover {
                allow_plain_text_follow_up,
                error,
            } => {
                if let Some(outcome) = parse_plain_json_completion(assistant_message.as_deref()) {
                    return Ok(attach_usage(outcome, usage));
                }
                if allow_plain_text_follow_up {
                    if let Some(assistant_message) = assistant_message {
                        return Ok(AgentTurnOutcome::NeedsUserInput(AgentNeedUserInput {
                            raw_text: assistant_message.clone(),
                            assistant_message,
                        }));
                    }
                }
                Err(AgentError::Failed(error.to_string()))
            }
        };
    }

    if let Some(index) = tool_calls
        .iter()
        .position(|call| call.name == SUBMIT_OUTPUT_TOOL || call.name == REQUEST_INPUT_TOOL)
    {
        if tool_calls.len() != 1 {
            return Err(AgentError::Failed(format!(
                "{provider_label} response mixed internal and external tool calls"
            )));
        }
        let call = &tool_calls[index];
        return parse_internal_tool_outcome(
            &call.name,
            &call.arguments.to_string(),
            assistant_message,
            provider_label,
            output_schema,
        )
        .map(|outcome| attach_usage(outcome, usage.clone()));
    }

    let assistant_message = if filter_assistant_on_external_batch {
        filter_tool_turn_assistant_message(assistant_message)
    } else {
        assistant_message
    };
    Ok(attach_usage(
        AgentTurnOutcome::ToolCalls(AgentToolCallBatch {
            raw_text: assistant_message.clone().unwrap_or_default(),
            assistant_message,
            tool_calls,
            usage: None,
        }),
        usage,
    ))
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
        usage: None,
    }))
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

    fn parse_compatible_tool_call(call: &Value) -> Result<ToolCall, AgentError> {
        let call_id = call
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("call-legacy");
        let function = call.get("function").unwrap_or(call);
        let name = function
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| AgentError::Failed("tool call missing function.name".into()))?;
        let arguments = function
            .get("arguments")
            .and_then(Value::as_str)
            .ok_or_else(|| AgentError::Failed("tool call missing function.arguments".into()))?;
        Ok(ToolCall {
            id: call_id.to_string(),
            name: name.to_string(),
            arguments: try_parse_or_recover_json(arguments).map_err(|error| {
                AgentError::Failed(format!("tool call arguments were not valid JSON: {error}"))
            })?,
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
    fn internal_submit_output_nests_flat_object_schema_fields() {
        let schema = json!({
            "type": "object",
            "properties": {
                "assistantMessage": { "type": "string" },
                "workflowDraft": {
                    "type": "object",
                    "properties": {
                        "name": { "type": "string" },
                        "nodes": { "type": "array" },
                        "edges": { "type": "array" }
                    },
                    "required": ["name", "nodes", "edges"]
                }
            },
            "required": ["assistantMessage", "workflowDraft"]
        });
        let outcome = parse_internal_tool_outcome(
            SUBMIT_OUTPUT_TOOL,
            r#"{"assistantMessage":"Created flow","name":"My Flow","nodes":[],"edges":[],"assistant_message":null}"#,
            None,
            "test",
            Some(&schema),
        )
        .expect("expected outcome");
        let AgentTurnOutcome::Completed(success) = outcome else {
            panic!("expected completed outcome");
        };
        assert_eq!(
            success.output,
            json!({
                "assistantMessage": "Created flow",
                "workflowDraft": {
                    "name": "My Flow",
                    "nodes": [],
                    "edges": []
                }
            })
        );
    }

    #[test]
    fn internal_submit_output_nests_flat_fields_inside_output_wrapper() {
        let schema = json!({
            "type": "object",
            "properties": {
                "assistantMessage": { "type": "string" },
                "workflowDraft": {
                    "type": "object",
                    "properties": {
                        "name": { "type": "string" },
                        "nodes": { "type": "array" },
                        "edges": { "type": "array" }
                    },
                    "required": ["name", "nodes", "edges"]
                }
            },
            "required": ["assistantMessage", "workflowDraft"]
        });
        let outcome = parse_internal_tool_outcome(
            SUBMIT_OUTPUT_TOOL,
            r#"{"output":{"assistantMessage":"Created flow","name":"My Flow","nodes":[],"edges":[]},"assistant_message":null}"#,
            None,
            "test",
            Some(&schema),
        )
        .expect("expected outcome");
        let AgentTurnOutcome::Completed(success) = outcome else {
            panic!("expected completed outcome");
        };
        assert!(success.output.get("workflowDraft").is_some());
    }

    #[test]
    fn internal_submit_output_salvages_assistant_message_only() {
        let schema = json!({
            "type": "object",
            "properties": { "summary": { "type": "string" } },
            "required": ["summary"]
        });
        let outcome = parse_internal_tool_outcome(
            SUBMIT_OUTPUT_TOOL,
            r#"{"assistant_message": "Architecture uses a hexagonal layout with clear ports."}"#,
            None,
            "test",
            Some(&schema),
        )
        .expect("expected outcome");
        let AgentTurnOutcome::Completed(success) = outcome else {
            panic!("expected completed outcome");
        };
        assert_eq!(
            success.output,
            json!({"summary": "Architecture uses a hexagonal layout with clear ports."})
        );
        assert_eq!(success.assistant_message, None);
    }

    #[test]
    fn internal_submit_output_salvages_assistant_message_only_open_object_schema() {
        let schema = json!({ "type": "object" });
        let outcome = parse_internal_tool_outcome(
            SUBMIT_OUTPUT_TOOL,
            r#"{"assistant_message": "Layered services with orchestration at the center."}"#,
            None,
            "test",
            Some(&schema),
        )
        .expect("expected outcome");
        let AgentTurnOutcome::Completed(success) = outcome else {
            panic!("expected completed outcome");
        };
        assert_eq!(
            success.output,
            json!({"content": "Layered services with orchestration at the center."})
        );
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
        .unwrap_err();
        assert!(err.is_malformed_submit_output());
    }

    #[test]
    fn submit_output_strips_echoed_tool_call_markup_from_content_fallback() {
        let preamble = "I'll submit the prepared markdown summary.";
        let echoed = format!(
            "{preamble}<tool_call>\n<function=openflow_submit_node_output>\n<parameter=output>{{\"summary\":\"done\"}}</parameter>\n</function>\n</tool_call>"
        );
        let schema = json!({
            "type": "object",
            "properties": { "summary": { "type": "string" } },
            "required": ["summary"]
        });
        let tool_call = parse_compatible_tool_call(&make_tool_call_value(
            SUBMIT_OUTPUT_TOOL,
            r#"{"output":{"summary":"done"},"assistant_message":null}"#,
        ))
        .unwrap();
        let outcome = resolve_tool_turn_outcome(ResolveToolTurnParams {
            tool_calls: vec![tool_call],
            assistant_message: Some(echoed),
            no_tool_calls: NoToolCallsPolicy::Recover {
                allow_plain_text_follow_up: false,
                error: "unused",
            },
            output_schema: Some(&schema),
            provider_label: "test",
            usage: None,
            filter_assistant_on_external_batch: true,
        })
        .unwrap();
        let AgentTurnOutcome::Completed(success) = outcome else {
            panic!("expected completed outcome");
        };
        assert_eq!(success.assistant_message.as_deref(), Some(preamble));
    }

    #[test]
    fn tool_call_batch_strips_redundant_xml_assistant_message() {
        let tool_call = parse_compatible_tool_call(&make_tool_call_value(
            "search",
            r#"{"pattern":"TODO","paths":"rpo"}"#,
        ))
        .unwrap();
        let outcome = resolve_tool_turn_outcome(ResolveToolTurnParams {
            tool_calls: vec![tool_call],
            assistant_message: Some("<tool_call>\n<function=search>\n<parameter=pattern>TODO</parameter>\n</function>\n</tool_call>".into()),
            no_tool_calls: NoToolCallsPolicy::Recover {
                allow_plain_text_follow_up: false,
                error: "unused",
            },
            output_schema: None,
            provider_label: "test",
            usage: None,
            filter_assistant_on_external_batch: true,
        })
        .unwrap();
        let AgentTurnOutcome::ToolCalls(batch) = outcome else {
            panic!("expected tool call batch");
        };
        assert_eq!(batch.tool_calls.len(), 1);
        assert_eq!(batch.tool_calls[0].name, "search");
        assert_eq!(batch.assistant_message, None);
    }

    #[test]
    fn resolve_tool_turn_rejects_mixed_internal_and_external_calls() {
        let err = resolve_tool_turn_outcome(ResolveToolTurnParams {
            tool_calls: vec![
                ToolCall {
                    id: "1".to_string(),
                    name: SUBMIT_OUTPUT_TOOL.to_string(),
                    arguments: json!({"output": {"x": 1}}),
                },
                ToolCall {
                    id: "2".to_string(),
                    name: "search".to_string(),
                    arguments: json!({"pattern": "x"}),
                },
            ],
            assistant_message: None,
            no_tool_calls: NoToolCallsPolicy::Recover {
                allow_plain_text_follow_up: false,
                error: "unused",
            },
            output_schema: None,
            provider_label: "test",
            usage: None,
            filter_assistant_on_external_batch: false,
        })
        .expect_err("mixed calls should fail");

        assert!(err.to_string().contains("mixed internal and external"));
    }

    #[test]
    fn submit_output_tool_uses_fallback_when_output_schema_null() {
        use engine::{NodeId, WorkflowId};

        let request = AgentRequest {
            workflow_id: WorkflowId::from("wf-1"),
            node_id: NodeId::from("node-1"),
            node_label: "Node".to_string(),
            model: "gpt-5.5".to_string(),
            system_messages: vec!["system".to_string()],
            task_prompt: "task".to_string(),
            input: json!({}),
            output_schema: Value::Null,
            tool_config: engine::NodeToolConfig::default(),
            available_tools: Vec::new(),
            transcript: Vec::new(),
            model_attempt: 1,
            reasoning_effort: None,
            reasoning_budget_tokens: None,
            allow_user_input: true,
        };

        let tool = submit_output_tool(&request);
        let output = &tool.parameters["properties"]["output"];
        assert_eq!(output["type"], "object");
        assert_eq!(output["required"], json!(["summary"]));
    }

    #[test]
    fn workflow_authoring_disallows_request_user_input_tool() {
        use engine::{NodeId, WorkflowId};

        let request = AgentRequest {
            workflow_id: WorkflowId::from("workflow-authoring"),
            node_id: NodeId::from("authoring"),
            node_label: "Workflow authoring".to_string(),
            model: "gpt-5.5".to_string(),
            system_messages: vec!["design workflows".to_string()],
            task_prompt: "Create a draft.".to_string(),
            input: json!({}),
            output_schema: json!({ "type": "object" }),
            tool_config: engine::NodeToolConfig::default(),
            available_tools: Vec::new(),
            transcript: vec![AgentTranscriptItem::UserMessage {
                content: "Build a planner".to_string(),
            }],
            model_attempt: 1,
            reasoning_effort: None,
            reasoning_budget_tokens: None,
            allow_user_input: false,
        };

        assert!(!should_allow_user_input(&request));
        let tool_names: Vec<_> = all_tool_specs(&request)
            .into_iter()
            .map(|tool| tool.name)
            .collect();
        assert_eq!(tool_names, vec![SUBMIT_OUTPUT_TOOL.to_string()]);
    }

    #[test]
    fn should_allow_user_input_false_when_node_disallows() {
        use engine::{NodeId, WorkflowId};

        let mut request = AgentRequest {
            workflow_id: WorkflowId::from("wf-1"),
            node_id: NodeId::from("node-1"),
            node_label: "Node".to_string(),
            model: "gpt-5.5".to_string(),
            system_messages: vec!["system".to_string()],
            task_prompt: "task".to_string(),
            input: json!({}),
            output_schema: json!({ "type": "object" }),
            tool_config: engine::NodeToolConfig::default(),
            available_tools: Vec::new(),
            transcript: vec![AgentTranscriptItem::UserMessage {
                content: "hi".to_string(),
            }],
            model_attempt: 1,
            reasoning_effort: None,
            reasoning_budget_tokens: None,
            allow_user_input: true,
        };
        assert!(should_allow_user_input(&request));

        request.allow_user_input = false;
        assert!(!should_allow_user_input(&request));
    }
}
