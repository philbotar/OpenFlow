use engine::{
    AgentContinueWork, AgentError, AgentMessageTurn, AgentNeedUserInput, AgentReasoning,
    AgentRequest, AgentToolCallBatch, AgentTurnOutcome, AgentTurnPhase, AgentTurnSuccess,
    CompleteSubmitOutputParams, OUTPUT_REPAIR_RAW_ARGUMENTS_MAX_BYTES, SUBMIT_NODE_OUTPUT_TOOL,
    ToolCall, ToolDefinition, complete_submit_output, effective_output_schema,
    filter_tool_turn_assistant_message, malformed_submit_invalid_json,
};
use serde::Deserialize;
use serde_json::{Value, json};

pub const SUBMIT_OUTPUT_TOOL: &str = SUBMIT_NODE_OUTPUT_TOOL;
pub const REQUEST_INPUT_TOOL: &str = "openflow_request_user_input";
pub const CONTINUE_WORK_TOOL: &str = "openflow_continue_work";
#[derive(Debug, Clone)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

pub fn build_node_context(request: &AgentRequest) -> String {
    let input =
        serde_json::to_string_pretty(&request.input).unwrap_or_else(|_| request.input.to_string());
    format!(
        "Node: {}\nTask:\n{}\n\nUpstream input JSON:\n{}",
        request.node_label, request.task_prompt, input
    )
}

pub const fn should_allow_user_input(request: &AgentRequest) -> bool {
    request.allow_user_input
}

pub fn submit_output_tool(request: &AgentRequest) -> ToolSpec {
    let mut output_schema = effective_output_schema(&request.output_schema);
    annotate_large_string_file_references(&mut output_schema);
    ToolSpec {
        name: SUBMIT_OUTPUT_TOOL.to_string(),
        description: "Submit the final structured node output when the task is complete. Required shape: {\"output\": {...schema fields...}, \"assistant_message\": null|string}. Schema fields must be nested under \"output\", not at the top level. If a large string value was already written to a repository-relative file, submit its path instead of copying the file contents."
            .to_string(),
        parameters: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "output": output_schema,
                "assistant_message": {
                    "type": "string",
                    "description": "Optional human-facing note to show alongside the final result. Use an empty string when none."
                }
            },
            "required": ["output", "assistant_message"]
        }),
    }
}

