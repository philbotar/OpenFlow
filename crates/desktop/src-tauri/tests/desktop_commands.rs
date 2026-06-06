//! Integration tests for desktop Tauri commands using a mock backend.
//!
//! These tests verify the command logic in isolation by providing
//! canned data through the port trait implementations.

use app_backend::Workflow;
use async_trait::async_trait;
use tokio::sync::mpsc;

use app_backend::agent_store::AgentDefinition;
use app_backend::backend::{
    AgentDefinitionSummary, BackendError, ProviderReadiness, WorkflowListItem,
    WorkflowValidationSummary,
};
use app_backend::execution::ExecutionEvent;
use app_backend::settings_store::AppSettings;
use app_backend::state::WorkflowRunState;

use agent_workflow_desktop_lib::ports::outbound::{
    AgentRepository, CredentialStore, ProviderResolver, RunOrchestrator, SettingsStore,
    WorkflowRepository,
};

// ── Mock Backend ───────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct MockBackend {
    pub workflows: Vec<Workflow>,
    pub agents: Vec<AgentDefinition>,
    pub settings: AppSettings,
    pub run_state: Option<WorkflowRunState>,
}

impl Default for MockBackend {
    fn default() -> Self {
        Self {
            workflows: Vec::new(),
            agents: Vec::new(),
            settings: AppSettings::default(),
            run_state: None,
        }
    }
}

impl MockBackend {
    pub fn with_workflow(mut self, workflow: Workflow) -> Self {
        self.workflows.push(workflow);
        self
    }

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
            .ok_or(BackendError::WorkflowNotFound(workflow_id.to_string()))
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
    ) -> Result<app_backend::Node, BackendError> {
        Ok(app_backend::Node::agent("mock-node", 0.0, 0.0))
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
fn mock_list_workflows_empty() {
    let backend = MockBackend::default();
    let result = backend.list_workflows().unwrap();
    assert!(result.is_empty());
}

#[test]
fn mock_list_workflows_returns_items() {
    let workflow = Workflow::new("Test Workflow");
    let backend = MockBackend::default().with_workflow(workflow);
    let result = backend.list_workflows().unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].name, "Test Workflow");
}

#[test]
fn mock_load_workflow_found() {
    let mut workflow = Workflow::new("Test");
    workflow.id = "wf-1".into();

    let backend = MockBackend::default().with_workflow(workflow);
    let result = backend.load_workflow("wf-1").unwrap();

    assert_eq!(result.id, "wf-1");
}

#[test]
fn mock_load_workflow_not_found() {
    let backend = MockBackend::default();
    let result = backend.load_workflow("nonexistent");

    assert!(result.is_err());
    match result.unwrap_err() {
        BackendError::WorkflowNotFound(id) => assert_eq!(id, "nonexistent"),
        _ => panic!("Expected WorkflowNotFound error"),
    }
}

#[test]
fn mock_create_workflow() {
    let backend = MockBackend::default();
    let workflow = backend.create_workflow("New Workflow".to_string()).unwrap();

    assert_eq!(workflow.name, "New Workflow");
}

#[test]
fn mock_save_workflow_roundtrip() {
    let backend = MockBackend::default();
    let workflow = Workflow::new("Test");

    let saved = backend.save_workflow(workflow.clone()).unwrap();
    assert_eq!(saved.name, workflow.name);
}

#[test]
fn mock_rename_workflow() {
    let backend = MockBackend::default();
    let result = backend
        .rename_workflow("wf-1", "Renamed".to_string())
        .unwrap();

    assert_eq!(result.name, "Renamed");
}

#[test]
fn mock_validate_workflow() {
    let backend = MockBackend::default();
    let workflow = Workflow::new("Test");
    let result = backend.validate_workflow(&workflow).unwrap();

    assert_eq!(result.layer_count, 1);
    assert_eq!(result.layers[0], vec!["node-1".to_string()]);
}

#[test]
fn mock_list_agents_empty() {
    let backend = MockBackend::default();
    let result = backend.list_agents().unwrap();
    assert!(result.is_empty());
}

#[test]
fn mock_list_agents_returns_items() {
    let mut agent = AgentDefinition::new("Test Agent");
    agent.id = "agent-1".to_string();
    agent.model = "gpt-4".to_string();

    let backend = MockBackend::default().with_agent(agent);
    let result = backend.list_agents().unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].id, "agent-1");
    assert_eq!(result[0].model, "gpt-4");
}

#[test]
fn mock_create_agent_definition() {
    let backend = MockBackend::default();
    let agent = backend
        .create_agent_definition("New Agent".to_string())
        .unwrap();

    assert_eq!(agent.name, "mock-agent");
}

#[test]
fn mock_load_settings() {
    let backend = MockBackend::default();
    let settings = backend.load_settings().unwrap();
    assert_eq!(settings, AppSettings::default());
}

#[test]
fn mock_save_settings() {
    let backend = MockBackend::default();
    let settings = AppSettings::default();
    let result = backend.save_settings(&settings);
    assert!(result.is_ok());
}

#[test]
fn mock_credential_operations() {
    let backend = MockBackend::default();

    let key = backend.load_provider_api_key("openai").unwrap();
    assert_eq!(key, Some("mock-api-key".to_string()));

    let save_result = backend.save_provider_api_key("openai", "test-key");
    assert!(save_result.is_ok());

    let delete_result = backend.delete_provider_api_key("openai");
    assert!(delete_result.is_ok());
}

#[test]
fn mock_provider_readiness() {
    let backend = MockBackend::default();
    let settings = AppSettings::default();
    let readiness = backend.resolve_provider_readiness(&settings, None);

    assert!(readiness.ready);
    assert_eq!(readiness.provider, "mock-provider");
}

#[tokio::test]
async fn mock_run_orchestrator() {
    let backend = MockBackend::default();
    let workflow = Workflow::new("Test");
    let settings = AppSettings::default();

    let (state, _rx) = backend
        .start_run(workflow.clone(), None, &settings, None)
        .await
        .unwrap();
    assert_eq!(state.active, true);

    let state = backend.get_run_state().await;
    assert!(state.is_none());

    let state = backend.clear_run_trace().await.unwrap();
    assert!(state.is_none());
}

#[test]
fn mock_default_backend() {
    let backend = MockBackend::default();
    assert!(backend.workflows.is_empty());
    assert!(backend.agents.is_empty());
    assert!(backend.run_state.is_none());
}
