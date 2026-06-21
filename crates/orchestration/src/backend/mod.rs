use crate::adapters::storage::agent_store::FileAgentStore;
use crate::adapters::storage::app_workflow_store::FileWorkflowStore;
use crate::adapters::storage::incident_store::FileIncidentStore;
use crate::adapters::storage::project_store::FileProjectStore;
use crate::adapters::storage::project_workflow_store::FileProjectWorkflowStore;
use crate::adapters::storage::run_checkpoint_store::FileRunCheckpointStore;
use crate::adapters::storage::settings_store::FileSettingsStore;
use crate::adapters::storage::skill_store::FileSkillCatalog;
use crate::agent::library::AgentLibrary;
use crate::agent::ports::AgentStore;
use crate::incident::{
    IncidentCategory, IncidentContext, IncidentRecord, IncidentRecorder, IncidentSeverity,
};
use crate::project::ports::{Project, ProjectStore};
use crate::project::registry::ProjectRegistry;
use crate::run::coordinator::{RunCoordinator, RunStartParams};
use crate::run::execution::ExecutionEvent;
use crate::run::persistence::RunStoreRoot;
use crate::run::ports::RunCheckpointStore;
use crate::run::state::WorkflowRunState;
use crate::schedule::ScheduleService;
use crate::settings::facade::SettingsFacade;
use crate::settings::model::AppSettings;
use crate::settings::ports::{SettingsStore, SkillCatalog, SkillSummary};
use crate::settings::provider::ProviderEnv;
use crate::terminal::{TerminalEvent, TerminalManager, TerminalStart};
use crate::workflow::authoring::WorkflowAuthoringService;
use crate::workflow::catalog::WorkflowCatalog;
use crate::workflow::ports::{ProjectWorkflowStore, WorkflowStore};
use chrono::{DateTime, Utc};
use engine::{CallableAgent, Node, Workflow};
use std::io;
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedReceiver;

pub use crate::api::{
    AgentDefinitionSummary, FileEditPreview, IncidentSummary, ProjectFileReference,
    ProjectFileReferenceContent, ProviderReadiness, ScheduleStatus, ScheduledRunCandidate,
    WorkflowAuthoringTurnResult, WorkflowListItem, WorkflowValidationSummary,
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
    run_store: Box<dyn RunCheckpointStore>,
    incidents: Arc<IncidentRecorder>,
    terminal: TerminalManager,
    workflow_authoring: WorkflowAuthoringService,
    schedule: ScheduleService,
    /// Keeps an owned runtime alive for tests and non-Tauri entrypoints.
    _owned_runtime: Option<tokio::runtime::Runtime>,
}

impl AppBackend {
    #[must_use]
    pub fn new(deps: AppBackendDeps, owned_runtime: Option<tokio::runtime::Runtime>) -> Self {
        let retention_max = deps
            .settings_store
            .load()
            .map(|settings| settings.incident_retention_max)
            .unwrap_or(500);
        let incidents = Arc::new(IncidentRecorder::with_retention_max(
            Arc::new(FileIncidentStore::new(FileIncidentStore::default_path())),
            retention_max,
        ));
        Self {
            workflows: WorkflowCatalog::new(deps.workflow_store, deps.project_workflow_store),
            agents: AgentLibrary::new(deps.agent_store),
            projects: ProjectRegistry::new(deps.project_store),
            settings: SettingsFacade::new(deps.settings_store, deps.skill_catalog, deps.env),
            runs: RunCoordinator::new(deps.runtime_handle, incidents.clone()),
            run_store: Box::new(FileRunCheckpointStore),
            incidents,
            terminal: TerminalManager::new(),
            workflow_authoring: WorkflowAuthoringService::new(),
            schedule: ScheduleService::new(),
            _owned_runtime: owned_runtime,
        }
    }

    #[must_use]
    pub fn incidents(&self) -> &IncidentRecorder {
        self.incidents.as_ref()
    }

    pub fn backend_err(&self, error: BackendError) -> BackendError {
        let ctx = self.current_incident_context();
        if let Err(io_error) = self.incidents.record_backend(&error, &ctx) {
            log::warn!("failed to persist backend incident: {io_error}");
        }
        error
    }

    pub fn list_incidents(&self, limit: usize) -> io::Result<Vec<IncidentRecord>> {
        self.incidents.list_unresolved(limit)
    }

