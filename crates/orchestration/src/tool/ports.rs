//! Outbound ports for tool execution (application layer contracts).

use crate::tool_errors::ToolError;
use serde_json::Value;

/// Regex content search over files under the execution cwd.
pub trait ContentSearch: Send + Sync {
    fn search(&self, args: Value) -> Result<String, ToolError>;
}
