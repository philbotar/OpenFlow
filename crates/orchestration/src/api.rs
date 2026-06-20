use crate::project::ports::Project;
use engine::{FileChangeOp, Workflow};
use serde::{Deserialize, Serialize};

pub use crate::schedule::{ScheduleStatus, ScheduledRunCandidate};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowListItem {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CopyWorkflowToProjectResult {
    pub workflow: Workflow,
    pub projects: Vec<Project>,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProjectFileReferenceKind {
    File,
    Directory,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectFileReference {
    pub path: String,
    pub display_path: String,
    pub kind: ProjectFileReferenceKind,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectFileReferenceContent {
    pub path: String,
    pub kind: ProjectFileReferenceKind,
    pub content: String,
    pub truncated: bool,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IncidentSummary {
    pub id: String,
    pub created_at_ms: u64,
    pub severity: String,
    pub category: String,
    pub code: String,
    pub message: String,
    pub retryable: bool,
    pub resolved: bool,
    pub workflow_id: Option<String>,
    pub run_id: Option<String>,
    pub node_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowAuthoringMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowAuthoringValidation {
    pub valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dag: Option<WorkflowValidationSummary>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowAuthoringTurnResult {
    pub session_id: String,
    pub assistant_message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub draft: Option<Workflow>,
    pub validation: WorkflowAuthoringValidation,
    pub messages: Vec<WorkflowAuthoringMessage>,
}

impl From<crate::incident::IncidentRecord> for IncidentSummary {
    fn from(record: crate::incident::IncidentRecord) -> Self {
        let (workflow_id, run_id, node_id) = match record.scope {
            crate::incident::IncidentScope::Node {
                run_id,
                workflow_id,
                node_id,
            } => (Some(workflow_id), Some(run_id), Some(node_id.0)),
            crate::incident::IncidentScope::Run {
                run_id,
                workflow_id,
            } => (Some(workflow_id), Some(run_id), None),
            _ => (None, None, None),
        };
        Self {
            id: record.id,
            created_at_ms: record.created_at_ms,
            severity: format!("{:?}", record.severity).to_lowercase(),
            category: format!("{:?}", record.category).to_lowercase(),
            code: record.code,
            message: record.message,
            retryable: record.retryable,
            resolved: record.resolved,
            workflow_id,
            run_id,
            node_id,
        }
    }
}