    pub fn dismiss_incident(&self, id: &str) -> io::Result<()> {
        self.incidents.dismiss(id)
    }

    pub fn clear_resolved_incidents(&self) -> io::Result<usize> {
        self.incidents.clear_resolved()
    }

    pub fn list_incident_summaries(&self, limit: usize) -> io::Result<Vec<IncidentSummary>> {
        self.list_incidents(limit)
            .map(|records| records.into_iter().map(IncidentSummary::from).collect())
    }

    fn current_incident_context(&self) -> IncidentContext {
        self.runs.incident_context()
    }

    fn record_incident(
        &self,
        severity: IncidentSeverity,
        category: IncidentCategory,
        code: &str,
        message: &str,
    ) {
        let ctx = self.current_incident_context();
        if let Err(io_error) = self
            .incidents
            .record_custom(&ctx, severity, category, code, message)
        {
            log::warn!("failed to persist incident: {io_error}");
        }
    }

    fn persistence_err(&self, code: &str, error: BackendError) -> BackendError {
        self.record_incident(
            IncidentSeverity::Error,
            IncidentCategory::Persistence,
            code,
            &error.to_string(),
        );
        error
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
        let workflows = self.workflows.load_all(&self.projects)?;
        let _ = self.schedule.refresh(&workflows, Utc::now());
        Ok(workflows)
    }

    pub fn load_workflow(&self, workflow_id: &str) -> Result<Workflow, BackendError> {
        self.workflows.load_one(&self.projects, workflow_id)
    }

    pub fn create_workflow(&self, name: String) -> Result<Workflow, BackendError> {
        self.workflows.create(name)
    }

    pub fn save_workflow(&self, workflow: Workflow) -> Result<Workflow, BackendError> {
        let saved = self
            .workflows
            .save_one(&self.projects, workflow)
            .map_err(|error| self.persistence_err("persistence.workflow_save", error))?;
        self.refresh_schedules()?;
        Ok(saved)
    }

    pub fn save_workflows(&self, workflows: &[Workflow]) -> Result<(), BackendError> {
        self.workflows
            .save_all(&self.projects, workflows)
            .map_err(|error| self.persistence_err("persistence.workflow_save", error))?;
        self.refresh_schedules()?;
        Ok(())
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
        self.settings
            .save(settings)
            .map_err(|error| self.persistence_err("persistence.settings_save", error))?;
        self.incidents
            .set_retention_max(settings.incident_retention_max);
        Ok(())
    }

