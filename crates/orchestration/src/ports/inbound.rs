//! Inbound ports for orchestration use-cases.

use crate::agent_store::AgentDefinition;
use crate::backend::{
	AgentDefinitionSummary, BackendError, ProviderReadiness, WorkflowListItem,
	WorkflowValidationSummary,
};
use crate::settings_store::AppSettings;
use workflow_core::{Node, Workflow};

pub trait OrchestrationCommandsPort {
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

	fn load_settings(&self) -> Result<AppSettings, BackendError>;
	fn save_settings(&self, settings: &AppSettings) -> Result<(), BackendError>;
	fn load_provider_api_key(&self, provider_id: &str) -> Result<Option<String>, BackendError>;
	fn save_provider_api_key(&self, provider_id: &str, api_key: &str)
		-> Result<(), BackendError>;
	fn delete_provider_api_key(&self, provider_id: &str) -> Result<(), BackendError>;
	fn resolve_provider_readiness(
		&self,
		settings: &AppSettings,
		transient_api_key: Option<&str>,
	) -> ProviderReadiness;
}