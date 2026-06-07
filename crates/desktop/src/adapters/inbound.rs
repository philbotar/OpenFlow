//! Inbound adapters for desktop transport (Tauri commands/events).
//!
//! Concrete implementations of inbound port traits backed by `AppBackend`.
//! These provide the Tauri command interface.

use async_trait::async_trait;
use orchestration::{Node, Project, Workflow};

use orchestration::agent_store::AgentDefinition;
use orchestration::backend::{
    AgentDefinitionSummary, AppBackend, BackendError, ProviderReadiness, WorkflowListItem,
    WorkflowValidationSummary,
};
use orchestration::settings_store::AppSettings;
use orchestration::state::WorkflowRunState;

use crate::ports::inbound::{
    AgentCommands, BootstrapPort, CredentialCommands, ProjectCommands, ProviderCommands,
    RunCommands, SettingsCommands, SkillCommands, WorkflowCommands,
};

// ── Bootstrap ──────────────────────────────────────────────────

#[async_trait]
impl BootstrapPort for AppBackend {
    async fn bootstrap(
        &self,
    ) -> Result<
        (
            Vec<Workflow>,
            Vec<AgentDefinition>,
            Vec<Project>,
            AppSettings,
            Option<WorkflowRunState>,
        ),
        BackendError,
    > {
        let workflows = self.load_all_workflows()?;
        let agents = self.load_agents()?;
        let projects = self.list_projects()?;
        let settings = self.load_settings()?;
        let run_state = self.get_run_state().await;
        Ok((workflows, agents, projects, settings, run_state))
    }
}

// ── Workflow commands ──────────────────────────────────────────

impl WorkflowCommands for AppBackend {
    fn list_workflows(&self) -> Result<Vec<WorkflowListItem>, BackendError> {
        AppBackend::list_workflows(self)
    }

    fn load_all_workflows(&self) -> Result<Vec<Workflow>, BackendError> {
        AppBackend::load_all_workflows(self)
    }

    fn load_workflow(&self, workflow_id: &str) -> Result<Workflow, BackendError> {
        AppBackend::load_workflow(self, workflow_id)
    }

    fn create_workflow(&self, name: String) -> Result<Workflow, BackendError> {
        AppBackend::create_workflow(self, name)
    }

    fn save_workflow(&self, workflow: Workflow) -> Result<Workflow, BackendError> {
        AppBackend::save_workflow(self, workflow)
    }

    fn save_workflows(&self, workflows: Vec<Workflow>) -> Result<(), BackendError> {
        AppBackend::save_workflows(self, &workflows)
    }

    fn rename_workflow(
        &self,
        workflow_id: String,
        name: String,
    ) -> Result<WorkflowListItem, BackendError> {
        AppBackend::rename_workflow(self, &workflow_id, name)
    }

    fn validate_workflow(
        &self,
        workflow: Workflow,
    ) -> Result<WorkflowValidationSummary, BackendError> {
        AppBackend::validate_workflow(self, &workflow)
    }
}

// ── Project commands ───────────────────────────────────────────

impl ProjectCommands for AppBackend {
    fn list_projects(&self) -> Result<Vec<Project>, BackendError> {
        AppBackend::list_projects(self)
    }

    fn save_projects(&self, projects: Vec<Project>) -> Result<(), BackendError> {
        AppBackend::save_projects(self, &projects)
    }

    fn create_project_from_directory(&self, path: String) -> Result<Project, BackendError> {
        AppBackend::create_project_from_directory(self, path)
    }

    fn assign_workflow_to_project(
        &self,
        project_id: String,
        workflow_id: String,
    ) -> Result<Vec<Project>, BackendError> {
        AppBackend::assign_workflow_to_project(self, &project_id, &workflow_id)
    }

    fn unassign_workflow_from_project(
        &self,
        project_id: String,
        workflow_id: String,
    ) -> Result<Vec<Project>, BackendError> {
        AppBackend::unassign_workflow_from_project(self, &project_id, &workflow_id)
    }
}