    /// Spawn an ephemeral MCP server and return discovered tool names.
    pub async fn probe_mcp_server(
        &self,
        config: crate::settings::model::McpServerConfig,
    ) -> Result<Vec<String>, BackendError> {
        let client = crate::adapters::mcp::McpStdioClient::spawn(&config)
            .await
            .map_err(|error| io::Error::other(error.to_string()))?;
        let names = client
            .list_tool_names()
            .await
            .map_err(|error| io::Error::other(error.to_string()))?;
        Ok(names)
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

    pub fn refresh_bedrock_models(
        &self,
        settings: &AppSettings,
    ) -> Result<Vec<String>, BackendError> {
        self.settings.refresh_bedrock_models(settings)
    }

    pub fn start_workflow_authoring(&self, base_workflow: Option<Workflow>) -> String {
        self.workflow_authoring.start_session(base_workflow)
    }

    pub async fn workflow_authoring_turn(
        &self,
        session_id: String,
        message: String,
        settings: &AppSettings,
        transient_api_key: Option<&str>,
    ) -> Result<WorkflowAuthoringTurnResult, BackendError> {
        let provider_config = crate::settings::provider::resolve_provider_config(
            settings,
            transient_api_key,
            self.settings.env(),
        )?;
        let ai = providers::create_provider(provider_config);
        self.workflow_authoring
            .send_turn(&session_id, message, settings, &ai)
            .await
            .map_err(BackendError::AuthoringFailed)
    }

    pub fn load_projects(&self) -> Result<Vec<Project>, BackendError> {
        self.projects.load()
    }

    pub fn list_projects(&self) -> Result<Vec<Project>, BackendError> {
        self.projects.list()
    }

    pub fn list_project_file_references(
        &self,
        execution_cwd: String,
        query: Option<String>,
        limit: Option<usize>,
    ) -> Result<Vec<ProjectFileReference>, BackendError> {
        crate::project::file_refs::list_project_file_references(
            &execution_cwd,
            query.as_deref(),
            limit,
        )
    }

    pub fn read_project_file_references(
        &self,
        execution_cwd: String,
        paths: Vec<String>,
    ) -> Result<Vec<ProjectFileReferenceContent>, BackendError> {
        crate::project::file_refs::read_project_file_references(&execution_cwd, &paths)
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

    pub fn copy_workflow_to_project(
        &self,
        target_project_id: &str,
        source_workflow_id: &str,
    ) -> Result<crate::api::CopyWorkflowToProjectResult, BackendError> {
        let workflow = self.workflows.copy_to_project(
            &self.projects,
            target_project_id,
            source_workflow_id,
        )?;
        let projects = self.projects.load()?;
        Ok(crate::api::CopyWorkflowToProjectResult { workflow, projects })
    }

    pub fn unassign_workflow_from_project(
        &self,
        project_id: &str,
        workflow_id: &str,
    ) -> Result<Vec<Project>, BackendError> {
        self.workflows
            .unassign_from_project(&self.projects, project_id, workflow_id)
    }

    pub fn delete_workflow(&self, workflow_id: &str) -> Result<Vec<Project>, BackendError> {
        let projects = self
            .workflows
            .delete(&self.projects, workflow_id)
            .map_err(|error| self.persistence_err("persistence.workflow_delete", error))?;
        self.refresh_schedules()?;
        Ok(projects)
    }

    fn run_roots(&self) -> Result<Vec<RunStoreRoot>, BackendError> {
        let mut roots = vec![RunStoreRoot {
            project_id: None,
            root: FileRunCheckpointStore::app_runs_root(),
        }];
        for project in self.projects.load()? {
            roots.push(RunStoreRoot {
                project_id: Some(project.id),
                root: std::path::Path::new(&project.path)
                    .join(".flow")
                    .join("runs"),
            });
        }
        Ok(roots)
    }

    fn run_root_for_workflow(&self, workflow_id: &str) -> Result<RunStoreRoot, BackendError> {
        for project in self.projects.load()? {
            if project.workflow_ids.iter().any(|id| id == workflow_id) {
                return Ok(RunStoreRoot {
                    project_id: Some(project.id),
                    root: std::path::Path::new(&project.path)
                        .join(".flow")
                        .join("runs"),
                });
            }
        }
        Ok(RunStoreRoot {
            project_id: None,
            root: FileRunCheckpointStore::app_runs_root(),
        })
    }

    pub fn list_runs(
        &self,
        workflow_id: Option<&str>,
    ) -> Result<Vec<crate::run::persistence::RunSummary>, BackendError> {
        let roots = self.run_roots()?;
        self.runs
            .list_runs(self.run_store.as_ref(), &roots, workflow_id)
    }

    pub fn replay_run(&self, run_id: &str) -> Result<WorkflowRunState, BackendError> {
        let roots = self.run_roots()?;
        self.runs
            .replay_run(self.run_store.as_ref(), &roots, run_id)
    }

    pub async fn resume_durable_run(
        &self,
        run_id: &str,
        settings: &AppSettings,
        transient_api_key: Option<&str>,
    ) -> Result<(WorkflowRunState, UnboundedReceiver<ExecutionEvent>, String), BackendError> {
        let roots = self.run_roots()?;
        let (root, record) = self
            .run_store
            .load_record(&roots, run_id)?
            .ok_or_else(|| BackendError::RunNotFound(run_id.to_string()))?;
        let workflow_name = record.workflow_name.clone();
        let checkpoint = self
            .run_store
            .load_latest_checkpoint(&root, run_id)?
            .ok_or_else(|| BackendError::RunHasNoCheckpoints(run_id.to_string()))?;
        let workflow = self.load_workflow(&record.workflow_id)?;
        let (state, event_rx) = self
            .runs
            .resume_durable_run(crate::run::coordinator::DurableResumeParams {
                run_id,
                workflow,
                root,
                record,
                checkpoint,
                settings,
                transient_api_key,
                agent_store: self.agents.store(),
                settings_store: self.settings.store(),
                run_store: self.run_store.as_ref(),
                env: self.settings.env(),
            })
            .await
            .map_err(|error| self.backend_err(error))?;
        Ok((state, event_rx, workflow_name))
    }

    pub async fn start_run(
        &self,
        workflow: Workflow,
        entrypoint: Option<String>,
        execution_cwd: Option<String>,
        settings: &AppSettings,
        transient_api_key: Option<&str>,
    ) -> Result<(WorkflowRunState, UnboundedReceiver<ExecutionEvent>), BackendError> {
        let run_root = self.run_root_for_workflow(&workflow.id)?;
        self.runs
            .start_run(RunStartParams {
                workflow,
                entrypoint,
                execution_cwd,
                run_root,
                settings,
                transient_api_key,
                agent_store: self.agents.store(),
                settings_store: self.settings.store(),
                run_store: self.run_store.as_ref(),
                env: self.settings.env(),
            })
            .await
            .map_err(|error| self.backend_err(error))
    }

    /// Stops the active workflow run cooperatively.
    ///
    /// # Errors
    ///
    /// Returns an error when there is no run session to stop.
    pub async fn stop_run(&self) -> Result<WorkflowRunState, BackendError> {
        self.runs.stop_run().await
    }

    pub async fn continue_run(
        &self,
        workflow: Workflow,
        entrypoint: Option<String>,
        settings: &AppSettings,
        transient_api_key: Option<&str>,
    ) -> Result<(WorkflowRunState, UnboundedReceiver<ExecutionEvent>), BackendError> {
        let run_root = self.run_root_for_workflow(&workflow.id)?;
        self.runs
            .continue_run(RunStartParams {
                workflow,
                entrypoint,
                execution_cwd: None,
                run_root,
                settings,
                transient_api_key,
                agent_store: self.agents.store(),
                settings_store: self.settings.store(),
                run_store: self.run_store.as_ref(),
                env: self.settings.env(),
            })
            .await
            .map_err(|error| self.backend_err(error))
    }

    #[must_use]
    pub async fn is_run_continuable(&self) -> bool {
        self.runs.is_run_continuable().await
    }

    pub async fn interrupt_node(&self, node_id: &str) -> Result<WorkflowRunState, BackendError> {
        self.runs.interrupt_node(node_id).await
    }

    pub async fn retry_node(&self, node_id: &str) -> Result<WorkflowRunState, BackendError> {
        self.runs.retry_node(node_id).await
    }

    #[must_use]
    pub async fn is_run_active(&self) -> bool {
        self.runs.is_run_active().await
    }

    pub async fn apply_execution_event(
        &self,
        event: ExecutionEvent,
    ) -> Result<WorkflowRunState, BackendError> {
        self.runs
            .apply_execution_event(event, self.run_store.as_ref())
            .await
    }

    pub async fn submit_user_input(
        &self,
        node_id: &str,
        text: String,
    ) -> Result<WorkflowRunState, BackendError> {
        self.runs
            .submit_user_input(node_id, text)
            .await
            .map_err(|error| self.backend_err(error))
    }

    pub async fn submit_tool_approval(
        &self,
        approval_id: &str,
        allow: bool,
        reason: Option<String>,
    ) -> Result<WorkflowRunState, BackendError> {
        self.runs
            .submit_tool_approval(approval_id, allow, reason)
            .await
            .map_err(|error| self.backend_err(error))
    }

    pub async fn complete_manual_node(&self) -> Result<WorkflowRunState, BackendError> {
        self.runs.complete_manual_node().await
    }

    pub async fn get_run_state(&self) -> Option<WorkflowRunState> {
        self.runs.get_run_state().await
    }

    pub async fn current_run_id(&self) -> Option<String> {
        self.runs.current_run_id().await
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

    pub fn start_terminal(
        &self,
        cwd: Option<&str>,
        cols: u16,
        rows: u16,
    ) -> Result<(TerminalStart, UnboundedReceiver<TerminalEvent>), BackendError> {
        self.terminal.start(cwd, cols, rows).map_err(|message| {
            self.record_incident(
                IncidentSeverity::Error,
                IncidentCategory::Terminal,
                "terminal.start_failed",
                &message,
            );
            BackendError::ProjectOperation(message)
        })
    }

    pub fn write_terminal(&self, session_id: &str, data: &str) -> Result<(), BackendError> {
        self.terminal
            .write(session_id, data)
            .map_err(BackendError::ProjectOperation)
    }

    pub fn resize_terminal(
        &self,
        session_id: &str,
        cols: u16,
        rows: u16,
    ) -> Result<(), BackendError> {
        self.terminal
            .resize(session_id, cols, rows)
            .map_err(BackendError::ProjectOperation)
    }

    pub fn stop_terminal(&self, session_id: &str) -> Result<(), BackendError> {
        self.terminal
            .stop(session_id)
            .map_err(BackendError::ProjectOperation)
    }

    pub fn stop_all_terminals(&self) {
        self.terminal.stop_all();
    }

    pub fn refresh_schedules(&self) -> Result<Vec<ScheduleStatus>, BackendError> {
        self.refresh_schedules_at(Utc::now())
    }

    pub fn refresh_schedules_at(
        &self,
        now: DateTime<Utc>,
    ) -> Result<Vec<ScheduleStatus>, BackendError> {
        let workflows = self.workflows.load_all(&self.projects)?;
        self.schedule
            .refresh(&workflows, now)
            .map_err(BackendError::Schedule)?;
        Ok(self.schedule.statuses())
    }

    pub fn tick_schedules_at(&self, now: DateTime<Utc>) {
        self.schedule.tick_at(now);
    }

    pub fn tick_schedules(&self) {
        self.tick_schedules_at(Utc::now());
    }

    #[must_use]
    pub fn list_schedule_statuses(&self) -> Vec<ScheduleStatus> {
        self.schedule.statuses()
    }

    pub async fn claim_due_scheduled_run(
        &self,
    ) -> Result<Option<ScheduledRunCandidate>, BackendError> {
        self.claim_due_scheduled_run_at(Utc::now()).await
    }

    pub async fn claim_due_scheduled_run_at(
        &self,
        now: DateTime<Utc>,
    ) -> Result<Option<ScheduledRunCandidate>, BackendError> {
        let active = self.is_run_active().await;
        Ok(self.schedule.claim_due_run(now, active))
    }

    fn scheduled_execution_cwd(&self, workflow_id: &str) -> Result<Option<String>, BackendError> {
        let projects = self.projects.load()?;
        let cwd = projects
            .iter()
            .find(|project| project.workflow_ids.iter().any(|id| id == workflow_id))
            .map(|project| {
                let candidate = project.default_execution_cwd.trim();
                if candidate.is_empty() {
                    project.path.clone()
                } else {
                    candidate.to_string()
                }
            });
        Ok(cwd)
    }

    pub async fn start_scheduled_run(
        &self,
        workflow_id: String,
    ) -> Result<(WorkflowRunState, UnboundedReceiver<ExecutionEvent>), BackendError> {
        if self.is_run_active().await {
            return Err(BackendError::Schedule(
                "Skipped because another workflow run was active".to_string(),
            ));
        }

        let workflow = self.load_workflow(&workflow_id)?;
        let execution_cwd = self.scheduled_execution_cwd(&workflow_id)?;
        let settings = self.load_settings()?;
        let run_root = self.run_root_for_workflow(&workflow_id)?;
        self.runs
            .start_run(RunStartParams {
                workflow,
                entrypoint: None,
                execution_cwd,
                run_root,
                settings: &settings,
                transient_api_key: None,
                agent_store: self.agents.store(),
                settings_store: self.settings.store(),
                run_store: self.run_store.as_ref(),
                env: self.settings.env(),
            })
            .await
            .map_err(|error| {
                self.schedule
                    .record_start_error(&workflow_id, error.to_string());
                self.backend_err(error)
            })
    }

    pub async fn start_due_scheduled_run(
        &self,
    ) -> Result<Option<(WorkflowRunState, UnboundedReceiver<ExecutionEvent>, String)>, BackendError>
    {
        let Some(candidate) = self.claim_due_scheduled_run().await? else {
            return Ok(None);
        };
        let workflow_name = self.load_workflow(&candidate.workflow_id)?.name;
        let (state, event_rx) = self.start_scheduled_run(candidate.workflow_id).await?;
        Ok(Some((state, event_rx, workflow_name)))
    }
}

#[cfg(test)]
#[allow(
    clippy::float_cmp,
    reason = "backend tests compare exact layout coordinates"
)]
mod tests;
