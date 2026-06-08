//! Integration tests for desktop Tauri commands using a mock backend.
//!
//! These tests verify the command logic in isolation by providing
//! canned data through the port trait implementations.

use async_trait::async_trait;
use orchestration::Workflow;
use tokio::sync::mpsc;

use orchestration::agent_store::AgentDefinition;
use orchestration::backend::{
    AgentDefinitionSummary, BackendError, ProviderReadiness, WorkflowListItem,
    WorkflowValidationSummary,
};
use orchestration::execution::ExecutionEvent;
use orchestration::settings_store::AppSettings;
use orchestration::state::WorkflowRunState;

use desktop::ports::outbound::{
    AgentRepository, CredentialStore, ProviderResolver, RunOrchestrator, SettingsStore,
    WorkflowRepository,
};

// ── Mock Backend ───────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct MockBackend {
    pub workflows: Vec<Workflow>,
    pub agents: Vec<AgentDefinition>,
    pub settings: AppSettings,
    pub run_state: Option<WorkflowRunState>,
}

impl MockBackend {
    #[must_use]
    pub fn with_workflow(mut self, workflow: Workflow) -> Self {
        self.workflows.push(workflow);
        self
    }

    #[must_use]
    pub fn with_agent(mut self, agent: AgentDefinition) -> Self {
        self.agents.push(agent);
        self
    }
}

// ── WorkflowRepository impl ────────────────────────────────────

impl WorkflowRepository for MockBackend {
    fn list_workflows(&self) -> Result<Vec<WorkflowListItem>, BackendError> {
        Ok(self
            .workflows
            .iter()
            .map(|w| WorkflowListItem {
                id: w.id.to_string(),
                name: w.name.clone(),
            })
            .collect())
    }

    fn load_all_workflows(&self) -> Result<Vec<Workflow>, BackendError> {
        Ok(self.workflows.clone())
    }

    fn load_workflow(&self, workflow_id: &str) -> Result<Workflow, BackendError> {
        self.workflows
            .iter()
            .find(|w| w.id == workflow_id)
            .cloned()
            .ok_or_else(|| BackendError::WorkflowNotFound(workflow_id.to_string()))
    }

    fn create_workflow(&self, name: String) -> Result<Workflow, BackendError> {
        Ok(Workflow::new(name))
    }

    fn save_workflow(&self, workflow: Workflow) -> Result<Workflow, BackendError> {
        Ok(workflow)
    }

    fn save_workflows(&self, _workflows: &[Workflow]) -> Result<(), BackendError> {
        Ok(())
    }

    fn rename_workflow(
        &self,
        _workflow_id: &str,
        name: String,
    ) -> Result<WorkflowListItem, BackendError> {
        Ok(WorkflowListItem {
            id: "mock-id".to_string(),
            name,
        })
    }

    fn validate_workflow(
        &self,
        _workflow: &Workflow,
    ) -> Result<WorkflowValidationSummary, BackendError> {
        Ok(WorkflowValidationSummary {
            layer_count: 1,
            layers: vec![vec!["node-1".to_string()]],
        })
    }
}

// ── AgentRepository impl ───────────────────────────────────────

impl AgentRepository for MockBackend {
    fn list_agents(&self) -> Result<Vec<AgentDefinitionSummary>, BackendError> {
        Ok(self
            .agents
            .iter()
            .map(|a| AgentDefinitionSummary {
                id: a.id.clone(),
                name: a.name.clone(),
                model: a.model.clone(),
            })
            .collect())
    }

    fn load_agents(&self) -> Result<Vec<AgentDefinition>, BackendError> {
        Ok(self.agents.clone())
    }

    fn create_agent_definition(&self, _name: String) -> Result<AgentDefinition, BackendError> {
        Ok(AgentDefinition::new("mock-agent"))
    }

    fn save_agents(&self, _agents: &[AgentDefinition]) -> Result<(), BackendError> {
        Ok(())
    }

