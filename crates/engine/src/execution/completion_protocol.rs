//! Schema-validating completion protocol for `openflow_submit_node_output`.
//!
//! Providers decode wire bytes into JSON; this module owns envelope normalization,
//! output-schema validation, and repair-candidate construction when recovery fails.

use crate::conversation::filter_tool_turn_assistant_message;
use crate::graph::effective_output_schema;
use crate::ports::{
    AgentError, AgentTurnOutcome, AgentTurnSuccess, OutputRepairCandidate, OutputRepairFailureKind,
    UsageReport,
};
use serde::Deserialize;
use serde_json::{json, Value};

/// Builtin tool that submits final structured node output.
pub const SUBMIT_NODE_OUTPUT_TOOL: &str = "openflow_submit_node_output";

/// Cap raw malformed arguments retained for overseer repair (slice 3+).
pub const OUTPUT_REPAIR_RAW_ARGUMENTS_MAX_BYTES: usize = 64 * 1024;

/// Inputs for converting a decoded submit-output payload into a turn outcome.
pub struct CompleteSubmitOutputParams<'a> {
    pub decoded: Value,
    pub raw_arguments: &'a str,
    pub output_schema: Option<&'a Value>,
    pub assistant_message: Option<String>,
    pub provider_label: &'a str,
    pub tool_call_id: Option<String>,
    pub finish_reason: Option<&'a str>,
    pub usage: Option<UsageReport>,
}

#[derive(Deserialize)]
struct SubmitOutputArgs {
    output: Value,
    assistant_message: Option<String>,
}

/// Normalize, schema-validate, and complete a submit-output call.
///
/// Returns [`AgentTurnOutcome::Completed`] only when the envelope is well-formed
/// and `output` satisfies the effective node schema.
///
/// # Errors
///
/// Returns an [`AgentError`] when the provider response is truncated, malformed,
/// or does not satisfy the configured output schema.
pub fn complete_submit_output(
    params: CompleteSubmitOutputParams<'_>,
) -> Result<AgentTurnOutcome, AgentError> {
    let CompleteSubmitOutputParams {
        decoded,
        raw_arguments,
        output_schema,
        assistant_message,
        provider_label,
        tool_call_id,
        finish_reason,
        usage,
    } = params;

    let schema = output_schema.map_or_else(
        || effective_output_schema(&Value::Null),
        effective_output_schema,
    );

    if is_truncated_finish_reason(finish_reason) {
        return Err(malformed_with_candidate(
            provider_label,
            raw_arguments,
            "response was truncated before the final output tool call completed",
            OutputRepairFailureKind::TruncatedResponse,
            &schema,
            tool_call_id,
            finish_reason,
            usage,
        ));
    }

    let normalized = normalize_submit_output_arguments(decoded, Some(&schema));

    let args: SubmitOutputArgs = match serde_json::from_value(normalized) {
        Ok(args) => args,
        Err(error) => {
            return Err(malformed_with_candidate(
                provider_label,
                raw_arguments,
                error.to_string(),
                OutputRepairFailureKind::WrongEnvelope,
                &schema,
                tool_call_id,
                finish_reason,
                usage,
            ));
        }
    };

    if let Err(detail) = validate_output_against_schema(&args.output, &schema) {
        return Err(malformed_with_candidate(
            provider_label,
            raw_arguments,
            detail,
            OutputRepairFailureKind::SchemaViolation,
            &schema,
            tool_call_id,
            finish_reason,
            usage,
        ));
    }

    Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
        output: args.output,
        raw_text: raw_arguments.to_string(),
        assistant_message: filter_tool_turn_assistant_message(
            args.assistant_message.or(assistant_message),
        ),
        reasoning: Vec::new(),
        usage,
    }))
}

