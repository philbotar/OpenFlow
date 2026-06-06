//! Outbound ports for desktop dependencies.
//!
//! These traits define the contracts the desktop layer depends on.
//! The adapters module provides concrete implementations backed by `AppBackend`.

use async_trait::async_trait;
use tokio::sync::mpsc::UnboundedReceiver;
use workflow_core::Node;

use app_backend::agent_store::AgentDefinition;
use app_backend::backend::{
    AgentDefinitionSummary, BackendError, ProviderReadiness, WorkflowListItem,
    WorkflowValidationSummary,
};
use app_backend::execution::ExecutionEvent;
use app_backend::settings_store::AppSettings;
use app_backend::state::WorkflowRunState;
use workflow_core::Workflow;

// ── Workflow CRUD ──────────────────────────────────────────────

pub trait WorkflowRepository {
    fn list_workflows(&self) -> Result<Vec<WorkflowListItem>, BackendError>;
    fn load_all_workflows(&self) -> Result<Vec<Workflow>, BackendError>;
    fn load_workflow(&self, workflow_id: &str) -> Result<Workflow, BackendError>;
    fn create_workflow(&self, name: String) -> Result<Workflow, BackendError>;
    fn save_workflow(&self, workflow: Workflow) -> Result<Workflow, BackendError>;
    fn save_workflows(&self, workflows: &[Workflow]) -> Result<(), BackendError>;
    fn rename_workflow(
        &self,
        workflow_id: &str,
        name: String,
    ) -> Result<WorkflowListItem, BackendError>;
    fn validate_workflow(
        &self,
        workflow: &Workflow,
    ) -> Result<WorkflowValidationSummary, BackendError>;
}

// ── Agent CRUD ─────────────────────────────────────────────────

pub trait AgentRepository {
    fn list_agents(&self) -> Result<Vec<AgentDefinitionSummary>, BackendError>;
    fn load_agents(&self) -> Result<Vec<AgentDefinition>, BackendError>;
    fn create_agent_definition(&self, name: String) -> Result<AgentDefinition, BackendError>;
    fn save_agents(&self, agents: &[AgentDefinition]) -> Result<(), BackendError>;
    fn create_agent_node(
        &self,
        index: usize,
        x: f32,
        y: f32,
        agent_id: Option<&str>,
    ) -> Result<Node, BackendError>;
}

// ── Settings ───────────────────────────────────────────────────

pub trait SettingsStore {
    fn load_settings(&self) -> Result<AppSettings, BackendError>;
    fn save_settings(&self, settings: &AppSettings) -> Result<(), BackendError>;
}

// ── Credentials ────────────────────────────────────────────────

pub trait CredentialStore {
    fn load_provider_api_key(&self, provider_id: &str) -> Result<Option<String>, BackendError>;
    fn save_provider_api_key(&self, provider_id: &str, api_key: &str) -> Result<(), BackendError>;
    fn delete_provider_api_key(&self, provider_id: &str) -> Result<(), BackendError>;
}

// ── Provider ───────────────────────────────────────────────────

pub trait ProviderResolver {
    fn resolve_provider_readiness(
        &self,
        settings: &AppSettings,
        transient_api_key: Option<&str>,
    ) -> ProviderReadiness;
}

// ── Run lifecycle ──────────────────────────────────────────────

#[async_trait]
pub trait RunOrchestrator: Send + Sync {
    async fn start_run(
        &self,
        workflow: Workflow,
        entrypoint: Option<String>,
        settings: &AppSettings,
        transient_api_key: Option<&str>,
    ) -> Result<(WorkflowRunState, UnboundedReceiver<ExecutionEvent>), BackendError>;
    async fn apply_execution_event(
        &self,
        event: ExecutionEvent,
    ) -> Result<WorkflowRunState, BackendError>;
    async fn submit_user_input(
        &self,
        node_id: &str,
        text: String,
    ) -> Result<WorkflowRunState, BackendError>;
    async fn submit_tool_approval(
        &self,
        approval_id: &str,
        allow: bool,
    ) -> Result<WorkflowRunState, BackendError>;
    async fn complete_manual_node(&self) -> Result<WorkflowRunState, BackendError>;
    async fn get_run_state(&self) -> Option<WorkflowRunState>;
    async fn clear_run_trace(&self) -> Result<Option<WorkflowRunState>, BackendError>;
}
