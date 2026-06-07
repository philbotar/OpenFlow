//! Outbound adapters used by desktop runtime.
//!
//! Concrete implementations of outbound port traits backed by `AppBackend`.

use async_trait::async_trait;
use tokio::sync::mpsc::UnboundedReceiver;

use orchestration::agent_store::AgentDefinition;
use orchestration::backend::{
    AgentDefinitionSummary, AppBackend, BackendError, ProviderReadiness, WorkflowListItem,
    WorkflowValidationSummary,
};
use orchestration::execution::ExecutionEvent;
use orchestration::settings_store::AppSettings;
use orchestration::state::WorkflowRunState;
use orchestration::{Node, Workflow};

use crate::ports::outbound::{
    AgentRepository, CredentialStore, ProviderResolver, RunOrchestrator, SettingsStore,
    WorkflowRepository,
};

// ── Workflow CRUD ──────────────────────────────────────────────

impl WorkflowRepository for AppBackend {
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

    fn save_workflows(&self, workflows: &[Workflow]) -> Result<(), BackendError> {
        AppBackend::save_workflows(self, workflows)
    }

    fn rename_workflow(
        &self,
        workflow_id: &str,
        name: String,
    ) -> Result<WorkflowListItem, BackendError> {
        AppBackend::rename_workflow(self, workflow_id, name)
    }

    fn validate_workflow(
        &self,
        workflow: &Workflow,
    ) -> Result<WorkflowValidationSummary, BackendError> {
        AppBackend::validate_workflow(self, workflow)
    }
}

// ── Agent CRUD ─────────────────────────────────────────────────

impl AgentRepository for AppBackend {
    fn list_agents(&self) -> Result<Vec<AgentDefinitionSummary>, BackendError> {
        AppBackend::list_agents(self)
    }

    fn load_agents(&self) -> Result<Vec<AgentDefinition>, BackendError> {
        AppBackend::load_agents(self)
    }

    fn create_agent_definition(&self, name: String) -> Result<AgentDefinition, BackendError> {
        AppBackend::create_agent_definition(self, name)
    }

    fn save_agents(&self, agents: &[AgentDefinition]) -> Result<(), BackendError> {
        AppBackend::save_agents(self, agents)
    }

    fn create_agent_node(
        &self,
        index: usize,
        x: f32,
        y: f32,
        agent_id: Option<&str>,
    ) -> Result<Node, BackendError> {
        AppBackend::create_agent_node(self, index, x, y, agent_id)
    }
}

// ── Settings ───────────────────────────────────────────────────

impl SettingsStore for AppBackend {
    fn load_settings(&self) -> Result<AppSettings, BackendError> {
        AppBackend::load_settings(self)
    }

    fn save_settings(&self, settings: &AppSettings) -> Result<(), BackendError> {
        AppBackend::save_settings(self, settings)
    }
}

// ── Credentials ────────────────────────────────────────────────

impl CredentialStore for AppBackend {
    fn load_provider_api_key(&self, provider_id: &str) -> Result<Option<String>, BackendError> {
        AppBackend::load_provider_api_key(self, provider_id)
    }

    fn save_provider_api_key(&self, provider_id: &str, api_key: &str) -> Result<(), BackendError> {
        AppBackend::save_provider_api_key(self, provider_id, api_key)
    }

    fn delete_provider_api_key(&self, provider_id: &str) -> Result<(), BackendError> {
        AppBackend::delete_provider_api_key(self, provider_id)
    }
}

// ── Provider ───────────────────────────────────────────────────

impl ProviderResolver for AppBackend {
    fn resolve_provider_readiness(
        &self,
        settings: &AppSettings,
        transient_api_key: Option<&str>,
    ) -> ProviderReadiness {
        AppBackend::resolve_provider_readiness(self, settings, transient_api_key)
    }
}

// ── Run lifecycle ──────────────────────────────────────────────

#[async_trait]
impl RunOrchestrator for AppBackend {
    async fn start_run(
        &self,
        workflow: Workflow,
        entrypoint: Option<String>,
        settings: &AppSettings,
        transient_api_key: Option<&str>,
    ) -> Result<(WorkflowRunState, UnboundedReceiver<ExecutionEvent>), BackendError> {
        AppBackend::start_run(self, workflow, entrypoint, settings, transient_api_key).await
    }

    async fn apply_execution_event(
        &self,
        event: ExecutionEvent,
    ) -> Result<WorkflowRunState, BackendError> {
        AppBackend::apply_execution_event(self, event).await
    }

    async fn submit_user_input(
        &self,
        node_id: &str,
        text: String,
    ) -> Result<WorkflowRunState, BackendError> {
        AppBackend::submit_user_input(self, node_id, text).await
    }

    async fn submit_tool_approval(
        &self,
        approval_id: &str,
        allow: bool,
    ) -> Result<WorkflowRunState, BackendError> {
        AppBackend::submit_tool_approval(self, approval_id, allow).await
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
