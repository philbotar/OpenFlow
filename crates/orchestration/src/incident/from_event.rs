use crate::incident::recorder::{
    build_record, context_from_incident, scope_from_context, NewIncidentRecord,
};
use crate::incident::{IncidentCategory, IncidentContext, IncidentRecord, IncidentSeverity};
use engine::{NodeId, RunTelemetry};
use serde_json::json;

const MALFORMED_SUBMIT_OUTPUT_MARKER: &str = "final output tool arguments were not valid JSON";

pub fn incident_from_execution_event(
    event: &RunTelemetry,
    ctx: &IncidentContext,
) -> Option<IncidentRecord> {
    match event {
        RunTelemetry::ToolCompleted {
            node_id,
            tool_call_id,
            tool_name,
            content,
            is_error,
            ..
        } => {
            if !is_error {
                return None;
            }
            let merged = ctx_with_node(ctx, node_id, None);
            let code = parse_tool_code(content);
            let retryable = tool_content_retryable(content, &code);
            let mut context = context_from_incident(&merged);
            context.insert("toolCallId".to_string(), json!(tool_call_id));
            context.insert("toolName".to_string(), json!(tool_name));
            Some(build_record(NewIncidentRecord {
                scope: scope_from_context(&merged),
                severity: IncidentSeverity::Error,
                category: IncidentCategory::Tool,
                code,
                message: content.clone(),
                hint: parse_tool_hint(content),
                retryable,
                context,
            }))
        }
        RunTelemetry::ToolDenied {
            node_id,
            tool_call_id,
            tool_name,
            reason,
            ..
        } => {
            let merged = ctx_with_node(ctx, node_id, None);
            let mut context = context_from_incident(&merged);
            context.insert("toolCallId".to_string(), json!(tool_call_id));
            context.insert("toolName".to_string(), json!(tool_name));
            Some(build_record(NewIncidentRecord {
                scope: scope_from_context(&merged),
                severity: IncidentSeverity::Warning,
                category: IncidentCategory::Tool,
                code: "tool.denied".to_string(),
                message: reason.clone(),
                hint: None,
                retryable: false,
                context,
            }))
        }
        RunTelemetry::AiInvokeFailed {
            node_id,
            label,
            error,
        } => {
            let merged = ctx_with_node(ctx, node_id, Some(label));
            let (code, severity, retryable, hint) = classify_ai_invoke_failure(error);
            Some(build_record(NewIncidentRecord {
                scope: scope_from_context(&merged),
                severity,
                category: IncidentCategory::AiInvoke,
                code,
                message: error.clone(),
                hint,
                retryable,
                context: context_from_incident(&merged),
            }))
        }
        RunTelemetry::NodeErrored {
            node_id,
            label,
            error,
        } => {
            if is_malformed_submit_output_message(error) {
                return None;
            }
            let merged = ctx_with_node(ctx, node_id, Some(label));
            Some(build_record(NewIncidentRecord {
                scope: scope_from_context(&merged),
                severity: IncidentSeverity::Error,
                category: IncidentCategory::Node,
                code: "node.errored".to_string(),
                message: error.clone(),
                hint: None,
                retryable: true,
                context: context_from_incident(&merged),
            }))
        }
        RunTelemetry::NodeFailed {
            node_id,
            label,
            error,
        } => {
            if is_malformed_submit_output_message(error) {
                return None;
            }
            let merged = ctx_with_node(ctx, node_id, Some(label));
            Some(build_record(NewIncidentRecord {
                scope: scope_from_context(&merged),
                severity: IncidentSeverity::Fatal,
                category: IncidentCategory::Node,
                code: "node.failed".to_string(),
                message: error.clone(),
                hint: None,
                retryable: false,
                context: context_from_incident(&merged),
            }))
        }
        RunTelemetry::SubagentFailed {
            node_id,
            subagent_id,
            error,
        } => {
            let merged = ctx_with_node(ctx, node_id, None);
            let mut context = context_from_incident(&merged);
            context.insert("subagentId".to_string(), json!(subagent_id));
            Some(build_record(NewIncidentRecord {
                scope: scope_from_context(&merged),
                severity: IncidentSeverity::Error,
                category: IncidentCategory::Subagent,
                code: "subagent.failed".to_string(),
                message: error.clone(),
                hint: None,
                retryable: false,
                context,
            }))
        }
        RunTelemetry::Error(message) => Some(build_record(NewIncidentRecord {
            scope: scope_from_context(ctx),
            severity: IncidentSeverity::Fatal,
            category: IncidentCategory::Run,
            code: "run.error".to_string(),
            message: message.clone(),
            hint: None,
            retryable: false,
            context: context_from_incident(ctx),
        })),
        _ => None,
    }
}

fn is_malformed_submit_output_message(error: &str) -> bool {
    error.contains(MALFORMED_SUBMIT_OUTPUT_MARKER)
}

fn classify_ai_invoke_failure(error: &str) -> (String, IncidentSeverity, bool, Option<String>) {
    if is_malformed_submit_output_message(error) {
        return (
            "ai.malformed_submit_output".to_string(),
            IncidentSeverity::Error,
            true,
            Some(
                "Call openflow_submit_node_output with \
                 {\"output\": {...schema fields...}, \"assistant_message\": null}."
                    .to_string(),
            ),
        );
    }
    if error.starts_with("transient:") {
        return (
            "ai.transient".to_string(),
            IncidentSeverity::Error,
            true,
            None,
        );
    }
    if error.starts_with("permanent:") {
        return (
            "ai.permanent".to_string(),
            IncidentSeverity::Error,
            false,
            None,
        );
    }
    if error.contains("interrupted") {
        return (
            "ai.interrupted".to_string(),
            IncidentSeverity::Warning,
            false,
            None,
        );
    }
    (
        "ai.failed".to_string(),
        IncidentSeverity::Error,
        false,
        None,
    )
}

fn ctx_with_node(ctx: &IncidentContext, node_id: &NodeId, label: Option<&str>) -> IncidentContext {
    let mut merged = ctx.clone();
    merged.node_id = Some(node_id.clone());
    if label.is_some() && merged.node_label.is_none() {
        merged.node_label = label.map(str::to_string);
    }
    merged
}

fn parse_tool_code(content: &str) -> String {
    if let Some(tag) = extract_bracket_tag(content) {
        format!("tool.{tag}")
    } else {
        "tool.failed".to_string()
    }
}

fn extract_bracket_tag(content: &str) -> Option<&str> {
    let trimmed = content.trim_start();
    if !trimmed.starts_with('[') {
        return None;
    }
    let end = trimmed[1..].find(']')?;
    Some(&trimmed[1..1 + end])
}

fn parse_tool_hint(content: &str) -> Option<String> {
    if let Some(tag_end) = content.find(']') {
        let rest = content[tag_end + 1..].trim_start();
        if let Some(pos) = rest.find('—') {
            let hint = rest[pos + '—'.len_utf8()..].trim();
            if !hint.is_empty() {
                return Some(hint.to_string());
            }
        }
    }
    None
}

fn tool_content_retryable(content: &str, code: &str) -> bool {
    if code == "tool.timeout" {
        return true;
    }
    let lower = content.to_lowercase();
    lower.contains("timeout")
        || lower.contains("timed out")
        || lower.contains("connection reset")
        || lower.contains("connection refused")
        || lower.contains("temporarily unavailable")
        || lower.contains("503")
        || lower.contains("502")
        || lower.contains("429")
}
