use domain::FileChangeOp;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowListItem {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderReadiness {
    pub ready: bool,
    pub provider: String,
    pub message: String,
    pub env_var: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowValidationSummary {
    pub layer_count: usize,
    pub layers: Vec<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentDefinitionSummary {
    pub id: String,
    pub name: String,
    pub model: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileEditPreviewEntry {
    pub path: String,
    pub op: FileChangeOp,
    pub diff: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rename_to: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileEditPreview {
    pub entries: Vec<FileEditPreviewEntry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}
