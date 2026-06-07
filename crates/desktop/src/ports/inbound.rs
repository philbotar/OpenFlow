//! Inbound ports for desktop-facing use-cases.
//!
//! These traits define the contracts the desktop layer exposes to its consumers.
//! The adapters module provides the concrete Tauri command implementations.

use async_trait::async_trait;
use orchestration::Workflow;

use orchestration::agent_store::AgentDefinition;
use orchestration::backend::{
    AgentDefinitionSummary, BackendError, ProviderReadiness, WorkflowListItem,
    WorkflowValidationSummary,
};
use orchestration::settings_store::AppSettings;
use orchestration::skill_store::SkillSummary;
use orchestration::state::WorkflowRunState;

// ── Bootstrap ──────────────────────────────────────────────────

#[async_trait]
pub trait BootstrapPort: Send + Sync {
    async fn bootstrap(
        &self,
    ) -> Result<
        (
            Vec<Workflow>,
            Vec<AgentDefinition>,
            AppSettings,
            Option<WorkflowRunState>,
        ),
        BackendError,
    >;
}

// ── Workflow commands ──────────────────────────────────────────

pub trait WorkflowCommands {
    fn list_workflows(&self) -> Result<Vec<WorkflowListItem>, BackendError>;
    fn load_all_workflows(&self) -> Result<Vec<Workflow>, BackendError>;
    fn load_workflow(&self, workflow_id: &str) -> Result<Workflow, BackendError>;
    fn create_workflow(&self, name: String) -> Result<Workflow, BackendError>;
    fn save_workflow(&self, workflow: Workflow) -> Result<Workflow, BackendError>;
    fn save_workflows(&self, workflows: Vec<Workflow>) -> Result<(), BackendError>;
    fn rename_workflow(
        &self,
        workflow_id: String,
        name: String,
    ) -> Result<WorkflowListItem, BackendError>;
    fn validate_workflow(
        &self,
        workflow: Workflow,
    ) -> Result<WorkflowValidationSummary, BackendError>;
}

// ── Agent commands ─────────────────────────────────────────────

pub trait AgentCommands {
    fn list_agents(&self) -> Result<Vec<AgentDefinitionSummary>, BackendError>;
    fn load_agents(&self) -> Result<Vec<AgentDefinition>, BackendError>;
    fn create_agent_definition(&self, name: String) -> Result<AgentDefinition, BackendError>;
    fn save_agents(&self, agents: Vec<AgentDefinition>) -> Result<(), BackendError>;
    fn create_agent_node(
        &self,
        index: usize,
        x: f32,
        y: f32,
        agent_id: Option<String>,
    ) -> Result<orchestration::Node, BackendError>;
}

// ── Skill commands ─────────────────────────────────────────────

pub trait SkillCommands {
    fn list_skills(&self) -> Result<Vec<SkillSummary>, BackendError>;
}

// ── Settings commands ──────────────────────────────────────────

pub trait SettingsCommands {
    fn load_settings(&self) -> Result<AppSettings, BackendError>;
    fn save_settings(&self, settings: AppSettings) -> Result<(), BackendError>;
}

// ── Credential commands ────────────────────────────────────────

pub trait CredentialCommands {
    fn load_provider_api_key(&self, provider_id: String) -> Result<Option<String>, BackendError>;
    fn save_provider_api_key(
        &self,
        provider_id: String,
        api_key: String,
    ) -> Result<(), BackendError>;
    fn delete_provider_api_key(&self, provider_id: String) -> Result<(), BackendError>;
}

// ── Provider commands ──────────────────────────────────────────

pub trait ProviderCommands {
    fn resolve_provider_readiness(
        &self,
        settings: AppSettings,
        transient_api_key: Option<String>,
    ) -> ProviderReadiness;
}

// ── Run commands ───────────────────────────────────────────────

#[async_trait]
pub trait RunCommands: Send + Sync {
    async fn start_run(
        &self,
        workflow: Workflow,
        settings: AppSettings,
        transient_api_key: Option<String>,
    ) -> Result<WorkflowRunState, BackendError>;
    async fn submit_user_input(
        &self,
        node_id: String,
        text: String,
    ) -> Result<WorkflowRunState, BackendError>;
    async fn submit_tool_approval(
        &self,
        approval_id: String,
        allow: bool,
    ) -> Result<WorkflowRunState, BackendError>;
    async fn complete_manual_node(&self) -> Result<WorkflowRunState, BackendError>;
    async fn get_run_state(&self) -> Option<WorkflowRunState>;
    async fn clear_run_trace(&self) -> Result<Option<WorkflowRunState>, BackendError>;
}
