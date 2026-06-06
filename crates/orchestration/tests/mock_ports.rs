//! Integration tests for orchestration port traits using mock implementations.
//!
//! These tests verify the trait contracts in isolation by providing
//! canned data through mock implementations of the outbound port traits.

use domain::{Node, Workflow};
use std::collections::BTreeMap;
use std::io;

use orchestration::agent_store::AgentDefinition;
use orchestration::backend::{
    AgentDefinitionSummary, BackendError, ProviderReadiness, WorkflowListItem,
    WorkflowValidationSummary,
};
use orchestration::credential_store::{CredentialStore, CredentialStoreError};
use orchestration::provider_config::{ProviderConfigError, ProviderEnv};
use orchestration::settings_store::AppSettings;

use orchestration::ports::inbound::OrchestrationCommandsPort;
use orchestration::ports::outbound::{
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
    ) -> Result<providers::AiClientConfig, ProviderConfigError> {
        Ok(providers::AiClientConfig::openai("mock-key"))
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
