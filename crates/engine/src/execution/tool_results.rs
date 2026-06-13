//! Shared constructors for error [`ToolResult`] values.

use crate::tools::{ToolCall, ToolResult};
use std::fmt::Write;

pub(in crate::execution) fn format_tool_denial_message(reason: Option<&str>) -> String {
    let mut message = "Tool call denied by the user.".to_string();
    if let Some(reason) = reason.map(str::trim).filter(|value| !value.is_empty()) {
        let _ = write!(message, " Reason: {reason}.");
    }
    message.push_str(" Do not retry this exact call; adjust your approach or ask the user.");
    message
}

pub(in crate::execution) fn denied_tool_result(
    call: &ToolCall,
    reason: Option<&str>,
) -> ToolResult {
    ToolResult {
        tool_call_id: call.id.clone(),
        tool_name: call.name.clone(),
        content: format_tool_denial_message(reason),
        is_error: true,
        artifact_ids: Vec::new(),
        output_meta: None,
    }
}

pub(in crate::execution) fn error_tool_result(
    call: &ToolCall,
    message: impl Into<String>,
) -> ToolResult {
    ToolResult {
        tool_call_id: call.id.clone(),
        tool_name: call.name.clone(),
        content: serde_json::json!({ "error": message.into() }).to_string(),
        is_error: true,
        artifact_ids: Vec::new(),
        output_meta: None,
    }
}
