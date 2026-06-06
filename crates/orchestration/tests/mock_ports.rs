//! Integration tests for orchestration port traits using mock implementations.
//!
//! These tests verify the trait contracts in isolation by providing
//! canned data through mock implementations of the outbound port traits.

use std::collections::BTreeMap;
use std::io;
use workflow_core::{Node, Workflow};

use app_backend::agent_store::AgentDefinition;
use app_backend::backend::{
    AgentDefinitionSummary, BackendError, ProviderReadiness, WorkflowListItem,
    WorkflowValidationSummary,
};
use app_backend::credential_store::{CredentialStore, CredentialStoreError};
use app_backend::provider_config::{ProviderConfigError, ProviderEnv};
use app_backend::settings_store::AppSettings;

use app_backend::ports::inbound::OrchestrationCommandsPort;
use app_backend::ports::outbound::{
    AgentStoragePort, CredentialLookupPort, ProviderConfigResolverPort, SettingsStoragePort,
    WorkflowStoragePort,
};

// ── Mock Stores ────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct MockWorkflowStore {
    pub workflows: Vec<Workflow>,
}

#[derive(Debug, Clone, Default)]
pub struct MockAgentStore {
    pub agents: Vec<AgentDefinition>,
}

#[derive(Debug, Clone, Default)]
pub struct MockSettingsStore {
    pub settings: AppSettings,
    pub credential_store: MockCredentialStore,
}

#[derive(Debug, Clone, Default)]
pub struct MockCredentialStore {
    pub keys: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Default)]
pub struct MockProviderResolver {
    pub ready: bool,
}

// ── WorkflowStoragePort impl ───────────────────────────────────

impl WorkflowStoragePort for MockWorkflowStore {
    fn load_workflows(&self) -> Result<Vec<Workflow>, io::Error> {
        Ok(self.workflows.clone())
    }

    fn save_workflows(&self, _workflows: &[Workflow]) -> Result<(), io::Error> {
        Ok(())
    }
}

// ── AgentStoragePort impl ──────────────────────────────────────

impl AgentStoragePort for MockAgentStore {
    fn load_agents(&self) -> Result<Vec<AgentDefinition>, io::Error> {
        Ok(self.agents.clone())
    }

    fn save_agents(&self, _agents: &[AgentDefinition]) -> Result<(), io::Error> {
        Ok(())
    }
}

// ── SettingsStoragePort impl ───────────────────────────────────

impl SettingsStoragePort for MockSettingsStore {
    fn load_settings(&self) -> Result<AppSettings, io::Error> {
        Ok(self.settings.clone())
    }

    fn save_settings(&self, _settings: &AppSettings) -> Result<(), io::Error> {
        Ok(())
    }

    fn credential_store(&self) -> &CredentialStore {
        // This is a bit awkward - we need to return a real CredentialStore
        // but for testing purposes, we can use a dummy implementation
        // In practice, this would need a mock that also implements CredentialStore
        unimplemented!("Mock credential_store not needed for these tests")
    }
}

// ── CredentialLookupPort impl ──────────────────────────────────

impl CredentialLookupPort for MockCredentialStore {
    fn get(&self, key_ref: &str) -> Result<Option<String>, CredentialStoreError> {
        Ok(self.keys.get(key_ref).cloned())
    }

    fn set(&self, _key_ref: &str, _value: &str) -> Result<(), CredentialStoreError> {
        Ok(())
    }

    fn delete(&self, _key_ref: &str) -> Result<(), CredentialStoreError> {
        Ok(())
    }
}

// ── ProviderConfigResolverPort impl ────────────────────────────

impl ProviderConfigResolverPort for MockProviderResolver {
    fn resolve_provider_config(
        &self,
        _settings: &AppSettings,
        _transient_api_key: Option<&str>,
        _env: &ProviderEnv,
        _credential_store: &CredentialStore,
    ) -> Result<ai::AiClientConfig, ProviderConfigError> {
        Ok(ai::AiClientConfig::openai("mock-key"))
    }
}

// ── Mock OrchestrationCommandsPort ─────────────────────────────

