use serde::{Deserialize, Serialize};
use serde_json::Value;

pub use engine::{McpClientRequestKind, PendingMcpClientRequest};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpClientRequestDecision {
    pub allow: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<Value>,
}
