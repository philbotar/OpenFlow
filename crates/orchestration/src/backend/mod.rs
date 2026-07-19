use crate::adapters::storage::agent_store::FileAgentStore;
use crate::adapters::storage::app_workflow_store::FileWorkflowStore;
use crate::adapters::storage::project_store::FileProjectStore;
use crate::adapters::storage::project_workflow_store::FileProjectWorkflowStore;
use crate::adapters::storage::run_checkpoint_store::FileRunCheckpointStore;
use crate::adapters::storage::settings_store::FileSettingsStore;
use crate::adapters::storage::skill_store::FileSkillCatalog;
use crate::agent::{AgentLibrary, AgentStore};
use crate::project::ports::ProjectStore;
use crate::project::registry::ProjectRegistry;
use crate::run::coordinator::RunCoordinator;
use crate::run::ports::RunCheckpointStore;
use crate::schedule::ScheduleService;
use crate::settings::facade::SettingsFacade;
use crate::settings::ports::{SettingsStore, SkillCatalog};
use crate::settings::provider::ProviderEnv;
use crate::terminal::TerminalManager;
use crate::workflow::authoring::WorkflowAuthoringService;
use crate::workflow::catalog::WorkflowCatalog;
use crate::workflow::ports::{ProjectWorkflowStore, WorkflowStore};
use std::sync::Arc;

mod agents;
mod authoring;
mod helpers;
mod projects;
mod runs;
mod schedule;
mod settings;
mod terminal;
mod workflow;

pub use crate::api::{
    AgentDefinitionSummary, FileEditPreview, ProjectFileReference, ProviderReadiness,
    ScheduleDraft, ScheduleStatus, ScheduledRunCandidate, WorkflowAuthoringTurnResult,
    WorkflowListItem, WorkflowValidationSummary,
};
pub use crate::error::BackendError;
pub use crate::CodexLoginStatus;

pub struct AppBackendDeps {
    pub workflow_store: Box<dyn WorkflowStore>,
    pub project_workflow_store: Box<dyn ProjectWorkflowStore>,
    pub agent_store: Box<dyn AgentStore>,
    pub project_store: Box<dyn ProjectStore>,
    pub settings_store: Arc<dyn SettingsStore>,
    pub skill_catalog: Box<dyn SkillCatalog>,
    pub env: ProviderEnv,
    pub runtime_handle: tokio::runtime::Handle,
}

pub struct AppBackend {
    pub(super) workflows: WorkflowCatalog,
    pub(super) agents: AgentLibrary,
    pub(super) projects: ProjectRegistry,
    pub(super) settings: SettingsFacade,
    pub(super) runs: RunCoordinator,
    pub(super) run_store: Box<dyn RunCheckpointStore>,
    pub(super) terminal: TerminalManager,
    pub(super) workflow_authoring: WorkflowAuthoringService,
    pub(super) schedule: ScheduleService,
    /// Keeps an owned runtime alive for tests and non-Tauri entrypoints.
    _owned_runtime: Option<tokio::runtime::Runtime>,
}

impl AppBackend {
    #[must_use]
    pub fn new(deps: AppBackendDeps, owned_runtime: Option<tokio::runtime::Runtime>) -> Self {
        Self {
            workflows: WorkflowCatalog::new(deps.workflow_store, deps.project_workflow_store),
            agents: AgentLibrary::new(deps.agent_store),
            projects: ProjectRegistry::new(deps.project_store),
            settings: SettingsFacade::new(deps.settings_store, deps.skill_catalog, deps.env),
            runs: RunCoordinator::new(deps.runtime_handle),
            run_store: Box::new(FileRunCheckpointStore),
            terminal: TerminalManager::new(),
            workflow_authoring: WorkflowAuthoringService::new(),
            schedule: ScheduleService::new(),
            _owned_runtime: owned_runtime,
        }
    }

    #[must_use]
    pub fn default_deps(runtime_handle: tokio::runtime::Handle) -> AppBackendDeps {
        AppBackendDeps {
            workflow_store: Box::new(FileWorkflowStore::new(FileWorkflowStore::default_path())),
            project_workflow_store: Box::new(FileProjectWorkflowStore),
            agent_store: Box::new(FileAgentStore::new(FileAgentStore::default_path())),
            project_store: Box::new(FileProjectStore::new(FileProjectStore::default_path())),
            settings_store: Arc::new(FileSettingsStore::new(FileSettingsStore::default_path())),
            skill_catalog: Box::new(FileSkillCatalog),
            env: ProviderEnv::from_system(),
            runtime_handle,
        }
    }

    #[must_use]
    pub fn with_runtime_handle(runtime_handle: tokio::runtime::Handle) -> Self {
        Self::new(Self::default_deps(runtime_handle), None)
    }

    #[cfg(test)]
    pub(crate) fn block_on_test<F>(&self, future: F) -> F::Output
    where
        F: std::future::Future,
    {
        self._owned_runtime
            .as_ref()
            .expect("test backend must own a runtime")
            .block_on(future)
    }
}

#[cfg(test)]
#[allow(
    clippy::float_cmp,
    reason = "backend tests compare exact layout coordinates"
)]
mod tests;