#[derive(Debug, Clone)]
pub struct MockOrchestrationCommands {
    pub workflows: Vec<Workflow>,
    pub agents: Vec<AgentDefinition>,
    pub settings: AppSettings,
    pub credentials: MockCredentialStore,
}

impl Default for MockOrchestrationCommands {
    fn default() -> Self {
        Self {
            workflows: Vec::new(),
            agents: Vec::new(),
            settings: AppSettings::default(),
            credentials: MockCredentialStore::default(),
        }
    }
}

impl MockOrchestrationCommands {
    pub fn with_workflow(mut self, workflow: Workflow) -> Self {
        self.workflows.push(workflow);
        self
    }

    pub fn with_agent(mut self, agent: AgentDefinition) -> Self {
        self.agents.push(agent);
        self
    }
}

impl OrchestrationCommandsPort for MockOrchestrationCommands {
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
    ) -> Result<Node, BackendError> {
        Ok(Node::agent("mock-node", 0.0, 0.0))
    }

    fn load_settings(&self) -> Result<AppSettings, BackendError> {
        Ok(self.settings.clone())
    }

    fn save_settings(&self, _settings: &AppSettings) -> Result<(), BackendError> {
        Ok(())
    }

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

// ── Workflow Storage Tests ─────────────────────────────────────

#[test]
fn mock_workflow_store_load_empty() {
    let store = MockWorkflowStore::default();
    let workflows = store.load_workflows().unwrap();
    assert!(workflows.is_empty());
}

#[test]
fn mock_workflow_store_load_returns_items() {
    let workflow = Workflow::new("Test Workflow");
    let store = MockWorkflowStore {
        workflows: vec![workflow],
    };
    let workflows = store.load_workflows().unwrap();

    assert_eq!(workflows.len(), 1);
    assert_eq!(workflows[0].name, "Test Workflow");
}

#[test]
fn mock_workflow_store_save_succeeds() {
    let store = MockWorkflowStore::default();
    let workflow = Workflow::new("New Workflow");
    let result = store.save_workflows(&[workflow]);
    assert!(result.is_ok());
}

// ── Agent Storage Tests ────────────────────────────────────────

#[test]
fn mock_agent_store_load_empty() {
    let store = MockAgentStore::default();
    let agents = store.load_agents().unwrap();
    assert!(agents.is_empty());
}

#[test]
fn mock_agent_store_load_returns_items() {
    let agent = AgentDefinition::new("Test Agent");
    let store = MockAgentStore {
        agents: vec![agent],
    };
    let agents = store.load_agents().unwrap();

    assert_eq!(agents.len(), 1);
    assert_eq!(agents[0].name, "Test Agent");
}

#[test]
fn mock_agent_store_save_succeeds() {
    let store = MockAgentStore::default();
    let agent = AgentDefinition::new("New Agent");
    let result = store.save_agents(&[agent]);
    assert!(result.is_ok());
}

// ── Credential Tests ───────────────────────────────────────────

#[test]
fn mock_credential_get_returns_key() {
    let mut store = MockCredentialStore::default();
    store
        .keys
        .insert("openai".to_string(), "test-key".to_string());

    let key = store.get("openai").unwrap();
    assert_eq!(key, Some("test-key".to_string()));
}

#[test]
fn mock_credential_get_returns_none() {
    let store = MockCredentialStore::default();
    let key = store.get("nonexistent").unwrap();
    assert!(key.is_none());
}

#[test]
fn mock_credential_set_succeeds() {
    let store = MockCredentialStore::default();
    let result = store.set("openai", "new-key");
    assert!(result.is_ok());
}

#[test]
fn mock_credential_delete_succeeds() {
    let store = MockCredentialStore::default();
    let result = store.delete("openai");
    assert!(result.is_ok());
}

// ── OrchestrationCommands Tests ────────────────────────────────

#[test]
fn mock_commands_list_workflows_empty() {
    let commands = MockOrchestrationCommands::default();
    let result = commands.list_workflows().unwrap();
    assert!(result.is_empty());
}