/// Build a malformed-submit error for JSON that could not be decoded.
#[must_use]
pub fn malformed_submit_invalid_json(
    provider_label: impl Into<String>,
    raw_arguments: &str,
    detail: impl Into<String>,
    output_schema: Option<&Value>,
    tool_call_id: Option<String>,
    finish_reason: Option<&str>,
    usage: Option<UsageReport>,
) -> AgentError {
    let schema = output_schema.map_or_else(
        || effective_output_schema(&Value::Null),
        effective_output_schema,
    );
    let kind = if is_truncated_finish_reason(finish_reason) {
        OutputRepairFailureKind::TruncatedResponse
    } else {
        OutputRepairFailureKind::InvalidJson
    };
    malformed_with_candidate(
        provider_label,
        raw_arguments,
        detail,
        kind,
        &schema,
        tool_call_id,
        finish_reason,
        usage,
    )
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

fn validate_output_against_schema(output: &Value, schema: &Value) -> Result<(), String> {
    // ponytail: authored schemas may be incomplete; skip when the meta-schema rejects them.
    // Upgrade: surface a permanent authoring error once workflow validation checks schemas.
    let Ok(validator) = jsonschema::validator_for(schema) else {
        return Ok(());
    };
    match validator.validate(output) {
        Ok(()) => Ok(()),
        Err(error) => Err(sanitize_validation_detail(error.to_string())),
    }
}

fn sanitize_validation_detail(detail: String) -> String {
    const MAX: usize = 512;
    if detail.chars().count() <= MAX {
        return detail;
    }
    let truncated: String = detail.chars().take(MAX).collect();
    format!("{truncated}…")
}

fn is_truncated_finish_reason(finish_reason: Option<&str>) -> bool {
    finish_reason.is_some_and(|reason| reason.eq_ignore_ascii_case("length"))
}

fn cap_raw_arguments(raw: &str) -> String {
    if raw.len() <= OUTPUT_REPAIR_RAW_ARGUMENTS_MAX_BYTES {
        return raw.to_string();
    }
    raw.chars()
        .take(OUTPUT_REPAIR_RAW_ARGUMENTS_MAX_BYTES)
        .collect()
}

#[allow(
    clippy::too_many_arguments,
    reason = "private helper mirrors CompleteSubmitOutputParams fields for one error path"
)]
fn malformed_with_candidate(
    provider_label: impl Into<String>,
    raw_arguments: &str,
    detail: impl Into<String>,
    failure_kind: OutputRepairFailureKind,
    output_schema: &Value,
    tool_call_id: Option<String>,
    finish_reason: Option<&str>,
    usage: Option<UsageReport>,
) -> AgentError {
    let detail = sanitize_validation_detail(detail.into());
    let candidate = OutputRepairCandidate {
        tool_call_id,
        tool_name: SUBMIT_NODE_OUTPUT_TOOL.to_string(),
        raw_arguments: cap_raw_arguments(raw_arguments),
        detail: detail.clone(),
        output_schema: output_schema.clone(),
        failure_kind,
        usage,
        finish_reason: finish_reason.map(str::to_string),
    };
    AgentError::malformed_submit_with_candidate(provider_label, detail, candidate)
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "engine tests use unwrap/expect and panic for concise failure messages"
)]
mod tests {
    use super::*;
    use crate::ports::AgentError;

    fn summary_schema() -> Value {
        json!({
            "type": "object",
            "properties": { "summary": { "type": "string" } },
            "required": ["summary"]
        })
    }

    #[test]
    fn valid_wrapped_result_completes() {
        let schema = summary_schema();
        let raw = r#"{"output":{"summary":"done"},"assistant_message":null}"#;
        let decoded = serde_json::from_str(raw).expect("json");
        let outcome = complete_submit_output(CompleteSubmitOutputParams {
            decoded,
            raw_arguments: raw,
            output_schema: Some(&schema),
            assistant_message: None,
            provider_label: "test",
            tool_call_id: Some("call_1".into()),
            finish_reason: None,
            usage: None,
        })
        .expect("complete");
        let AgentTurnOutcome::Completed(success) = outcome else {
            panic!("expected completed");
        };
        assert_eq!(success.output, json!({"summary": "done"}));
        assert_eq!(success.raw_text, raw);
    }

