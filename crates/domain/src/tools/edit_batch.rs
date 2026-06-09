//! Pre-edit snapshots for reverting agent write batches.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileSnapshot {
    pub path: String,
    pub existed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditBatch {
    pub batch_id: String,
    pub node_id: String,
    pub tool_call_id: String,
    pub tool_name: String,
    pub timestamp_ms: u64,
    pub snapshots: Vec<FileSnapshot>,
}