// ── Agent commands ─────────────────────────────────────────────

impl SkillCommands for AppBackend {
    fn list_skills(&self) -> Result<Vec<orchestration::skill_store::SkillSummary>, BackendError> {
        AppBackend::list_skills(self)
    }
}

impl AgentCommands for AppBackend {
    fn list_agents(&self) -> Result<Vec<AgentDefinitionSummary>, BackendError> {
        AppBackend::list_agents(self)
    }

    fn load_agents(&self) -> Result<Vec<AgentDefinition>, BackendError> {
        AppBackend::load_agents(self)
    }

    fn create_agent_definition(&self, name: String) -> Result<AgentDefinition, BackendError> {
        AppBackend::create_agent_definition(self, name)
    }

    fn save_agents(&self, agents: Vec<AgentDefinition>) -> Result<(), BackendError> {
        AppBackend::save_agents(self, &agents)
    }

    fn create_agent_node(
        &self,
        index: usize,
        x: f32,
        y: f32,
        agent_id: Option<String>,
    ) -> Result<Node, BackendError> {
        AppBackend::create_agent_node(self, index, x, y, agent_id.as_deref())
    }
}

// ── Settings commands ──────────────────────────────────────────

impl SettingsCommands for AppBackend {
    fn load_settings(&self) -> Result<AppSettings, BackendError> {
        AppBackend::load_settings(self)
    }

    fn save_settings(&self, settings: AppSettings) -> Result<(), BackendError> {
        AppBackend::save_settings(self, &settings)
    }
}

// ── Credential commands ────────────────────────────────────────

impl CredentialCommands for AppBackend {
    fn load_provider_api_key(&self, provider_id: String) -> Result<Option<String>, BackendError> {
        AppBackend::load_provider_api_key(self, &provider_id)
    }

    fn save_provider_api_key(
        &self,
        provider_id: String,
        api_key: String,
    ) -> Result<(), BackendError> {
        AppBackend::save_provider_api_key(self, &provider_id, &api_key)
    }

    fn delete_provider_api_key(&self, provider_id: String) -> Result<(), BackendError> {
        AppBackend::delete_provider_api_key(self, &provider_id)
    }
}

// ── Provider commands ──────────────────────────────────────────

impl ProviderCommands for AppBackend {
    fn resolve_provider_readiness(
        &self,
        settings: AppSettings,
        transient_api_key: Option<String>,
    ) -> ProviderReadiness {
        AppBackend::resolve_provider_readiness(self, &settings, transient_api_key.as_deref())
    }
}

// ── Run commands ───────────────────────────────────────────────

#[async_trait]
impl RunCommands for AppBackend {
    async fn start_run(
        &self,
        workflow: Workflow,
        settings: AppSettings,
        execution_cwd: Option<String>,
        transient_api_key: Option<String>,
    ) -> Result<WorkflowRunState, BackendError> {
        let (state, _rx) = AppBackend::start_run(
            self,
            workflow,
            None,
            execution_cwd,
            &settings,
            transient_api_key.as_deref(),
        )
        .await?;
        Ok(state)
    }

    async fn submit_user_input(
        &self,
        node_id: String,
        text: String,
    ) -> Result<WorkflowRunState, BackendError> {
        AppBackend::submit_user_input(self, &node_id, text).await
    }

    async fn submit_tool_approval(
        &self,
        approval_id: String,
        allow: bool,
    ) -> Result<WorkflowRunState, BackendError> {
        AppBackend::submit_tool_approval(self, &approval_id, allow).await
    }

    async fn complete_manual_node(&self) -> Result<WorkflowRunState, BackendError> {
        AppBackend::complete_manual_node(self).await
    }

    async fn get_run_state(&self) -> Option<WorkflowRunState> {
        AppBackend::get_run_state(self).await
    }

    async fn clear_run_trace(&self) -> Result<Option<WorkflowRunState>, BackendError> {
        AppBackend::clear_run_trace(self).await
    }
}
