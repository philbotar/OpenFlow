//! Inbound adapters for orchestration (CLI, IPC, HTTP, etc.).

use crate::agent_store::AgentDefinition;
use crate::backend::{
	AgentDefinitionSummary, AppBackend, BackendError, ProviderReadiness, WorkflowListItem,
	WorkflowValidationSummary,
};
use crate::ports::inbound::OrchestrationCommandsPort;
use crate::settings_store::AppSettings;
use workflow_core::{Node, Workflow};

impl OrchestrationCommandsPort for AppBackend {
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

	fn load_settings(&self) -> Result<AppSettings, BackendError> {
		AppBackend::load_settings(self)
	}

	fn save_settings(&self, settings: &AppSettings) -> Result<(), BackendError> {
		AppBackend::save_settings(self, settings)
	}

	fn load_provider_api_key(&self, provider_id: &str) -> Result<Option<String>, BackendError> {
		AppBackend::load_provider_api_key(self, provider_id)
	}

	fn save_provider_api_key(
		&self,
		provider_id: &str,
		api_key: &str,
	) -> Result<(), BackendError> {
		AppBackend::save_provider_api_key(self, provider_id, api_key)
	}

	fn delete_provider_api_key(&self, provider_id: &str) -> Result<(), BackendError> {
		AppBackend::delete_provider_api_key(self, provider_id)
	}

	fn resolve_provider_readiness(
		&self,
		settings: &AppSettings,
		transient_api_key: Option<&str>,
	) -> ProviderReadiness {
		AppBackend::resolve_provider_readiness(self, settings, transient_api_key)
	}
}