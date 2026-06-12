//! Shared constructors for error [`ToolResult`] values.

use crate::tools::{ToolCall, ToolResult};

pub(in crate::execution) fn denied_tool_result(
    call: &ToolCall,
    content: impl Into<String>,
) -> ToolResult {
    ToolResult {
        tool_call_id: call.id.clone(),
        tool_name: call.name.clone(),
        content: content.into(),
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