    fn create_agent_node(
        &self,
        _index: usize,
        _x: f32,
        _y: f32,
        _agent_id: Option<&str>,
    ) -> Result<orchestration::Node, BackendError> {
        Ok(orchestration::Node::agent("mock-node", 0.0, 0.0))
    }
}

// ── SettingsStore impl ─────────────────────────────────────────

impl SettingsStore for MockBackend {
    fn load_settings(&self) -> Result<AppSettings, BackendError> {
        Ok(self.settings.clone())
    }

    fn save_settings(&self, _settings: &AppSettings) -> Result<(), BackendError> {
        Ok(())
    }
}

// ── CredentialStore impl ───────────────────────────────────────

impl CredentialStore for MockBackend {
    fn load_provider_api_key(&self, _provider_id: &str) -> Result<Option<String>, BackendError> {
        Ok(Some("mock-api-key".to_string()))
    }

    fn save_provider_api_key(
        &self,
        _provider_id: &str,
        _api_key: &str,
    ) -> Result<(), BackendError> {
        Ok(())
    }

    fn delete_provider_api_key(&self, _provider_id: &str) -> Result<(), BackendError> {
        Ok(())
    }
}

// ── ProviderResolver impl ──────────────────────────────────────

impl ProviderResolver for MockBackend {
    fn resolve_provider_readiness(
        &self,
        _settings: &AppSettings,
        _transient_api_key: Option<&str>,
    ) -> ProviderReadiness {
        ProviderReadiness {
            ready: true,
            provider: "mock-provider".to_string(),
            message: "Ready".to_string(),
            env_var: "MOCK_API_KEY".to_string(),
        }
    }
}

// ── RunOrchestrator impl ───────────────────────────────────────

#[async_trait]
impl RunOrchestrator for MockBackend {
    async fn start_run(
        &self,
        workflow: Workflow,
        _entrypoint: Option<String>,
        _execution_cwd: Option<String>,
        _settings: &AppSettings,
        _transient_api_key: Option<&str>,
    ) -> Result<(WorkflowRunState, mpsc::UnboundedReceiver<ExecutionEvent>), BackendError> {
        let state = self
            .run_state
            .clone()
            .unwrap_or_else(|| WorkflowRunState::running_for_workflow(&workflow));
        let (_tx, rx) = mpsc::unbounded_channel();
        Ok((state, rx))
    }

    async fn apply_execution_event(
        &self,
        _event: ExecutionEvent,
    ) -> Result<WorkflowRunState, BackendError> {
        Ok(self
            .run_state
            .clone()
            .unwrap_or_else(|| WorkflowRunState::idle_for_workflow(&Workflow::new("mock"))))
    }

    async fn submit_user_input(
        &self,
        _node_id: &str,
        _text: String,
    ) -> Result<WorkflowRunState, BackendError> {
        Ok(self
            .run_state
            .clone()
            .unwrap_or_else(|| WorkflowRunState::idle_for_workflow(&Workflow::new("mock"))))
    }

    async fn submit_tool_approval(
        &self,
        _approval_id: &str,
        _allow: bool,
    ) -> Result<WorkflowRunState, BackendError> {
        Ok(self
            .run_state
            .clone()
            .unwrap_or_else(|| WorkflowRunState::idle_for_workflow(&Workflow::new("mock"))))
    }

    async fn complete_manual_node(&self) -> Result<WorkflowRunState, BackendError> {
        Ok(self
            .run_state
            .clone()
            .unwrap_or_else(|| WorkflowRunState::idle_for_workflow(&Workflow::new("mock"))))
    }

    async fn get_run_state(&self) -> Option<WorkflowRunState> {
        self.run_state.clone()
    }

    async fn clear_run_trace(&self) -> Result<Option<WorkflowRunState>, BackendError> {
        Ok(self.run_state.clone())
    }
}

// ── Tests ──────────────────────────────────────────────────────

#[test]
fn mock_default_backend() {
    let backend = MockBackend::default();
    assert!(backend.workflows.is_empty());
    assert!(backend.agents.is_empty());
    assert!(backend.run_state.is_none());
}
