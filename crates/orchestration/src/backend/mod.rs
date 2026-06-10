use crate::adapters::storage::agent_store::FileAgentStore;
use crate::adapters::storage::app_workflow_store::FileWorkflowStore;
use crate::adapters::storage::project_store::FileProjectStore;
use crate::adapters::storage::project_workflow_store::FileProjectWorkflowStore;
use crate::adapters::storage::settings_store::FileSettingsStore;
use crate::adapters::storage::skill_store::FileSkillCatalog;
use crate::agent::library::AgentLibrary;
use crate::agent::ports::AgentStore;
use crate::project::ports::{Project, ProjectStore};
use crate::project::registry::ProjectRegistry;
use crate::run::coordinator::{RunCoordinator, RunStartParams};
use crate::run::execution::ExecutionEvent;
use crate::run::state::WorkflowRunState;
use crate::settings::facade::SettingsFacade;
use crate::settings::model::AppSettings;
use crate::settings::ports::{SettingsStore, SkillCatalog, SkillSummary};
use crate::settings::provider::ProviderEnv;
use crate::workflow::catalog::WorkflowCatalog;
use crate::workflow::ports::{ProjectWorkflowStore, WorkflowStore};
use engine::{CallableAgent, Node, Workflow};
use tokio::sync::mpsc::UnboundedReceiver;

pub use crate::api::{
    AgentDefinitionSummary, FileEditPreview, ProviderReadiness, WorkflowListItem,
    WorkflowValidationSummary,
};
pub use crate::error::BackendError;

pub struct AppBackendDeps {
    pub workflow_store: Box<dyn WorkflowStore>,
    pub project_workflow_store: Box<dyn ProjectWorkflowStore>,
    pub agent_store: Box<dyn AgentStore>,
    pub project_store: Box<dyn ProjectStore>,
    pub settings_store: Box<dyn SettingsStore>,
    pub skill_catalog: Box<dyn SkillCatalog>,
    pub env: ProviderEnv,
    pub runtime_handle: tokio::runtime::Handle,
}

