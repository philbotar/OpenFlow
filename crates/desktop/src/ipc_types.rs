use orchestration::api::McpDiscoveryRow;
use orchestration::backend::{BackendError, ScheduleStatus};
use orchestration::run::state::WorkflowRunState;
use orchestration::{AgentDefinition, AppSettings, Project, SkillSummary, Workflow};
use serde::{Deserialize, Serialize};

/// Bootstrap payload returned on app startup.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BootstrapPayload {
    pub(crate) workflows: Vec<Workflow>,
    pub(crate) agents: Vec<AgentDefinition>,
    pub(crate) projects: Vec<Project>,
    pub(crate) skills: Vec<SkillSummary>,
    pub(crate) settings: AppSettings,
    pub(crate) discovered_mcp: Vec<McpDiscoveryRow>,
    pub(crate) run_state: Option<WorkflowRunState>,
    pub(crate) run_continuable: bool,
    pub(crate) schedule_statuses: Vec<ScheduleStatus>,
}

/// Error type returned to Tauri frontend.
#[derive(Debug, thiserror::Error)]
pub(crate) enum CommandError {
    #[error(transparent)]
    Backend(#[from] BackendError),
}

impl serde::Serialize for CommandError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}
