use crate::project::ports::Project;
use engine::{FileChangeOp, Workflow};
use serde::{Deserialize, Serialize};

pub use crate::schedule::{ScheduleStatus, ScheduledRunCandidate};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SchedulePreset {
    Timed,
    Interval,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScheduleIntervalUnit {
    Minutes,
    Hours,
    Days,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleDraft {
    pub preset: SchedulePreset,
    pub time: String,
    pub weekdays: Vec<String>,
    pub interval_value: String,
    pub interval_unit: ScheduleIntervalUnit,
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_cron: Option<String>,
}

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
pub struct McpDiscoveryRow {
    pub id: String,
    pub display_name: String,
    pub command: String,
    pub args: Vec<String>,
    pub enabled: bool,
    pub source: String,
    pub source_path: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsLoadPayload {
    pub settings: crate::settings::model::AppSettings,
    pub discovered_mcp: Vec<McpDiscoveryRow>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugLogEntry {
    pub level: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugLogWrite {
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WorkflowAuthoringRole {
    User,
    Assistant,
    /// Internal repair-loop progress surfaced to the UI; excluded from the model transcript.
    Thinking,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowAuthoringMessage {
    pub role: WorkflowAuthoringRole,
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