fn annotate_large_string_file_references(schema: &mut Value) {
    let Some(object) = schema.as_object_mut() else {
        return;
    };
    if object.get("type").and_then(Value::as_str) == Some("string") {
        let note = "For large content already written to a file, use the repository-relative file path instead of duplicating the contents.";
        let description = object
            .get("description")
            .and_then(Value::as_str)
            .map_or_else(|| note.to_string(), |existing| format!("{existing} {note}"));
        object.insert("description".to_string(), Value::String(description));
    }
    for key in ["properties", "$defs", "definitions"] {
        if let Some(children) = object.get_mut(key).and_then(Value::as_object_mut) {
            for child in children.values_mut() {
                annotate_large_string_file_references(child);
            }
        }
    }
    for key in ["items", "additionalProperties"] {
        if let Some(child) = object.get_mut(key) {
            annotate_large_string_file_references(child);
        }
    }
    for key in ["allOf", "anyOf", "oneOf"] {
        if let Some(children) = object.get_mut(key).and_then(Value::as_array_mut) {
            for child in children {
                annotate_large_string_file_references(child);
            }
        }
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

pub fn continue_work_tool() -> ToolSpec {
    ToolSpec {
        name: CONTINUE_WORK_TOOL.to_string(),
        description: "Switch to a work turn that exposes executable tools. Call this only when more tool-backed work is required; after the tool batch finishes, OpenFlow returns to a control turn."
            .to_string(),
        parameters: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {}
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
    match request.turn_phase {
        AgentTurnPhase::Control => {
            let mut tools = vec![submit_output_tool(request)];
            if should_allow_user_input(request) {
                tools.push(request_input_tool());
            }
            if !request.available_tools.is_empty() {
                tools.push(continue_work_tool());
            }
            tools
        }
        AgentTurnPhase::Work => request
            .available_tools
            .iter()
            .map(external_tool_spec)
            .collect(),
    }
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
            AgentTurnOutcome::ContinueWork(mut continuation) => {
                continuation.usage = Some(u);
                AgentTurnOutcome::ContinueWork(continuation)
            }
            AgentTurnOutcome::Message(mut message) => {
                message.usage = Some(u);
                AgentTurnOutcome::Message(message)
            }
        },
    }
}

pub fn parse_internal_tool_outcome(
    tool_name: &str,
    arguments: &str,
    assistant_message: Option<String>,
    label: &str,
    output_schema: Option<&Value>,
    reasoning: Vec<AgentReasoning>,
) -> Result<AgentTurnOutcome, AgentError> {
    match tool_name {
        SUBMIT_OUTPUT_TOOL => {
            let raw = try_parse_or_recover_json(arguments).map_err(|error| {
                malformed_submit_invalid_json(
                    label,
                    arguments,
                    error.to_string(),
                    output_schema,
                    None,
                    None,
                    None,
                )
            })?;
            complete_submit_output(CompleteSubmitOutputParams {
                decoded: raw,
                raw_arguments: arguments,
                output_schema,
                assistant_message,
                provider_label: label,
                tool_call_id: None,
                finish_reason: None,
                usage: None,
            })
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
                reasoning,
            }))
        }
        CONTINUE_WORK_TOOL => {
            let _: serde_json::Map<String, Value> = try_deserialize_or_recover_json(arguments)
                .map_err(|error| {
                    AgentError::Failed(format!(
                        "{label} continue-work tool arguments were not valid JSON: {error}"
                    ))
                })?;
            Ok(AgentTurnOutcome::ContinueWork(AgentContinueWork {
                raw_text: arguments.to_string(),
                assistant_message: filter_tool_turn_assistant_message(assistant_message),
                reasoning,
                usage: None,
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
    /// Try plain JSON completion, then fail.
    Recover { error: &'static str },
}

pub struct ResolveToolTurnParams<'a> {
    pub tool_calls: Vec<ToolCall>,
    pub assistant_message: Option<String>,
    pub reasoning: Vec<AgentReasoning>,
    pub turn_phase: AgentTurnPhase,
    pub no_tool_calls: NoToolCallsPolicy,
    pub output_schema: Option<&'a Value>,
    pub provider_label: &'a str,
    pub usage: Option<engine::UsageReport>,
    /// When true, `OpenAI` wire formats strip boilerplate from `assistant_message` on external tool batches.
    pub filter_assistant_on_external_batch: bool,
}

/// Shared tail for provider parsers: empty-turn recovery, internal-tool routing, external batch.
#[allow(clippy::too_many_lines)] // shared tool-turn tail; split would scatter one control-flow path
pub fn resolve_tool_turn_outcome(
    params: ResolveToolTurnParams<'_>,
) -> Result<AgentTurnOutcome, AgentError> {
    let ResolveToolTurnParams {
        tool_calls,
        assistant_message,
        reasoning,
        turn_phase,
        no_tool_calls,
        output_schema,
        provider_label,
        usage,
        filter_assistant_on_external_batch,
    } = params;

    if tool_calls.is_empty() {
        let error = match no_tool_calls {
            NoToolCallsPolicy::Error(message) => message,
            NoToolCallsPolicy::Recover { error } => {
                if turn_phase == AgentTurnPhase::Control {
                    if let Some(outcome) = parse_plain_json_completion(assistant_message.as_deref())
                    {
                        return Ok(attach_usage(outcome, usage));
                    }
                }
                error
            }
        };
        if let Some(message) = assistant_message.filter(|message| !message.trim().is_empty()) {
            return Ok(attach_usage(
                AgentTurnOutcome::Message(AgentMessageTurn {
                    raw_text: message.clone(),
                    assistant_message: message,
                    reasoning,
                    usage: None,
                }),
                usage,
            ));
        }
        return Err(AgentError::Failed(error.to_string()));
    }

    let is_control_tool = |name: &str| {
        matches!(
            name,
            SUBMIT_OUTPUT_TOOL | REQUEST_INPUT_TOOL | CONTINUE_WORK_TOOL
        )
    };
    let control_count = tool_calls
        .iter()
        .filter(|call| is_control_tool(&call.name))
        .count();
    let call_names = || {
        tool_calls
            .iter()
            .map(|call| call.name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    };

    match turn_phase {
        AgentTurnPhase::Control => {
            if control_count != tool_calls.len() {
                return Err(AgentError::mixed_tool_turn(provider_label, call_names()));
            }
            if tool_calls.len() != 1 {
                // ponytail: same MixedToolTurn retry as wrong-phase / mixed batches
                return Err(AgentError::mixed_tool_turn(provider_label, call_names()));
            }
            let call = &tool_calls[0];
            if let Some(marker) = extract_malformed_tool_args_marker(&call.arguments) {
                return Err(error_from_malformed_tool_args_marker(
                    provider_label,
                    &call.name,
                    &call.id,
                    marker,
                    output_schema,
                    usage,
                ));
            }
            return parse_internal_tool_outcome(
                &call.name,
                &call.arguments.to_string(),
                assistant_message,
                provider_label,
                output_schema,
                reasoning,
            )
            .map(|outcome| attach_usage(outcome, usage));
        }
        AgentTurnPhase::Work if control_count != 0 => {
            // ponytail: reuse MixedToolTurn retry path; engine picks phase-aware feedback
            return Err(AgentError::mixed_tool_turn(provider_label, call_names()));
        }
        AgentTurnPhase::Work => {}
    }

    for call in &tool_calls {
        if let Some(marker) = extract_malformed_tool_args_marker(&call.arguments) {
            return Err(error_from_malformed_tool_args_marker(
                provider_label,
                &call.name,
                &call.id,
                marker,
                output_schema,
                usage,
            ));
        }
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
            reasoning,
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

/// Private wire marker so Rig can deserialize while mapping retains the raw candidate.
#[allow(clippy::redundant_pub_crate)] // crate-private module; keep pub(crate) for intentional crate API
pub(crate) const MALFORMED_TOOL_ARGS_MARKER_KEY: &str = "__openflow_malformed_tool_args_v1";

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::redundant_pub_crate)] // crate-private module; keep pub(crate) for intentional crate API
pub(crate) struct MalformedToolArgsMarker {
    pub raw: String,
    pub detail: String,
}

/// Parse or deterministically repair tool-argument JSON. Used by the `OpenAI` HTTP normalizer.
#[allow(clippy::redundant_pub_crate)] // crate-private module; keep pub(crate) for intentional crate API
pub(crate) fn parse_or_recover_tool_arguments(input: &str) -> Result<Value, String> {
    try_parse_or_recover_json(input).map_err(|error| error.to_string())
}

/// Encode an unrecoverable argument string as a marker object (valid JSON for Rig).
#[must_use]
#[allow(clippy::redundant_pub_crate)] // crate-private module; keep pub(crate) for intentional crate API
pub(crate) fn malformed_tool_args_marker_value(raw: &str, detail: &str) -> Value {
    let capped = if raw.len() <= OUTPUT_REPAIR_RAW_ARGUMENTS_MAX_BYTES {
        raw.to_string()
    } else {
        raw.chars()
            .take(OUTPUT_REPAIR_RAW_ARGUMENTS_MAX_BYTES)
            .collect()
    };
    json!({
        MALFORMED_TOOL_ARGS_MARKER_KEY: {
            "raw": capped,
            "detail": detail,
        }
    })
}

/// Pull a private malformed-args marker out of a Rig-parsed tool arguments value.
#[must_use]
#[allow(clippy::redundant_pub_crate)] // crate-private module; keep pub(crate) for intentional crate API
pub(crate) fn extract_malformed_tool_args_marker(value: &Value) -> Option<MalformedToolArgsMarker> {
    let marker = value.get(MALFORMED_TOOL_ARGS_MARKER_KEY)?.as_object()?;
    let raw = marker.get("raw")?.as_str()?.to_string();
    let detail = marker.get("detail")?.as_str()?.to_string();
    Some(MalformedToolArgsMarker { raw, detail })
}

fn error_from_malformed_tool_args_marker(
    provider_label: &str,
    tool_name: &str,
    tool_call_id: &str,
    marker: MalformedToolArgsMarker,
    output_schema: Option<&Value>,
    usage: Option<engine::UsageReport>,
) -> AgentError {
    if tool_name == SUBMIT_OUTPUT_TOOL {
        return malformed_submit_invalid_json(
            provider_label,
            &marker.raw,
            marker.detail,
            output_schema,
            Some(tool_call_id.to_string()),
            None,
            usage,
        );
    }
    AgentError::Failed(format!(
        "{provider_label} tool `{tool_name}` arguments were not valid JSON: {}",
        marker.detail
    ))
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
    use engine::AgentTranscriptItem;
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
        let outcome = parse_internal_tool_outcome(
            SUBMIT_OUTPUT_TOOL,
            arguments,
            None,
            "test",
            Some(&schema),
            Vec::new(),
        )
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
            Vec::new(),
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
            Vec::new(),
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
            Vec::new(),
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
            Vec::new(),
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
            Vec::new(),
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
            Vec::new(),
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
            reasoning: vec![],
            turn_phase: AgentTurnPhase::Control,
            no_tool_calls: NoToolCallsPolicy::Recover { error: "unused" },
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
            reasoning: vec![],
            turn_phase: AgentTurnPhase::Work,
            no_tool_calls: NoToolCallsPolicy::Recover { error: "unused" },
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
    fn control_turn_rejects_hallucinated_executable_tool_calls() {
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
            reasoning: vec![],
            turn_phase: AgentTurnPhase::Control,
            no_tool_calls: NoToolCallsPolicy::Recover { error: "unused" },
            output_schema: None,
            provider_label: "test",
            usage: None,
            filter_assistant_on_external_batch: false,
        })
        .expect_err("executable calls are invalid during a control turn");

        assert!(err.to_string().contains("tools from the wrong turn phase"));
        assert!(err.is_mixed_tool_turn());
        assert_eq!(
            err.mixed_tool_names(),
            Some("openflow_submit_node_output, search")
        );
    }

    #[test]
    fn control_turn_rejects_multiple_control_tool_calls_as_mixed() {
        let err = resolve_tool_turn_outcome(ResolveToolTurnParams {
            tool_calls: vec![
                ToolCall {
                    id: "1".to_string(),
                    name: CONTINUE_WORK_TOOL.to_string(),
                    arguments: json!({}),
                },
                ToolCall {
                    id: "2".to_string(),
                    name: SUBMIT_OUTPUT_TOOL.to_string(),
                    arguments: json!({"output": {"x": 1}}),
                },
            ],
            assistant_message: None,
            reasoning: vec![],
            turn_phase: AgentTurnPhase::Control,
            no_tool_calls: NoToolCallsPolicy::Recover { error: "unused" },
            output_schema: None,
            provider_label: "test",
            usage: None,
            filter_assistant_on_external_batch: false,
        })
        .expect_err("multiple control calls are invalid");

        assert!(err.is_mixed_tool_turn());
        assert_eq!(
            err.mixed_tool_names(),
            Some("openflow_continue_work, openflow_submit_node_output")
        );
    }

    #[test]
    fn work_turn_rejects_hallucinated_control_tool_calls() {
        let err = resolve_tool_turn_outcome(ResolveToolTurnParams {
            tool_calls: vec![ToolCall {
                id: "1".to_string(),
                name: CONTINUE_WORK_TOOL.to_string(),
                arguments: json!({}),
            }],
            assistant_message: None,
            reasoning: vec![],
            turn_phase: AgentTurnPhase::Work,
            no_tool_calls: NoToolCallsPolicy::Recover { error: "unused" },
            output_schema: None,
            provider_label: "test",
            usage: None,
            filter_assistant_on_external_batch: false,
        })
        .expect_err("control calls are invalid during a work turn");

        assert!(err.is_mixed_tool_turn());
        assert!(err.to_string().contains("tools from the wrong turn phase"));
        assert_eq!(err.mixed_tool_names(), Some("openflow_continue_work"));
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
            turn_phase: AgentTurnPhase::Control,
            tool_access_policy: engine::ToolAccessPolicy::Execution,
            allow_user_input: true,
        };

        let tool = submit_output_tool(&request);
        let output = &tool.parameters["properties"]["output"];
        assert_eq!(output["type"], "object");
        assert_eq!(output["required"], json!(["summary"]));
    }

    #[test]
    fn submit_output_tool_allows_large_strings_to_stay_file_backed() {
        use engine::{NodeId, WorkflowId};

        let request = AgentRequest {
            workflow_id: WorkflowId::from("wf-1"),
            node_id: NodeId::from("node-1"),
            node_label: "Node".to_string(),
            model: "minimax-m3".to_string(),
            system_messages: vec!["system".to_string()],
            task_prompt: "write a specification".to_string(),
            input: json!({}),
            output_schema: json!({
                "type": "object",
                "properties": {
                    "implementation_spec_markdown": { "type": "string" }
                },
                "required": ["implementation_spec_markdown"]
            }),
            tool_config: engine::NodeToolConfig::default(),
            available_tools: Vec::new(),
            transcript: Vec::new(),
            model_attempt: 1,
            reasoning_effort: None,
            reasoning_budget_tokens: None,
            turn_phase: AgentTurnPhase::Control,
            tool_access_policy: engine::ToolAccessPolicy::Execution,
            allow_user_input: false,
        };

        let tool = submit_output_tool(&request);
        let field =
            &tool.parameters["properties"]["output"]["properties"]["implementation_spec_markdown"];
        assert_eq!(field["type"], "string");
        assert!(
            field["description"]
                .as_str()
                .is_some_and(|description| description.contains("repository-relative file path"))
        );
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
            turn_phase: AgentTurnPhase::Control,
            tool_access_policy: engine::ToolAccessPolicy::Execution,
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
    fn control_and_work_turn_catalogs_are_disjoint() {
        use engine::{NodeId, ToolConcurrency, ToolTier, WorkflowId};

        let mut request = AgentRequest {
            workflow_id: WorkflowId::from("wf-1"),
            node_id: NodeId::from("node-1"),
            node_label: "Node".to_string(),
            model: "model".to_string(),
            system_messages: vec!["system".to_string()],
            task_prompt: "task".to_string(),
            input: json!({}),
            output_schema: json!({ "type": "object" }),
            tool_config: engine::NodeToolConfig::default(),
            available_tools: vec![ToolDefinition {
                name: "search".to_string(),
                description: "Search".to_string(),
                input_schema: json!({ "type": "object" }),
                tier: ToolTier::Read,
                concurrency: ToolConcurrency::Shared,
            }],
            transcript: Vec::new(),
            model_attempt: 1,
            reasoning_effort: None,
            reasoning_budget_tokens: None,
            turn_phase: AgentTurnPhase::Control,
            tool_access_policy: engine::ToolAccessPolicy::Execution,
            allow_user_input: true,
        };

        let control_names = all_tool_specs(&request)
            .into_iter()
            .map(|tool| tool.name)
            .collect::<Vec<_>>();
        assert!(control_names.iter().any(|name| name == SUBMIT_OUTPUT_TOOL));
        assert!(control_names.iter().any(|name| name == REQUEST_INPUT_TOOL));
        assert!(control_names.iter().any(|name| name == CONTINUE_WORK_TOOL));
        assert!(!control_names.iter().any(|name| name == "search"));

        request.turn_phase = AgentTurnPhase::Work;
        let work_names = all_tool_specs(&request)
            .into_iter()
            .map(|tool| tool.name)
            .collect::<Vec<_>>();
        assert_eq!(work_names, vec!["search".to_string()]);
    }

    #[test]
    fn request_user_input_tool_is_available_on_first_turn() {
        use engine::{NodeId, WorkflowId};

        let request = AgentRequest {
            workflow_id: WorkflowId::from("wf-1"),
            node_id: NodeId::from("grill"),
            node_label: "Grill".to_string(),
            model: "gpt-5.5".to_string(),
            system_messages: vec!["Ask before assuming.".to_string()],
            task_prompt: "Create a feature brief.".to_string(),
            input: json!({"request": "Create a Supabase backend"}),
            output_schema: json!({ "type": "object" }),
            tool_config: engine::NodeToolConfig::default(),
            available_tools: Vec::new(),
            transcript: Vec::new(),
            model_attempt: 1,
            reasoning_effort: None,
            reasoning_budget_tokens: None,
            turn_phase: AgentTurnPhase::Control,
            tool_access_policy: engine::ToolAccessPolicy::Execution,
            allow_user_input: true,
        };

        let tool_names: Vec<_> = all_tool_specs(&request)
            .into_iter()
            .map(|tool| tool.name)
            .collect();

        assert!(tool_names.iter().any(|name| name == REQUEST_INPUT_TOOL));
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
            turn_phase: AgentTurnPhase::Control,
            tool_access_policy: engine::ToolAccessPolicy::Execution,
            allow_user_input: true,
        };
        assert!(should_allow_user_input(&request));

        request.allow_user_input = false;
        assert!(!should_allow_user_input(&request));
    }

    #[test]
    fn build_node_context_pretty_prints_upstream_input_json() {
        use engine::{NodeId, WorkflowId};

        let request = AgentRequest {
            workflow_id: WorkflowId::from("wf-1"),
            node_id: NodeId::from("node-1"),
            node_label: "Review".to_string(),
            model: "gpt-5.5".to_string(),
            system_messages: vec![],
            task_prompt: "Review upstream".to_string(),
            input: json!({"upstream": [{"nodeId": "a", "output": {"ok": true}}]}),
            output_schema: Value::Null,
            tool_config: engine::NodeToolConfig::default(),
            available_tools: Vec::new(),
            transcript: Vec::new(),
            model_attempt: 1,
            reasoning_effort: None,
            reasoning_budget_tokens: None,
            turn_phase: AgentTurnPhase::Control,
            tool_access_policy: engine::ToolAccessPolicy::Execution,
            allow_user_input: true,
        };

        let context = build_node_context(&request);
        assert!(context.contains("Upstream input JSON:\n{\n"));
        assert!(context.contains("\"ok\": true"));
        assert!(!context.contains("Upstream input JSON:\n{\"upstream\""));
    }
}