    #[test]
    fn valid_flat_legacy_result_normalizes() {
        let schema = summary_schema();
        let raw = r#"{"summary":"done","assistant_message":null}"#;
        let decoded = serde_json::from_str(raw).expect("json");
        let outcome = complete_submit_output(CompleteSubmitOutputParams {
            decoded,
            raw_arguments: raw,
            output_schema: Some(&schema),
            assistant_message: None,
            provider_label: "test",
            tool_call_id: None,
            finish_reason: None,
            usage: None,
        })
        .expect("complete");
        let AgentTurnOutcome::Completed(success) = outcome else {
            panic!("expected completed");
        };
        assert_eq!(success.output, json!({"summary": "done"}));
    }

    #[test]
    fn schema_invalid_result_returns_schema_violation_candidate() {
        let schema = summary_schema();
        let secret = "SECRET_SENTINEL_raw_args_must_not_leak";
        let raw =
            format!(r#"{{"output":{{"summary":123,"leak":"{secret}"}},"assistant_message":null}}"#);
        let decoded = serde_json::from_str(&raw).expect("json");
        let err = complete_submit_output(CompleteSubmitOutputParams {
            decoded,
            raw_arguments: &raw,
            output_schema: Some(&schema),
            assistant_message: None,
            provider_label: "test",
            tool_call_id: Some("call_x".into()),
            finish_reason: None,
            usage: None,
        })
        .expect_err("schema violation");
        assert!(err.is_malformed_submit_output());
        let candidate = err.output_repair_candidate().expect("candidate");
        assert_eq!(
            candidate.failure_kind,
            OutputRepairFailureKind::SchemaViolation
        );
        assert!(candidate.is_repairable());
        assert!(candidate.raw_arguments().contains(secret));
        let debug = format!("{candidate:?}");
        let display = err.to_string();
        assert!(
            !debug.contains(secret),
            "Debug must redact raw args, got {debug}"
        );
        assert!(
            !display.contains(secret),
            "Display must omit raw args, got {display}"
        );
        assert!(debug.contains("raw_arguments_len"));
    }

    #[test]
    fn finish_reason_length_is_truncated_and_not_repairable() {
        let schema = summary_schema();
        let secret = "SECRET_TRUNCATED_ARGS";
        let raw = format!(r#"{{"output":{{"summary":"{secret}""#);
        let err = malformed_submit_invalid_json(
            "test",
            &raw,
            "EOF while parsing",
            Some(&schema),
            None,
            Some("length"),
            None,
        );
        let candidate = err.output_repair_candidate().expect("candidate");
        assert_eq!(
            candidate.failure_kind,
            OutputRepairFailureKind::TruncatedResponse
        );
        assert!(!candidate.is_repairable());
        let debug = format!("{candidate:?}");
        let display = err.to_string();
        assert!(!debug.contains(secret));
        assert!(!display.contains(secret));
    }

    #[test]
    fn wrong_envelope_is_typed_and_redacted() {
        let schema = summary_schema();
        let secret = "SECRET_ENVELOPE";
        let raw =
            format!(r#"{{"path":".flow/README.md","assistant_message":null,"x":"{secret}"}}"#);
        let decoded = serde_json::from_str(&raw).expect("json");
        let err = complete_submit_output(CompleteSubmitOutputParams {
            decoded,
            raw_arguments: &raw,
            output_schema: Some(&schema),
            assistant_message: None,
            provider_label: "test",
            tool_call_id: None,
            finish_reason: None,
            usage: None,
        })
        .expect_err("wrong envelope");
        let candidate = err.output_repair_candidate().expect("candidate");
        assert_eq!(
            candidate.failure_kind,
            OutputRepairFailureKind::WrongEnvelope
        );
        assert!(candidate.is_repairable());
        assert!(!format!("{candidate:?}").contains(secret));
        assert!(!err.to_string().contains(secret));
    }

    #[test]
    fn agent_error_without_candidate_still_classifies() {
        let err = AgentError::malformed_submit_output("AI provider", "missing field `output`");
        assert!(err.is_malformed_submit_output());
        assert!(err.output_repair_candidate().is_none());
        assert!(!err.is_repairable_submit_output());
    }
}