pub struct AppBackend {
    workflows: WorkflowCatalog,
    agents: AgentLibrary,
    projects: ProjectRegistry,
    settings: SettingsFacade,
    runs: RunCoordinator,
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
            settings_store: Box::new(FileSettingsStore::new(FileSettingsStore::default_path())),
            skill_catalog: Box::new(FileSkillCatalog),
            env: ProviderEnv::from_system(),
            runtime_handle,
        }
    }

    #[must_use]
    pub fn with_runtime_handle(runtime_handle: tokio::runtime::Handle) -> Self {
        Self::new(Self::default_deps(runtime_handle), None)
    }

    #[must_use]
    pub fn with_default_paths() -> Self {
        let runtime = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
        let handle = runtime.handle().clone();
        Self::new(Self::default_deps(handle), Some(runtime))
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

    pub fn list_workflows(&self) -> Result<Vec<WorkflowListItem>, BackendError> {
        self.workflows.list(&self.projects)
    }

    pub fn load_all_workflows(&self) -> Result<Vec<Workflow>, BackendError> {
        self.workflows.load_all(&self.projects)
    }

    pub fn load_workflow(&self, workflow_id: &str) -> Result<Workflow, BackendError> {
        self.workflows.load_one(&self.projects, workflow_id)
    }

    pub fn create_workflow(&self, name: String) -> Result<Workflow, BackendError> {
        self.workflows.create(name)
    }

    pub fn save_workflow(&self, workflow: Workflow) -> Result<Workflow, BackendError> {
        self.workflows.save_one(&self.projects, workflow)
    }

    pub fn save_workflows(&self, workflows: &[Workflow]) -> Result<(), BackendError> {
        self.workflows.save_all(&self.projects, workflows)
    }

    pub fn rename_workflow(
        &self,
        workflow_id: &str,
        name: String,
    ) -> Result<WorkflowListItem, BackendError> {
        self.workflows.rename(&self.projects, workflow_id, name)
    }

    pub fn load_agents(&self) -> Result<Vec<CallableAgent>, BackendError> {
        self.agents.load()
    }

    pub fn save_agents(&self, agents: &[CallableAgent]) -> Result<(), BackendError> {
        self.agents.save(agents)
    }

    pub fn create_agent_definition(&self, name: String) -> Result<CallableAgent, BackendError> {
        self.agents.create(name)
    }

    pub fn create_agent_node(
        &self,
        index: usize,
        x: f32,
        y: f32,
        agent_id: Option<&str>,
    ) -> Result<Node, BackendError> {
        self.agents.create_node(index, x, y, agent_id)
    }

    pub fn list_agents(&self) -> Result<Vec<AgentDefinitionSummary>, BackendError> {
        self.agents.list()
    }

    pub fn list_skills(&self) -> Result<Vec<SkillSummary>, BackendError> {
        self.settings.list_skills()
    }

    pub fn load_settings(&self) -> Result<AppSettings, BackendError> {
        self.settings.load()
    }

    pub fn save_settings(&self, settings: &AppSettings) -> Result<(), BackendError> {
        self.settings.save(settings)
    }

    pub fn load_provider_api_key(&self, provider_id: &str) -> Result<Option<String>, BackendError> {
        self.settings.load_provider_api_key(provider_id)
    }

    pub fn save_provider_api_key(
        &self,
        provider_id: &str,
        api_key: &str,
    ) -> Result<(), BackendError> {
        self.settings.save_provider_api_key(provider_id, api_key)
    }

    pub fn delete_provider_api_key(&self, provider_id: &str) -> Result<(), BackendError> {
        self.settings.delete_provider_api_key(provider_id)
    }

    #[must_use]
    pub fn resolve_provider_readiness(
        &self,
        settings: &AppSettings,
        transient_api_key: Option<&str>,
    ) -> ProviderReadiness {
        self.settings
            .resolve_provider_readiness(settings, transient_api_key)
    }

    pub fn validate_workflow(
        &self,
        workflow: &Workflow,
    ) -> Result<WorkflowValidationSummary, BackendError> {
        self.settings.validate_workflow(workflow)
    }

    pub fn load_projects(&self) -> Result<Vec<Project>, BackendError> {
        self.projects.load()
    }

    pub fn list_projects(&self) -> Result<Vec<Project>, BackendError> {
        self.projects.list()
    }

    pub fn save_projects(&self, projects: &[Project]) -> Result<(), BackendError> {
        self.projects.save(projects)
    }

    pub fn create_project_from_directory(&self, path: String) -> Result<Project, BackendError> {
        self.projects.create_from_directory(path)
    }

    pub fn assign_workflow_to_project(
        &self,
        project_id: &str,
        workflow_id: &str,
    ) -> Result<Vec<Project>, BackendError> {
        self.workflows
            .assign_to_project(&self.projects, project_id, workflow_id)
    }

    pub fn unassign_workflow_from_project(
        &self,
        project_id: &str,
        workflow_id: &str,
    ) -> Result<Vec<Project>, BackendError> {
        self.workflows
            .unassign_from_project(&self.projects, project_id, workflow_id)
    }

    pub async fn start_run(
        &self,
        workflow: Workflow,
        entrypoint: Option<String>,
        execution_cwd: Option<String>,
        settings: &AppSettings,
        transient_api_key: Option<&str>,
    ) -> Result<(WorkflowRunState, UnboundedReceiver<ExecutionEvent>), BackendError> {
        self.runs
            .start_run(RunStartParams {
                workflow,
                entrypoint,
                execution_cwd,
                settings,
                transient_api_key,
                agent_store: self.agents.store(),
                settings_store: self.settings.store(),
                env: self.settings.env(),
            })
            .await
    }

    /// Stops the active workflow run cooperatively.
    ///
    /// # Errors
    ///
    /// Returns an error when there is no run session to stop.
    pub async fn stop_run(&self) -> Result<WorkflowRunState, BackendError> {
        self.runs.stop_run().await
    }

    #[must_use]
    pub async fn is_run_active(&self) -> bool {
        self.runs.is_run_active().await
    }

    pub async fn apply_execution_event(
        &self,
        event: ExecutionEvent,
    ) -> Result<WorkflowRunState, BackendError> {
        self.runs.apply_execution_event(event).await
    }

    pub async fn submit_user_input(
        &self,
        node_id: &str,
        text: String,
    ) -> Result<WorkflowRunState, BackendError> {
        self.runs.submit_user_input(node_id, text).await
    }

    pub async fn submit_tool_approval(
        &self,
        approval_id: &str,
        allow: bool,
    ) -> Result<WorkflowRunState, BackendError> {
        self.runs.submit_tool_approval(approval_id, allow).await
    }

    pub async fn complete_manual_node(&self) -> Result<WorkflowRunState, BackendError> {
        self.runs.complete_manual_node().await
    }

    pub async fn get_run_state(&self) -> Option<WorkflowRunState> {
        self.runs.get_run_state().await
    }

    pub async fn preview_file_edit(
        &self,
        approval_id: &str,
        tool_name: String,
        arguments: serde_json::Value,
    ) -> Result<FileEditPreview, BackendError> {
        self.runs
            .preview_file_edit(approval_id, tool_name, arguments)
            .await
    }

    pub async fn git_diff_file(&self, path: String) -> Result<String, BackendError> {
        self.runs.git_diff_file(path).await
    }

    pub async fn revert_edit_batch(
        &self,
        batch_id: String,
    ) -> Result<WorkflowRunState, BackendError> {
        self.runs.revert_edit_batch(batch_id).await
    }

    pub async fn clear_run_trace(&self) -> Result<Option<WorkflowRunState>, BackendError> {
        self.runs.clear_run_trace().await
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests;