#[test]
fn mock_commands_list_workflows_returns_items() {
    let workflow = Workflow::new("Test Workflow");
    let commands = MockOrchestrationCommands::default().with_workflow(workflow);
    let result = commands.list_workflows().unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].name, "Test Workflow");
}

#[test]
fn mock_commands_load_workflow_found() {
    let mut workflow = Workflow::new("Test");
    workflow.id = "wf-1".into();

    let commands = MockOrchestrationCommands::default().with_workflow(workflow);
    let result = commands.load_workflow("wf-1").unwrap();

    assert_eq!(result.id, "wf-1");
}

#[test]
fn mock_commands_load_workflow_not_found() {
    let commands = MockOrchestrationCommands::default();
    let result = commands.load_workflow("nonexistent");

    assert!(result.is_err());
    match result.unwrap_err() {
        BackendError::WorkflowNotFound(id) => assert_eq!(id, "nonexistent"),
        _ => panic!("Expected WorkflowNotFound error"),
    }
}

#[test]
fn mock_commands_create_workflow() {
    let commands = MockOrchestrationCommands::default();
    let workflow = commands
        .create_workflow("New Workflow".to_string())
        .unwrap();
    assert_eq!(workflow.name, "New Workflow");
}

#[test]
fn mock_commands_save_workflow() {
    let commands = MockOrchestrationCommands::default();
    let workflow = Workflow::new("Test");
    let saved = commands.save_workflow(workflow.clone()).unwrap();
    assert_eq!(saved.name, workflow.name);
}

#[test]
fn mock_commands_rename_workflow() {
    let commands = MockOrchestrationCommands::default();
    let result = commands
        .rename_workflow("wf-1", "Renamed".to_string())
        .unwrap();
    assert_eq!(result.name, "Renamed");
}

#[test]
fn mock_commands_validate_workflow() {
    let commands = MockOrchestrationCommands::default();
    let workflow = Workflow::new("Test");
    let result = commands.validate_workflow(&workflow).unwrap();

    assert_eq!(result.layer_count, 1);
    assert_eq!(result.layers[0], vec!["node-1".to_string()]);
}

#[test]
fn mock_commands_list_agents_empty() {
    let commands = MockOrchestrationCommands::default();
    let result = commands.list_agents().unwrap();
    assert!(result.is_empty());
}

#[test]
fn mock_commands_list_agents_returns_items() {
    let mut agent = AgentDefinition::new("Test Agent");
    agent.id = "agent-1".to_string();
    agent.model = "gpt-4".to_string();

    let commands = MockOrchestrationCommands::default().with_agent(agent);
    let result = commands.list_agents().unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].id, "agent-1");
    assert_eq!(result[0].model, "gpt-4");
}

#[test]
fn mock_commands_create_agent_definition() {
    let commands = MockOrchestrationCommands::default();
    let agent = commands
        .create_agent_definition("New Agent".to_string())
        .unwrap();
    assert_eq!(agent.name, "mock-agent");
}

#[test]
fn mock_commands_load_settings() {
    let commands = MockOrchestrationCommands::default();
    let settings = commands.load_settings().unwrap();
    assert_eq!(settings, AppSettings::default());
}

#[test]
fn mock_commands_save_settings() {
    let commands = MockOrchestrationCommands::default();
    let settings = AppSettings::default();
    let result = commands.save_settings(&settings);
    assert!(result.is_ok());
}

#[test]
fn mock_commands_credential_operations() {
    let commands = MockOrchestrationCommands::default();

    let key = commands.load_provider_api_key("openai").unwrap();
    assert_eq!(key, Some("mock-api-key".to_string()));

    let save_result = commands.save_provider_api_key("openai", "test-key");
    assert!(save_result.is_ok());

    let delete_result = commands.delete_provider_api_key("openai");
    assert!(delete_result.is_ok());
}

#[test]
fn mock_commands_provider_readiness() {
    let commands = MockOrchestrationCommands::default();
    let settings = AppSettings::default();
    let readiness = commands.resolve_provider_readiness(&settings, None);

    assert!(readiness.ready);
    assert_eq!(readiness.provider, "mock-provider");
}
